import { SpinePet } from "../spinePet";
import type { PetAssetResolver } from "../petAssetResolver";
import { petLog, PetPhaseTimer } from "../log/petDebugLog";
import type { PetConfigPayload, SpineInitMode, SpineLoadResult } from "./types";
import { assetConfigFromPayload } from "./types";
import { prepareCanvasSurface, showFallbackSurface, showSpineSurface } from "./canvasHost";

export interface SpineLoaderDeps {
  canvas: HTMLCanvasElement;
  canvasWrap: HTMLElement;
  fallback: HTMLImageElement;
  displayW: number;
  displayH: number;
  pickLine: (lines: PetConfigPayload["lines"], animation: string) => string | null;
  showBubble: (text: string, animation?: string) => void;
  showBootHint?: () => void;
  hideBootHint?: () => void;
  onCanvasReplaced?: (canvas: HTMLCanvasElement) => void;
}

export interface SpineSession {
  pet: SpinePet;
  lines: PetConfigPayload["lines"];
}

export async function loadSpineSession(
  cfg: PetConfigPayload,
  resolver: PetAssetResolver,
  prior: SpinePet | null,
  mode: SpineInitMode,
  deps: SpineLoaderDeps,
): Promise<{ session: SpineSession | null; result: SpineLoadResult }> {
  const timer = new PetPhaseTimer("spine");
  const hadPet = prior !== null;
  const assets = assetConfigFromPayload(cfg);
  const petLines = cfg.lines ?? [];

  if (prior) {
    prior.dispose(mode === "teardown" ? "teardown" : "swap");
  }

  let canvasEl = await prepareCanvasSurface(deps.canvas, deps.canvasWrap, {
    mode,
    hadPet,
    displayW: deps.displayW,
    displayH: deps.displayH,
    showBootHint: mode !== "hot" ? deps.showBootHint : undefined,
  });
  if (canvasEl !== deps.canvas) {
    deps.onCanvasReplaced?.(canvasEl);
  }
  timer.mark("canvas-prep");

  let next: SpinePet | null = null;
  try {
    next = new SpinePet(canvasEl, assets, {
      resolveAssetUrl: resolver.urlFor,
      readViaIpc: resolver.readViaIpc,
      skipBootAnimation: true,
      idleAnimation: cfg.idle_animation,
      clickAnimation: cfg.click_animation,
      bootAnimation: cfg.boot_animation,
      returnIdleAnimation: cfg.return_idle_animation,
      dragAnimation: cfg.drag_animation,
      randomAnimations: cfg.random_animations ?? [],
      randomMinSec: cfg.random_min_sec ?? 30,
      randomMaxSec: cfg.random_max_sec ?? 120,
      onRandomAction: (name: string) => {
        const text = deps.pickLine(petLines, name);
        if (text) deps.showBubble(text, name);
      },
      onTap: (animation) => {
        if (!animation) return;
        const text = deps.pickLine(petLines, animation);
        if (text) deps.showBubble(text, animation);
      },
    });
    timer.mark("construct");

    const names = await next.start();
    timer.mark("start", { animations: names.length });

    next.resizeCanvas(deps.displayW, deps.displayH, mode !== "hot");
    next.configureAnimations(
      {
        idleAnimation: cfg.idle_animation,
        clickAnimation: cfg.click_animation,
        bootAnimation: cfg.boot_animation,
        returnIdleAnimation: cfg.return_idle_animation,
        dragAnimation: cfg.drag_animation,
        randomAnimations: cfg.random_animations ?? [],
        randomMinSec: cfg.random_min_sec ?? 30,
        randomMaxSec: cfg.random_max_sec ?? 120,
      },
      { soft: true },
    );

    showSpineSurface(deps.canvasWrap, deps.fallback);
    deps.hideBootHint?.();

    const durationMs = timer.finish("loadSpineSession");
    petLog("info", "spine", "loaded", { modelId: cfg.model_id, mode, durationMs });

    return {
      session: { pet: next, lines: petLines },
      result: { ok: true, animationNames: names, mode, durationMs },
    };
  } catch (err) {
    next?.dispose("teardown");
    const errMsg = err instanceof Error ? err.message : String(err);
    petLog("error", "spine", "load failed", {
      modelId: cfg.model_id,
      mode,
      err: errMsg,
      skel: cfg.skel_file,
      atlas: cfg.atlas_file,
      png: cfg.png_file,
    });
    const durationMs = timer.finish("loadSpineSession-fail");

    if (mode === "hot") {
      deps.hideBootHint?.();
      return {
        session: null,
        result: { ok: false, animationNames: [], error: err, mode, durationMs },
      };
    }

    showFallbackSurface(deps.canvasWrap, deps.fallback, deps.fallback.src);
    deps.hideBootHint?.();
    return {
      session: null,
      result: { ok: false, animationNames: [], error: err, mode, durationMs },
    };
  }
}
