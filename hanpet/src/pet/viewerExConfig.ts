/** Live2DViewerEX Spine 配置（.config.json / config.json，type=9） */

export interface ViewerExSpineAtlasEntry {
  atlas: string;
  tex_names: string[];
  textures: string[];
}

export interface ViewerExSpineConfig {
  conf_ver?: number;
  type?: number;
  skeleton: string;
  atlases: ViewerExSpineAtlasEntry[];
  options?: Record<string, unknown>;
  motions?: Record<string, unknown[]>;
  bones?: Array<Record<string, unknown>>;
  skins?: string[];
  skin?: string;
  default_skin?: string;
}

export interface PetAssetPaths {
  skelFile: string;
  atlasFile: string;
  pngFile: string;
}

export interface ViewerExLoadResult extends PetAssetPaths {
  config: ViewerExSpineConfig;
}

export function parseViewerExSpineConfig(raw: unknown): ViewerExSpineConfig {
  if (!raw || typeof raw !== "object") {
    throw new Error("配置 JSON 无效");
  }
  const cfg = raw as Partial<ViewerExSpineConfig>;
  if (cfg.type != null && cfg.type !== 9) {
    throw new Error(`不支持的模型类型: ${cfg.type}（仅支持 Spine type=9）`);
  }
  const skeleton = cfg.skeleton?.trim();
  if (!skeleton) throw new Error("配置缺少 skeleton 字段");

  const atlases = cfg.atlases;
  if (!Array.isArray(atlases) || atlases.length === 0) {
    throw new Error("配置缺少 atlases");
  }
  const first = atlases[0];
  const atlasFile = first.atlas?.trim();
  if (!atlasFile) throw new Error("配置 atlases[0].atlas 无效");

  const textures = first.textures;
  if (!Array.isArray(textures) || textures.length === 0 || !textures[0]?.trim()) {
    throw new Error("配置 atlases[0].textures 无效");
  }

  return cfg as ViewerExSpineConfig;
}

export function assetPathsFromConfig(cfg: ViewerExSpineConfig): PetAssetPaths {
  const first = cfg.atlases[0];
  return {
    skelFile: cfg.skeleton.trim(),
    atlasFile: first.atlas.trim(),
    pngFile: first.textures[0].trim(),
  };
}

const CONFIG_CANDIDATES = [".config.json", "config.json"];

export async function loadViewerExSpineConfig(
  pathPrefix: string,
  configFile?: string | null,
  resolveUrl?: (filename: string) => Promise<string>,
  readViaIpc?: (filename: string) => Promise<string>,
): Promise<ViewerExLoadResult | null> {
  const base = pathPrefix.endsWith("/") ? pathPrefix : `${pathPrefix}/`;
  const candidates = configFile
    ? [configFile, ...CONFIG_CANDIDATES.filter((c) => c !== configFile)]
    : CONFIG_CANDIDATES;

  let lastErr: unknown;
  for (const file of candidates) {
    try {
      const url = resolveUrl ? await resolveUrl(file) : `${base}${file}`;
      let res = await fetch(url).catch(() => null);
      if ((!res || !res.ok) && readViaIpc) {
        const blobUrl = await readViaIpc(file);
        res = await fetch(blobUrl);
      }
      if (!res?.ok) continue;
      const json = (await res.json()) as unknown;
      const config = parseViewerExSpineConfig(json);
      return { ...assetPathsFromConfig(config), config };
    } catch (e) {
      lastErr = e;
    }
  }
  if (configFile) {
    throw lastErr instanceof Error ? lastErr : new Error(`配置加载失败: ${configFile}`);
  }
  return null;
}
