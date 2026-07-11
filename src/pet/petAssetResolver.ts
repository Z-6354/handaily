import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export type ResolveAssetUrl = (filename: string) => Promise<string>;

export interface PetAssetResolver {
  urlFor: ResolveAssetUrl;
  /** 用户模型：asset 协议加载失败时回退 IPC blob */
  readViaIpc?: (filename: string) => Promise<string>;
  dispose: () => void;
}

/** 最多保留约 2 套模型的 blob URL，切换模型时释放旧资源 */
const MAX_BLOB_ENTRIES = 8;

const GLOBAL_BLOB_CACHE = new Map<string, string>();

function cacheKey(modelId: string, filename: string) {
  return `${modelId}:${filename}`;
}

function evictOldestBlob() {
  const first = GLOBAL_BLOB_CACHE.keys().next().value;
  if (!first) return;
  const url = GLOBAL_BLOB_CACHE.get(first);
  if (url) URL.revokeObjectURL(url);
  GLOBAL_BLOB_CACHE.delete(first);
}

function mimeForFilename(filename: string): string {
  if (filename.endsWith(".png")) return "image/png";
  if (filename.endsWith(".json")) return "application/json";
  if (filename.endsWith(".atlas")) return "text/plain";
  return "application/octet-stream";
}

function blobUrlFromBase64(modelId: string, filename: string, b64: string): string {
  const key = cacheKey(modelId, filename);
  const hit = GLOBAL_BLOB_CACHE.get(key);
  if (hit) return hit;
  while (GLOBAL_BLOB_CACHE.size >= MAX_BLOB_ENTRIES) {
    evictOldestBlob();
  }
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type: mimeForFilename(filename) }));
  GLOBAL_BLOB_CACHE.set(key, url);
  return url;
}

export function petBlobCacheSize(): number {
  return GLOBAL_BLOB_CACHE.size;
}

/** 批量 IPC 预热用户模型 blob 缓存（doc 82 换肤快路径） */
export async function warmModelBundleCache(modelId: string, filenames: string[]): Promise<void> {
  const files = filenames.filter(Boolean);
  if (files.length === 0) return;
  try {
    const bundle = await invoke<{ files: Record<string, string> }>("pet_read_model_bundle", {
      modelId,
      filenames: files,
    });
    for (const [filename, b64] of Object.entries(bundle.files ?? {})) {
      blobUrlFromBase64(modelId, filename, b64);
    }
  } catch {
    // 批量失败时回退逐文件 readViaIpc
  }
}

/** 预读模型资源：仅内置模型走 HTTP 缓存；用户模型按需加载。 */
export async function preloadModelAssets(
  _modelId: string,
  filenames: string[],
  useFileSrc: boolean,
  assetBase?: string,
): Promise<void> {
  if (useFileSrc) return;

  const files = filenames.filter(Boolean);
  if (files.length === 0) return;

  const base = assetBase?.endsWith("/") ? assetBase : `${assetBase ?? ""}/`;
  if (!base || base === "/") return;
  await Promise.all(
    files.map(async (f) => {
      try {
        await fetch(`${base}${f}`, { cache: "force-cache" });
      } catch {
        // 预加载失败时按需读取
      }
    }),
  );
}

function joinAssetPath(base: string, filename: string): string {
  const trimmed = base.replace(/[/\\]+$/, "");
  return `${trimmed}/${filename}`;
}

async function readModelAssetViaIpc(modelId: string, filename: string): Promise<string> {
  const key = cacheKey(modelId, filename);
  const cached = GLOBAL_BLOB_CACHE.get(key);
  if (cached) return cached;
  const b64 = await invoke<string>("pet_read_model_asset", { modelId, filename });
  return blobUrlFromBase64(modelId, filename, b64);
}

export function createPetAssetResolver(cfg: {
  asset_base: string;
  use_file_src: boolean;
  model_id: string;
}): PetAssetResolver {
  const base = cfg.asset_base.endsWith("/") ? cfg.asset_base : `${cfg.asset_base}/`;

  if (!cfg.use_file_src) {
    return {
      urlFor: async (filename) => `${base}${filename}`,
      dispose: () => {},
    };
  }

  return {
    urlFor: async (filename) => {
      const key = cacheKey(cfg.model_id, filename);
      const cached = GLOBAL_BLOB_CACHE.get(key);
      if (cached) return cached;
      return convertFileSrc(joinAssetPath(cfg.asset_base, filename));
    },
    readViaIpc: async (filename) => readModelAssetViaIpc(cfg.model_id, filename),
    dispose: () => {},
  };
}
