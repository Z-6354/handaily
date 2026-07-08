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

/** 卡片双标签：皮肤 + AI 性格摘要（每排两个） */
export function characterCardTags(
  c: Pick<CharacterBrief, "skin_count" | "active_skin_name" | "trait_summary">
): [string, string] {
  const skin =
    c.skin_count > 1
      ? `${c.skin_count} 套皮肤`
      : c.active_skin_name && c.active_skin_name !== "默认"
        ? c.active_skin_name
        : "默认皮肤";
  const trait = c.trait_summary.trim() || "性格待生成";
  return [skin, trait];
}
