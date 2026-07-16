/** 与 viewerExApply / models.rs 保持一致的待机动作名判定 */

export function isLikelyIdleName(name: string): boolean {
  const lower = name.toLowerCase();
  if (["normal", "stand", "idle", "standby", "default"].includes(lower)) return true;
  return ["idle", "stand", "normal"].some((k) => lower.includes(k));
}

export function pickIdleAnimation(names: string[]): string | null {
  if (names.length === 0) return null;
  for (const pref of ["normal", "stand", "idle", "standby", "default"]) {
    const hit = names.find((n) => n.toLowerCase() === pref);
    if (hit) return hit;
  }
  for (const key of ["idle", "stand", "normal"]) {
    const hit = names.find((n) => n.toLowerCase().includes(key));
    if (hit) return hit;
  }
  return names[0];
}

export function pickClickAnimation(names: string[]): string | null {
  for (const key of ["touch", "tap", "click", "hit"]) {
    const hit = names.find((n) => n.toLowerCase().includes(key));
    if (hit) return hit;
  }
  return null;
}

/** 适合作为 overlay 一次性动作（非待机循环） */
export function isOverlayActionName(name: string): boolean {
  return !isLikelyIdleName(name);
}
