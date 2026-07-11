import type { PetAssetConfig } from "../spinePet";

export interface PetRemarkLine {
  text: string;
  animation?: string | null;
}

export interface PetConfigPayload {
  model_id: string;
  model_name: string;
  asset_base: string;
  config_file?: string | null;
  skel_file: string;
  atlas_file: string;
  png_file: string;
  use_file_src: boolean;
  power_mode: string;
  scale: number;
  animations: string[];
  idle_animation?: string | null;
  click_animation?: string | null;
  boot_animation?: string | null;
  return_idle_animation?: string | null;
  drag_animation?: string | null;
  random_animations: string[];
  random_min_sec: number;
  random_max_sec: number;
  lines: PetRemarkLine[];
  window_width: number;
  window_height: number;
  offset_x: number;
  offset_y: number;
  bubble_enabled: boolean;
}

export interface PetAnimationMeta {
  animations: string[];
  idle_animation?: string | null;
  click_animation?: string | null;
  boot_animation?: string | null;
  return_idle_animation?: string | null;
  drag_animation?: string | null;
  random_animations: string[];
  random_min_sec: number;
  random_max_sec: number;
  lines: PetRemarkLine[];
}

export type SpineInitMode = "cold" | "hot" | "teardown";

export interface SpineInitOptions {
  mode: SpineInitMode;
  skipVisibilityWait?: boolean;
}

export interface SpineLoadResult {
  ok: boolean;
  animationNames: string[];
  error?: unknown;
  mode: SpineInitMode;
  durationMs: number;
}

export function assetConfigFromPayload(cfg: PetConfigPayload): PetAssetConfig {
  const base = cfg.asset_base.endsWith("/") ? cfg.asset_base : `${cfg.asset_base}/`;
  return {
    pathPrefix: base,
    configFile: cfg.config_file ?? null,
    skelFile: cfg.skel_file,
    atlasFile: cfg.atlas_file,
    pngFile: cfg.png_file,
  };
}

export function modelAssetFilenames(cfg: PetConfigPayload): string[] {
  const files = [cfg.skel_file, cfg.atlas_file];
  if (cfg.config_file) files.push(cfg.config_file);
  return files.filter(Boolean);
}
