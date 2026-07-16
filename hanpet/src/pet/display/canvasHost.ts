import { petLog } from "../log/petDebugLog";

export function releaseCanvasGlContext(canvas: HTMLCanvasElement) {
  try {
    const gl =
      (canvas.getContext("webgl2") ??
        canvas.getContext("webgl") ??
        canvas.getContext("experimental-webgl")) as WebGLRenderingContext | null;
    gl?.getExtension("WEBGL_lose_context")?.loseContext();
    petLog("debug", "canvas", "released GL context");
  } catch (err) {
    petLog("warn", "canvas", "release GL failed", { err: String(err) });
  }
}

export function ensureCanvasAttached(canvas: HTMLCanvasElement, wrap: HTMLElement) {
  if (!wrap.contains(canvas)) {
    wrap.append(canvas);
  }
}

/** 切换模型时替换 canvas 节点，避免 loseContext 后 WebGL 无法重建（Pixi shader 0 错误） */
export function replaceCanvasElement(
  wrap: HTMLElement,
  prev: HTMLCanvasElement,
  displayW: number,
  displayH: number,
): HTMLCanvasElement {
  const next = document.createElement("canvas");
  next.className = prev.className;
  if (prev.id) next.id = prev.id;
  wrap.replaceChild(next, prev);
  next.width = displayW;
  next.height = displayH;
  petLog("debug", "canvas", "replaced canvas element", { w: displayW, h: displayH });
  return next;
}

export function awaitAnimationFrames(count: number): Promise<void> {
  return new Promise((resolve) => {
    const step = (left: number) => {
      if (left <= 0) {
        resolve();
        return;
      }
      requestAnimationFrame(() => step(left - 1));
    };
    step(count);
  });
}

export interface CanvasPrepOptions {
  mode: "cold" | "hot" | "teardown";
  hadPet: boolean;
  displayW: number;
  displayH: number;
  showBootHint?: () => void;
}

export async function prepareCanvasSurface(
  canvas: HTMLCanvasElement,
  wrap: HTMLElement,
  opts: CanvasPrepOptions,
): Promise<HTMLCanvasElement> {
  const { mode, hadPet, displayW, displayH, showBootHint } = opts;
  ensureCanvasAttached(canvas, wrap);

  if (mode === "hot") {
    await awaitAnimationFrames(1);
    if (canvas.width !== displayW || canvas.height !== displayH) {
      canvas.width = displayW;
      canvas.height = displayH;
    }
    petLog("debug", "canvas", "hot prep", { w: displayW, h: displayH });
    return canvas;
  }

  if (mode === "cold" && !hadPet) {
    showBootHint?.();
    wrap.style.visibility = "hidden";
    if (canvas.width !== displayW || canvas.height !== displayH) {
      canvas.width = displayW;
      canvas.height = displayH;
    }
    petLog("debug", "canvas", "cold boot prep", { w: displayW, h: displayH });
    return canvas;
  }

  if (mode === "teardown" || (mode === "cold" && hadPet)) {
    showBootHint?.();
    wrap.style.visibility = "hidden";
    const next = replaceCanvasElement(wrap, canvas, displayW, displayH);
    await awaitAnimationFrames(2);
    petLog("debug", "canvas", mode === "cold" ? "cold reset prep" : "teardown prep", {
      w: displayW,
      h: displayH,
    });
    return next;
  }

  return canvas;
}

export function showSpineSurface(
  wrap: HTMLElement,
  fallback: HTMLImageElement,
  canvasBlock = true,
) {
  wrap.style.display = canvasBlock ? "block" : "none";
  wrap.style.visibility = "visible";
  fallback.style.display = "none";
}

export function showFallbackSurface(
  wrap: HTMLElement,
  fallback: HTMLImageElement,
  src: string,
) {
  fallback.src = src;
  wrap.style.visibility = "visible";
  wrap.style.display = "none";
  fallback.style.display = "block";
  petLog("warn", "canvas", "showing static fallback");
}
