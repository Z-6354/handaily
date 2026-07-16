import type { SpinePet } from "../spinePet";
import type { PetAssetResolver } from "../petAssetResolver";
import { petLog } from "../log/petDebugLog";
import { disposeAssetResolver, prepareModelAssets } from "./assetPipeline";
import { loadSpineSession, type SpineLoaderDeps } from "./spineLoader";
import type { ReloadCommand } from "./reloadCommand";
import type { PetDisplayDom, PetDisplayHost, InvokeFn } from "./displayContracts";
import type { PetConfigPayload, SpineInitMode, SpineLoadResult } from "./types";

export interface ReloadPipelineContext {
  command: ReloadCommand;
  mode: SpineInitMode;
  trace: string;
  cfg: PetConfigPayload;
  dom: PetDisplayDom;
  host: PetDisplayHost;
  invoke: InvokeFn;
  priorSpine: SpinePet | null;
  getSpine: () => SpinePet | null;
  setSpine: (spine: SpinePet | null) => void;
  getResolver: () => PetAssetResolver | null;
  setResolver: (resolver: PetAssetResolver | null) => void;
  lastFallbackSrc: string;
  setLastFallbackSrc: (url: string) => void;
  setLines: (lines: PetConfigPayload["lines"]) => void;
  onPhaseComplete?: (phase: string, ms: number) => void;
  w: number;
  h: number;
  result: SpineLoadResult | null;
}

export interface ReloadPipelinePhase {
  readonly name: string;
  run(ctx: ReloadPipelineContext): Promise<boolean>;
}

export const resolveWindowPhase: ReloadPipelinePhase = {
  name: "window",
  async run(ctx) {
    const { w, h } = await ctx.host.resolveWindowSize(ctx.cfg, ctx.mode);
    ctx.w = w;
    ctx.h = h;
    const size = ctx.dom.getDisplaySize();
    if (w !== size.w || h !== size.h) {
      await ctx.host.applyWindowSize(w, h);
      ctx.dom.setDisplaySize(w, h);
    }
    return true;
  },
};

export const assetPhase: ReloadPipelinePhase = {
  name: "asset",
  async run(ctx) {
    disposeAssetResolver(ctx.getResolver());
    ctx.setResolver(null);

    const { resolver, fallbackUrl } = await prepareModelAssets(
      ctx.cfg,
      ctx.mode === "hot" ? "hot" : "cold",
    );
    ctx.setResolver(resolver);
    ctx.setLastFallbackSrc(fallbackUrl);
    ctx.dom.setFallbackSrc(fallbackUrl);
    return true;
  },
};

export const layoutPhase: ReloadPipelinePhase = {
  name: "layout",
  async run(ctx) {
    ctx.host.applyLayoutFromConfig(ctx.cfg);
    return true;
  },
};

export const spinePhase: ReloadPipelinePhase = {
  name: "spine",
  async run(ctx) {
    const loaderDeps: SpineLoaderDeps = {
      canvas: ctx.dom.canvas,
      canvasWrap: ctx.dom.canvasWrap,
      fallback: ctx.dom.fallback,
      displayW: ctx.w,
      displayH: ctx.h,
      pickLine: ctx.dom.pickLine,
      showBubble: ctx.dom.showBubble,
      showBootHint: () => ctx.dom.showBootHint(),
      hideBootHint: () => ctx.dom.hideBootHint(),
      onCanvasReplaced: (next) => ctx.dom.replaceCanvas(next),
    };

    const resolver = ctx.getResolver();
    if (!resolver) {
      ctx.result = {
        ok: false,
        animationNames: [],
        error: new Error("missing asset resolver"),
        mode: ctx.mode,
        durationMs: 0,
      };
      return false;
    }

    const { session, result } = await loadSpineSession(
      ctx.cfg,
      resolver,
      ctx.priorSpine,
      ctx.mode,
      loaderDeps,
    );
    ctx.result = result;

    if (!result.ok || !session) {
      ctx.setSpine(null);
      if (ctx.mode === "hot") {
        petLog("warn", "display", "hot load failed, will retry teardown", {
          trace: ctx.trace,
        });
      }
      return false;
    }

    ctx.setSpine(session.pet);
    ctx.setLines(session.lines);
    ctx.host.applyCanvasDisplaySize();
    return true;
  },
};

export const defaultLoadPhases: ReloadPipelinePhase[] = [
  resolveWindowPhase,
  assetPhase,
  layoutPhase,
  spinePhase,
];

export async function runLoadPipeline(
  ctx: ReloadPipelineContext,
  phases: ReloadPipelinePhase[] = defaultLoadPhases,
): Promise<boolean> {
  for (const phase of phases) {
    const t0 = performance.now();
    const ok = await phase.run(ctx);
    const ms = Math.round(performance.now() - t0);
    ctx.onPhaseComplete?.(phase.name, ms);
    if (!ok) return false;
  }
  return true;
}

export function createPipelineContext(
  partial: Omit<
    ReloadPipelineContext,
    "w" | "h" | "result" | "lastFallbackSrc"
  > & { lastFallbackSrc: string },
): ReloadPipelineContext {
  return {
    ...partial,
    w: 0,
    h: 0,
    result: null,
  };
}
