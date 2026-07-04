import type { Spine } from "@pixi-spine/runtime-3.8";

import type { ViewerExSpineConfig } from "./viewerExConfig";

function num(v: unknown): number | null {
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "string" && v.trim()) {
    const n = Number(v);
    if (Number.isFinite(n)) return n;
  }
  return null;
}

function str(v: unknown): string | null {
  if (typeof v !== "string") return null;
  const s = v.trim();
  return s || null;
}

function motionAnimationName(entry: unknown): string | null {
  if (!entry || typeof entry !== "object") return null;
  const o = entry as Record<string, unknown>;
  return (
    str(o.animation) ??
    str(o.Animation) ??
    str(o.file) ??
    str(o.File) ??
    str(o.name) ??
    str(o.Name)
  );
}

export {
  isLikelyIdleName,
  pickIdleAnimation,
  pickClickAnimation,
  isOverlayActionName,
} from "../lib/petAnimationNames";

export function pickStartAnimation(cfg: ViewerExSpineConfig): string | null {
  const motions = cfg.motions;
  if (!motions || typeof motions !== "object") return null;
  const m = motions as Record<string, unknown>;
  for (const key of ["start", "idle", "Start", "Idle"]) {
    const group = m[key];
    if (!Array.isArray(group) || group.length === 0) continue;
    const name = motionAnimationName(group[0]);
    if (name) return name;
  }
  return null;
}

export function applyViewerExBindings(
  spine: Spine,
  cfg: ViewerExSpineConfig,
  opts?: { setupSlots?: boolean },
) {
  const setupSlots = opts?.setupSlots ?? true;
  const sk = spine.skeleton;

  const skinName =
    str(cfg.skin) ??
    str(cfg.default_skin) ??
    (Array.isArray(cfg.skins) ? str(cfg.skins[0]) : null);
  if (skinName) {
    const skin = sk.data.findSkin(skinName);
    if (skin) {
      sk.setSkin(skin);
      if (setupSlots) sk.setSlotsToSetupPose();
    }
  } else if (sk.data.defaultSkin) {
    sk.setSkin(sk.data.defaultSkin);
    if (setupSlots) sk.setSlotsToSetupPose();
  }

  const bones = cfg.bones;
  if (Array.isArray(bones)) {
    for (const raw of bones) {
      if (!raw || typeof raw !== "object") continue;
      const b = raw as Record<string, unknown>;
      const name = str(b.name) ?? str(b.Name);
      if (!name) continue;
      const bone = sk.findBone(name);
      if (!bone) continue;
      const x = num(b.x ?? b.X);
      const y = num(b.y ?? b.Y);
      const rotation = num(b.rotation ?? b.Rotation);
      const scaleX = num(b.scale_x ?? b.scaleX ?? b.ScaleX);
      const scaleY = num(b.scale_y ?? b.scaleY ?? b.ScaleY);
      if (x != null) bone.x = x;
      if (y != null) bone.y = y;
      if (rotation != null) bone.rotation = rotation;
      if (scaleX != null) bone.scaleX = scaleX;
      if (scaleY != null) bone.scaleY = scaleY;
    }
    sk.updateWorldTransform();
  }

  const configOpts = cfg.options;
  if (configOpts && typeof configOpts === "object") {
    const o = configOpts as Record<string, unknown>;
    const scale = num(o.scale_factor ?? o.scale ?? o.ScaleFactor);
    if (scale != null && scale > 0) {
      spine.scale.set(spine.scale.x * scale, spine.scale.y * scale);
    }
    const px = num(o.position_x ?? o.PositionX);
    const py = num(o.position_y ?? o.PositionY);
    if (px != null) spine.x += px;
    if (py != null) spine.y += py;
  }
}

export function applySkinSetupPose(spine: Spine) {
  spine.skeleton.setSlotsToSetupPose();
}

export function pumpAnimationState(spine: Spine, seconds: number, steps = 24) {
  if (seconds <= 0 || steps <= 0) return;
  const dt = seconds / steps;
  for (let i = 0; i < steps; i++) {
    spine.state.update(dt);
    spine.state.apply(spine.skeleton);
  }
  spine.skeleton.updateWorldTransform();
}
