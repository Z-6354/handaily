import {

  createPetAssetResolver,

  isModelBundleCached,

  preloadModelAssets,

  warmModelBundleCache,

  type PetAssetResolver,

} from "../petAssetResolver";

import { petLog, PetPhaseTimer } from "../log/petDebugLog";

import { parseViewerExSpineConfig } from "../viewerExConfig";

import type { PetConfigPayload } from "./types";

import { modelAssetFilenames } from "./types";



export interface AssetPipelineResult {

  resolver: PetAssetResolver;

  fallbackUrl: string;

}



/** 收集 Spine 加载所需的全部文件名（含 ViewerEX config 内引用的贴图） */

export async function collectModelAssetFilenames(

  cfg: PetConfigPayload,

  resolver: PetAssetResolver,

): Promise<string[]> {

  const files = new Set<string>([

    ...modelAssetFilenames(cfg),

    cfg.png_file,

  ].filter(Boolean));



  if (!cfg.config_file) {

    return [...files];

  }



  files.add(cfg.config_file);



  try {

    const url = await resolver.urlFor(cfg.config_file);

    let res = await fetch(url).catch(() => null);

    if ((!res || !res.ok) && resolver.readViaIpc) {

      const blobUrl = await resolver.readViaIpc(cfg.config_file);

      res = await fetch(blobUrl);

    }

    if (!res?.ok) {

      petLog("warn", "asset", "config fetch failed", {

        modelId: cfg.model_id,

        file: cfg.config_file,

      });

      return [...files];

    }



    const json = (await res.json()) as unknown;

    const vex = parseViewerExSpineConfig(json);

    files.add(vex.skeleton.trim());

    for (const atlas of vex.atlases) {

      if (atlas.atlas?.trim()) files.add(atlas.atlas.trim());

      for (const tex of atlas.textures ?? []) {

        if (tex?.trim()) files.add(tex.trim());

      }

    }

  } catch (err) {

    petLog("warn", "asset", "viewerEx config parse skipped", {

      modelId: cfg.model_id,

      file: cfg.config_file,

      err: err instanceof Error ? err.message : String(err),

    });

  }



  return [...files];

}



export async function prepareModelAssets(

  cfg: PetConfigPayload,

  mode: "cold" | "hot",

): Promise<AssetPipelineResult> {

  const timer = new PetPhaseTimer("asset");

  const resolver = createPetAssetResolver(cfg);

  const assetFiles = await collectModelAssetFilenames(cfg, resolver);



  let fallbackUrl: string;

  if (cfg.use_file_src) {

    const skipWarm = mode === "hot" && isModelBundleCached(cfg.model_id, assetFiles);

    if (skipWarm) {

      petLog("debug", "asset", "warm bundle cache hit", {

        modelId: cfg.model_id,

        files: assetFiles.length,

      });

      timer.mark("warm-bundle-hit");

    } else {

      petLog("info", "asset", "warm bundle cache", {

        modelId: cfg.model_id,

        files: assetFiles.length,

      });

      await warmModelBundleCache(cfg.model_id, assetFiles);

      timer.mark("warm-bundle");

    }

    fallbackUrl = await resolver.urlFor(cfg.png_file);

  } else {

    await preloadModelAssets(cfg.model_id, assetFiles, false, cfg.asset_base);

    timer.mark("preload-builtin");

    fallbackUrl = await resolver.urlFor(cfg.png_file);

  }



  timer.finish("prepareModelAssets");

  return { resolver, fallbackUrl };

}



export function disposeAssetResolver(resolver: PetAssetResolver | null) {

  resolver?.dispose();

}


