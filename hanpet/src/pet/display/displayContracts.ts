import type { SpinePet } from "../spinePet";
import type { PetAnimationMeta, PetConfigPayload, SpineInitMode } from "./types";

export type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

export interface PetDisplayHost {
  loadConfig(): Promise<PetConfigPayload>;
  refreshScreenBounds(): Promise<void>;
  resolveWindowSize(
    cfg: PetConfigPayload,
    mode: SpineInitMode,
  ): Promise<{ w: number; h: number }>;
  applyWindowSize(w: number, h: number): Promise<void>;
  applyLayoutFromConfig(cfg: PetConfigPayload): void;
  applyCanvasDisplaySize(): void;
  shouldExitEditBeforeReload(): Promise<void>;
  isAppExiting(): boolean;
  syncAnimations(
    modelId: string,
    names: string[],
    idle?: string | null,
  ): Promise<PetAnimationMeta | null>;
  getPendingPreview(): { animation: string; loop: boolean } | null;
  clearPendingPreview(): void;
  runPreviewAnimation(animation: string, loop: boolean): void;
}

export interface PetDisplayDom {
  canvas: HTMLCanvasElement;
  canvasWrap: HTMLElement;
  fallback: HTMLImageElement;
  getDisplaySize(): { w: number; h: number };
  setDisplaySize(w: number, h: number): void;
  getFallbackSrc(): string;
  setFallbackSrc(url: string): void;
  showBootHint(): void;
  hideBootHint(): void;
  clearLoadError(): void;
  replaceCanvas(next: HTMLCanvasElement): void;
  pickLine: (
    lines: PetConfigPayload["lines"],
    animation: string,
  ) => string | null;
  showBubble: (text: string, animation?: string) => void;
}

export type SpineLoaderPickLine = PetDisplayDom["pickLine"];

export interface PetDisplayRuntime {
  getSpine(): SpinePet | null;
  setSpine(spine: SpinePet | null): void;
}
