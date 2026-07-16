import { convertFileSrc } from "@tauri-apps/api/core";
import { tauriInvoke as invoke } from "../lib/tauriInvoke";
import { Cubism4ModelSettings } from "pixi-live2d-display-lipsyncpatch/cubism4";

/** LRU：仅回退路径的 blob；asset URL 不占用撤销列表 */
const BLOB_CACHE_MAX = 64;
const BLOB_CACHE = new Map<string, string>();
const IS_BLOB_URL = new Set<string>();
/** 当前模型 resolveURL 用的相对路径 → url */
let activeBlobByRelative: Map<string, string> | null = null;
let activeModelDir: string | null = null;
let activeAbsDir: string | null = null;

export type WarmDeferredFn = (model?: {
  internalModel?: {
    motionManager?: {
      definitions?: Record<string, CubismMotionRef[] | undefined>;
    };
  };
}) => Promise<void>;

function cacheKey(modelDir: string, filename: string) {
  return `${modelDir}:${filename}`;
}

function mimeForFilename(filename: string): string {
  if (filename.endsWith(".png")) return "image/png";
  if (filename.endsWith(".json")) return "application/json";
  if (filename.endsWith(".moc3")) return "application/octet-stream";
  return "application/octet-stream";
}

function bytesFromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const len = bin.length;
  const out = new Uint8Array(len);
  for (let i = 0; i < len; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function joinModelAbs(absDir: string, rel: string): string {
  const base = absDir.replace(/\\/g, "/").replace(/\/+$/, "");
  const r = rel.replace(/\\/g, "/").replace(/^\/+/, "");
  return `${base}/${r}`;
}

export function assetUrlFor(absDir: string, rel: string): string {
  return convertFileSrc(joinModelAbs(absDir, rel));
}

function touchLru(key: string, url: string, isBlob: boolean) {
  if (BLOB_CACHE.has(key)) BLOB_CACHE.delete(key);
  BLOB_CACHE.set(key, url);
  if (isBlob) IS_BLOB_URL.add(url);
  while (BLOB_CACHE.size > BLOB_CACHE_MAX) {
    let victim: string | undefined;
    for (const k of BLOB_CACHE.keys()) {
      if (activeModelDir && k.startsWith(`${activeModelDir}:`)) continue;
      victim = k;
      break;
    }
    if (!victim) victim = BLOB_CACHE.keys().next().value;
    if (!victim) break;
    const old = BLOB_CACHE.get(victim);
    if (old && IS_BLOB_URL.has(old)) {
      URL.revokeObjectURL(old);
      IS_BLOB_URL.delete(old);
    }
    BLOB_CACHE.delete(victim);
  }
}

function blobUrlFromBase64(modelDir: string, filename: string, b64: string): string {
  const key = cacheKey(modelDir, filename);
  const hit = BLOB_CACHE.get(key);
  if (hit) {
    touchLru(key, hit, IS_BLOB_URL.has(hit));
    return hit;
  }
  const bytes = bytesFromBase64(b64);
  const url = URL.createObjectURL(
    new Blob([bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength)], {
      type: mimeForFilename(filename),
    }),
  );
  touchLru(key, url, true);
  return url;
}

function ingestBundleFiles(modelDir: string, files: Record<string, string> | undefined) {
  for (const [filename, b64] of Object.entries(files ?? {})) {
    blobUrlFromBase64(modelDir, filename, b64);
  }
}

export function disposeKanmusuBlobs(modelDir?: string) {
  for (const [key, url] of [...BLOB_CACHE.entries()]) {
    if (modelDir && !key.startsWith(`${modelDir}:`)) continue;
    if (IS_BLOB_URL.has(url)) {
      URL.revokeObjectURL(url);
      IS_BLOB_URL.delete(url);
    }
    BLOB_CACHE.delete(key);
  }
  if (!modelDir || activeModelDir === modelDir) {
    activeBlobByRelative = null;
    activeModelDir = null;
    activeAbsDir = null;
  }
}

export function clearActiveKanmusuResolveMap() {
  activeBlobByRelative = null;
  activeModelDir = null;
  activeAbsDir = null;
}

export async function kanmusuBlobUrl(
  modelDir: string,
  filename: string,
): Promise<string> {
  const key = cacheKey(modelDir, filename);
  const cached = BLOB_CACHE.get(key);
  if (cached) {
    touchLru(key, cached, IS_BLOB_URL.has(cached));
    return cached;
  }
  if (activeAbsDir && activeModelDir === modelDir) {
    const url = assetUrlFor(activeAbsDir, filename);
    touchLru(key, url, false);
    return url;
  }
  const b64 = await invoke<string>("kanmusu_read_model_asset", { modelDir, filename });
  return blobUrlFromBase64(modelDir, filename, b64);
}

export async function kanmusuWarmBundle(
  modelDir: string,
  filenames: string[],
): Promise<void> {
  const files = filenames.filter(Boolean);
  if (files.length === 0) return;
  // asset 主路径：只需把 URL 登记进缓存，无需 IPC
  if (activeAbsDir && activeModelDir === modelDir) {
    for (const f of files) {
      const key = cacheKey(modelDir, f);
      if (BLOB_CACHE.has(key)) continue;
      touchLru(key, assetUrlFor(activeAbsDir, f), false);
    }
    return;
  }
  const missing = files.filter((f) => !BLOB_CACHE.has(cacheKey(modelDir, f)));
  if (missing.length === 0) return;
  try {
    const bundle = await invoke<{ files: Record<string, string> }>("kanmusu_read_model_bundle", {
      modelDir,
      filenames: missing,
    });
    ingestBundleFiles(modelDir, bundle.files);
  } catch {
    for (const filename of missing) {
      await kanmusuBlobUrl(modelDir, filename);
    }
  }
}

interface CubismMotionRef {
  File?: string;
  Name?: string;
}

interface CubismModel3Json {
  url?: string;
  Version?: number;
  FileReferences: {
    Moc: string;
    Textures: string[];
    Physics?: string;
    Expressions?: unknown;
    Motions?: Record<string, CubismMotionRef[] | undefined>;
  };
  Groups?: unknown[];
  HitAreas?: Array<{ Name?: string; Id?: string }>;
}

export type CubismPriorityHints = {
  idle?: string | null;
  click?: string | null;
  drag?: string | null;
  boot?: string | null;
  extra?: string[];
};

const ESSENTIAL_NAME_HINTS = [
  "idle",
  "login",
  "home",
  "welcome",
];

function motionMatchesHints(file: string, name: string | undefined, hints: string[]): boolean {
  const hay = `${file} ${name ?? ""}`.toLowerCase();
  return hints.some((h) => h && hay.includes(h.toLowerCase()));
}

function addEssentialMotion(
  file: string,
  item: CubismMotionRef,
  group: string,
  essentialMotionFiles: Set<string>,
  essentialMotions: Record<string, CubismMotionRef[]>,
  deferredMotionFiles: Set<string>,
  max: number,
): boolean {
  if (essentialMotionFiles.has(file)) return true;
  if (essentialMotionFiles.size >= max) return false;
  essentialMotionFiles.add(file);
  (essentialMotions[group] ??= []).push(item);
  deferredMotionFiles.delete(file);
  return true;
}

/**
 * 首包顺序必须是：idle/boot → 三区 touch → 少量 main。
 * 若先 pin touch/main 占满槽位，idle 被挤出 → Cubism 默认 PartOpacity
 * 常把多套部件全亮，看起来像「部件重复」。
 */
export function splitCubismFilenames(
  json: CubismModel3Json,
  priority: CubismPriorityHints = {},
): {
  essential: string[];
  deferred: string[];
  essentialMotions: Record<string, CubismMotionRef[]>;
  deferredMotions: Record<string, CubismMotionRef[]>;
} {
  const refs = json.FileReferences;
  const core = new Set<string>([refs.Moc, ...refs.Textures]);
  if (refs.Physics) core.add(refs.Physics);

  const essentialMotions: Record<string, CubismMotionRef[]> = {};
  const deferredMotions: Record<string, CubismMotionRef[]> = {};
  const essentialMotionFiles = new Set<string>();
  const deferredMotionFiles = new Set<string>();
  // idle(1~2) + touch×3 + main×1~2
  const MAX_ESSENTIAL_MOTIONS = 8;

  const motions = refs.Motions;
  if (!motions || typeof motions !== "object") {
    return {
      essential: [...core].filter(Boolean),
      deferred: [],
      essentialMotions,
      deferredMotions,
    };
  }

  const allItems: Array<{ group: string; item: CubismMotionRef; file: string }> = [];
  for (const [group, list] of Object.entries(motions)) {
    if (!Array.isArray(list)) continue;
    for (const item of list) {
      const file = item?.File?.trim();
      if (!file) continue;
      allItems.push({ group, item, file });
    }
  }

  const tryPin = (hints: string[]) => {
    const cleaned = hints.filter((x) => !!x && !!String(x).trim());
    if (!cleaned.length) return;
    for (const { group, item, file } of allItems) {
      if (essentialMotionFiles.size >= MAX_ESSENTIAL_MOTIONS) break;
      if (!motionMatchesHints(file, item.Name, cleaned)) continue;
      addEssentialMotion(
        file,
        item,
        group,
        essentialMotionFiles,
        essentialMotions,
        deferredMotionFiles,
        MAX_ESSENTIAL_MOTIONS,
      );
    }
  };

  // 1) idle / boot 绝对优先（决定部件显隐）
  tryPin([
    priority.idle ?? "",
    priority.boot ?? "",
    ...ESSENTIAL_NAME_HINTS,
  ]);
  for (const { group, item, file } of allItems) {
    if (essentialMotionFiles.size >= MAX_ESSENTIAL_MOTIONS) break;
    if (!/^(idle)$/i.test(group)) continue;
    addEssentialMotion(
      file,
      item,
      group,
      essentialMotionFiles,
      essentialMotions,
      deferredMotionFiles,
      MAX_ESSENTIAL_MOTIONS,
    );
  }

  // 2) 三区 touch（及 payload 给出的 click）
  tryPin([
    priority.click ?? "",
    ...(priority.extra ?? []).filter((x) => /touch/i.test(x)),
    "touch_head",
    "touch_body",
    "touch_special",
  ]);

  // 3) 整模回落 main（有槽再进）
  tryPin([
    ...(priority.extra ?? []).filter((x) => /main_/i.test(x)),
    "main_1",
    "main_2",
  ]);

  // 4) drag 等其它优先级提示
  tryPin([priority.drag ?? "", ...(priority.extra ?? [])]);

  // 其余进 deferred
  for (const { group, item, file } of allItems) {
    if (essentialMotionFiles.has(file)) continue;
    deferredMotionFiles.add(file);
    (deferredMotions[group] ??= []).push(item);
  }

  if (essentialMotionFiles.size === 0) {
    for (const { group, item, file } of allItems) {
      addEssentialMotion(
        file,
        item,
        group,
        essentialMotionFiles,
        essentialMotions,
        deferredMotionFiles,
        MAX_ESSENTIAL_MOTIONS,
      );
      if (essentialMotionFiles.size >= 2) break;
    }
  }

  const essential = [...core, ...essentialMotionFiles].filter(Boolean);
  const deferred = [...deferredMotionFiles].filter((f) => !essentialMotionFiles.has(f));
  return { essential, deferred, essentialMotions, deferredMotions };
}

export function collectCubismFilenames(json: CubismModel3Json): string[] {
  const { essential, deferred } = splitCubismFilenames(json);
  return [...essential, ...deferred];
}

function decodeRelPath(path: string): string {
  try {
    return decodeURI(path);
  } catch {
    return path;
  }
}

function bindResolveURL(
  settings: Cubism4ModelSettings,
  urlByRelative: Map<string, string>,
  absDir?: string | null,
) {
  settings.resolveURL = (path: string) => {
    if (
      path.startsWith("blob:") ||
      path.startsWith("asset:") ||
      path.startsWith("http:") ||
      path.startsWith("https:")
    ) {
      return path;
    }
    const decoded = decodeRelPath(path);
    const hit = urlByRelative.get(path) ?? urlByRelative.get(decoded);
    if (hit) return hit;
    if (absDir) {
      const url = assetUrlFor(absDir, decoded);
      urlByRelative.set(decoded, url);
      return url;
    }
    throw new Error(`舰娘资源未映射: ${path}`);
  };
}

async function fetchModel3Json(absDir: string, model3Filename: string): Promise<CubismModel3Json> {
  const url = assetUrlFor(absDir, model3Filename);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`读取 model3 失败: ${res.status}`);
  return (await res.json()) as CubismModel3Json;
}

function finishSettings(
  json: CubismModel3Json,
  modelDir: string,
  model3Filename: string,
  priority: CubismPriorityHints,
  urlByRelative: Map<string, string>,
  absDir: string | null,
  deferred: string[],
  deferredMotions: Record<string, CubismMotionRef[]>,
  essentialMotions: Record<string, CubismMotionRef[]>,
): { settings: Cubism4ModelSettings; warmDeferred: WarmDeferredFn } {
  activeBlobByRelative = urlByRelative;
  activeModelDir = modelDir;
  activeAbsDir = absDir;

  const out = {
    ...json,
    FileReferences: {
      ...json.FileReferences,
      Motions: essentialMotions,
    },
    url: model3Filename,
  };
  const settings = new Cubism4ModelSettings(out as never);
  bindResolveURL(settings, urlByRelative, absDir);

  const warmDeferred: WarmDeferredFn = async (model) => {
    if (!Object.keys(deferredMotions).length && deferred.length === 0) return;
    try {
      if (absDir) {
        // asset：无需 IPC，登记 URL + 合并 definitions
        for (const rel of deferred) {
          urlByRelative.set(rel, assetUrlFor(absDir, rel));
        }
      } else {
        await kanmusuWarmBundle(modelDir, deferred);
        if (activeModelDir !== modelDir || activeBlobByRelative !== urlByRelative) return;
        for (const rel of deferred) {
          urlByRelative.set(rel, await kanmusuBlobUrl(modelDir, rel));
        }
      }
      if (activeModelDir !== modelDir) return;
      const defs = model?.internalModel?.motionManager?.definitions;
      if (defs) {
        for (const [group, list] of Object.entries(deferredMotions)) {
          if (!list?.length) continue;
          const prev = defs[group] ?? [];
          const seen = new Set(
            prev
              .map((m) => (m.File || m.Name || "").toLowerCase())
              .filter(Boolean),
          );
          const fresh = list.filter((m) => {
            const key = (m.File || m.Name || "").toLowerCase();
            if (!key || seen.has(key)) return false;
            seen.add(key);
            return true;
          });
          if (fresh.length) defs[group] = [...prev, ...fresh];
        }
      }
    } catch {
      /* 后台失败不影响 idle */
    }
  };

  return { settings, warmDeferred };
}

/**
 * 主路径：convertFileSrc(AppData)；失败回退 prime_model / base64。
 */
export async function buildCubismSettings(
  modelDir: string,
  model3Filename: string,
  priority: CubismPriorityHints = {},
  modelAbsDir?: string | null,
): Promise<{
  settings: Cubism4ModelSettings;
  warmDeferred: WarmDeferredFn;
}> {
  const abs = modelAbsDir?.trim() || "";

  if (abs) {
    try {
      const json = await fetchModel3Json(abs, model3Filename);
      const { essential, deferred, essentialMotions, deferredMotions } = splitCubismFilenames(
        json,
        priority,
      );
      const urlByRelative = new Map<string, string>();
      for (const rel of essential) {
        const url = assetUrlFor(abs, rel);
        urlByRelative.set(rel, url);
        touchLru(cacheKey(modelDir, rel), url, false);
      }
      // 预检 moc：ensure asset 协议可读（贴图可懒加载）
      const moc = json.FileReferences?.Moc;
      if (moc) {
        // GET 首字节即可；HEAD 在部分 asset 协议实现上更慢或不可用
        const probe = await fetch(assetUrlFor(abs, moc), {
          method: "GET",
          headers: { Range: "bytes=0-0" },
        }).catch(() => null);
        if (probe && !(probe.ok || probe.status === 206)) {
          throw new Error(`asset 不可读: ${moc}`);
        }
      }
      return finishSettings(
        json,
        modelDir,
        model3Filename,
        priority,
        urlByRelative,
        abs,
        deferred,
        deferredMotions,
        essentialMotions,
      );
    } catch (e) {
      console.warn("[kanmusu] convertFileSrc 失败，回退 base64", e);
    }
  }

  const priorityNames = [
    priority.idle,
    priority.click,
    priority.drag,
    priority.boot,
    ...(priority.extra ?? []),
  ].filter((x): x is string => !!x && !!String(x).trim());

  let json: CubismModel3Json;
  try {
    const prime = await invoke<{
      model3_json: string;
      files: Record<string, string>;
    }>("kanmusu_prime_model", {
      modelDir,
      model3Filename,
      priorityNames,
    });
    json = JSON.parse(prime.model3_json) as CubismModel3Json;
    ingestBundleFiles(modelDir, prime.files);
  } catch {
    const settingsB64 = await invoke<string>("kanmusu_read_model_asset", {
      modelDir,
      filename: model3Filename,
    });
    const settingsText = new TextDecoder().decode(bytesFromBase64(settingsB64));
    json = JSON.parse(settingsText) as CubismModel3Json;
    const { essential } = splitCubismFilenames(json, priority);
    await kanmusuWarmBundle(modelDir, essential);
  }

  const { essential, deferred, essentialMotions, deferredMotions } = splitCubismFilenames(
    json,
    priority,
  );
  await kanmusuWarmBundle(modelDir, essential);

  const urlByRelative = new Map<string, string>();
  for (const rel of essential) {
    urlByRelative.set(rel, await kanmusuBlobUrl(modelDir, rel));
  }

  return finishSettings(
    json,
    modelDir,
    model3Filename,
    priority,
    urlByRelative,
    null,
    deferred,
    deferredMotions,
    essentialMotions,
  );
}

export function model3FilenameFromPath(model3Path: string): string {
  const normalized = model3Path.replace(/\\/g, "/");
  const idx = normalized.lastIndexOf("/");
  return idx >= 0 ? normalized.slice(idx + 1) : normalized;
}

/** 空闲预热：prefer asset HEAD；否则仍走 prime */
export function prefetchKanmusuSkin(
  modelDir: string,
  model3Filename: string,
  priorityNames: string[] = ["idle", "touch", "login", "home"],
  modelAbsDir?: string | null,
): Promise<void> {
  if (!modelDir || !model3Filename) return Promise.resolve();
  if (BLOB_CACHE.has(cacheKey(modelDir, model3Filename))) {
    return Promise.resolve();
  }
  const abs = modelAbsDir?.trim();
  if (abs) {
    return fetch(assetUrlFor(abs, model3Filename))
      .then(async (res) => {
        if (!res.ok) throw new Error(String(res.status));
        const json = (await res.json()) as CubismModel3Json;
        const { essential } = splitCubismFilenames(json, {
          extra: priorityNames,
        });
        for (const rel of essential.slice(0, 6)) {
          touchLru(cacheKey(modelDir, rel), assetUrlFor(abs, rel), false);
          void fetch(assetUrlFor(abs, rel)).catch(() => undefined);
        }
      })
      .catch(() =>
        invoke<{ model3_json: string; files: Record<string, string> }>("kanmusu_prime_model", {
          modelDir,
          model3Filename,
          priorityNames,
        }).then((prime) => ingestBundleFiles(modelDir, prime.files)),
      );
  }
  return invoke<{ model3_json: string; files: Record<string, string> }>("kanmusu_prime_model", {
    modelDir,
    model3Filename,
    priorityNames,
  })
    .then((prime) => {
      ingestBundleFiles(modelDir, prime.files);
    })
    .catch(() => undefined);
}

/** 点击补拉：确保相对路径可读 */
export async function ensureKanmusuAssetReady(
  modelDir: string,
  filename: string,
): Promise<string | null> {
  if (!filename) return null;
  try {
    return await kanmusuBlobUrl(modelDir, filename);
  } catch {
    return null;
  }
}
