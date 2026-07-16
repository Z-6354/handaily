import { tauriInvoke as invoke } from "../../lib/tauriInvoke";
import { petLog } from "../log/petDebugLog";
import { prepareModelAssets } from "./assetPipeline";
import type { PetConfigPayload } from "./types";

/** 用户停止操作后多久开始后台预热同角色其他皮肤（毫秒） */
const IDLE_PRELOAD_DELAY_MS = 4000;

interface SkinRef {
  model_id: string;
  model_ready?: boolean;
}

interface PetMenuSkinsPayload {
  character_id: string;
  character_name: string;
  skins: SkinRef[];
}

let idleTimer: ReturnType<typeof setTimeout> | null = null;
let preloadGeneration = 0;
let runningPreloadGeneration = 0;

export function cancelDeferredSkinPreload(reason = "activity") {
  preloadGeneration += 1;
  if (idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }
  petLog("debug", "preload", "cancelled", { reason, gen: preloadGeneration });
}

export function scheduleSiblingSkinPreload(activeModelId: string, characterId?: string) {
  cancelDeferredSkinPreload("reschedule");
  const gen = preloadGeneration;
  idleTimer = setTimeout(() => {
    idleTimer = null;
    void runSiblingSkinPreload(gen, activeModelId, characterId);
  }, IDLE_PRELOAD_DELAY_MS);
  petLog("debug", "preload", "scheduled", {
    activeModelId,
    characterId: characterId ?? null,
    delayMs: IDLE_PRELOAD_DELAY_MS,
    gen,
  });
}

async function runSiblingSkinPreload(
  gen: number,
  activeModelId: string,
  characterId?: string,
) {
  if (gen !== preloadGeneration) return;
  runningPreloadGeneration = gen;

  try {
    let skins: SkinRef[];
    let resolvedCharacterId = characterId ?? "";
    if (characterId) {
      const menu = await invoke<PetMenuSkinsPayload>("characters_pet_menu_skins_for", {
        characterId,
      });
      skins = menu.skins;
      resolvedCharacterId = menu.character_id;
    } else {
      const menu = await invoke<PetMenuSkinsPayload>("characters_pet_menu_skins");
      skins = menu.skins;
      resolvedCharacterId = menu.character_id;
    }

    if (gen !== preloadGeneration) return;

    const siblings = skins.filter(
      (s) =>
        s.model_ready !== false &&
        s.model_id &&
        s.model_id !== activeModelId,
    );

    if (siblings.length === 0) {
      petLog("debug", "preload", "no siblings", {
        characterId: resolvedCharacterId,
        activeModelId,
      });
      return;
    }

    petLog("info", "preload", "start siblings", {
      characterId: resolvedCharacterId,
      activeModelId,
      count: siblings.length,
    });

    for (const skin of siblings) {
      if (gen !== preloadGeneration) {
        petLog("debug", "preload", "aborted mid-run", { gen, modelId: skin.model_id });
        return;
      }
      await preloadOneModel(skin.model_id);
    }

    petLog("info", "preload", "siblings done", {
      characterId: resolvedCharacterId,
      count: siblings.length,
    });
  } catch (err) {
    petLog("warn", "preload", "siblings failed", {
      err: err instanceof Error ? err.message : String(err),
    });
  } finally {
    if (runningPreloadGeneration === gen) {
      runningPreloadGeneration = 0;
    }
  }
}

async function preloadOneModel(modelId: string) {
  const t0 = performance.now();
  const cfg = await invoke<PetConfigPayload>("pet_resolve_model_preload_config", {
    modelId,
  });
  await prepareModelAssets(cfg, "hot");
  petLog("debug", "preload", "model warmed", {
    modelId: cfg.model_id,
    ms: Math.round(performance.now() - t0),
  });
}

export function isDeferredPreloadRunning(): boolean {
  return runningPreloadGeneration === preloadGeneration && runningPreloadGeneration > 0;
}
