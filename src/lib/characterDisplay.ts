import type { CharacterBrief } from "./xiaohan";

export const CHARACTER_ACCENT: Record<string, string> = {
  cheshire: "#f59e0b",
  edu: "#8b5cf6",
  wushiling: "#06b6d4",
  qiye: "#64748b",
  tashigan: "#3b82f6",
};

export function characterAccent(id: string): string {
  return CHARACTER_ACCENT[id] ?? "#722ed1";
}

export function characterInitial(name: string): string {
  const t = name.trim();
  return t ? t.charAt(0) : "?";
}

/** 卡片标签：仅显示皮肤数量 */
export function characterSkinTag(
  c: Pick<CharacterBrief, "skin_count">
): string {
  const n = Math.max(0, c.skin_count);
  return n > 0 ? `${n} 套皮肤` : "暂无皮肤";
}
