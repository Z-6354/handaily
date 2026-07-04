import { invoke } from "@tauri-apps/api/core";

export type ResolveAssetUrl = (filename: string) => Promise<string>;

export interface PetAssetResolver {
  urlFor: ResolveAssetUrl;
  dispose: () => void;
}

function mimeForFilename(filename: string): string {
  if (filename.endsWith(".png")) return "image/png";
  if (filename.endsWith(".json")) return "application/json";
  if (filename.endsWith(".atlas")) return "text/plain";
  return "application/octet-stream";
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

  const blobUrls: string[] = [];
  const cache = new Map<string, string>();

  return {
    urlFor: async (filename) => {
      const hit = cache.get(filename);
      if (hit) return hit;
      const b64 = await invoke<string>("pet_read_model_asset", {
        modelId: cfg.model_id,
        filename,
      });
      const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
      const url = URL.createObjectURL(new Blob([bytes], { type: mimeForFilename(filename) }));
      blobUrls.push(url);
      cache.set(filename, url);
      return url;
    },
    dispose: () => {
      for (const url of blobUrls) URL.revokeObjectURL(url);
      blobUrls.length = 0;
      cache.clear();
    },
  };
}
