import { match } from "pinyin-pro";
import type { CharacterBrief } from "./xiaohan";

type RosterItem = Pick<
  CharacterBrief,
  | "id"
  | "name"
  | "source"
  | "description"
  | "persona_id"
  | "faction"
  | "ship_type"
  | "rarity"
>;

/** 人物 roster 搜索：名称/来源/简介/ID + 标签 + 拼音全拼与首字母 */
export function characterMatchesQuery(c: RosterItem, query: string): boolean {
  const q = query.trim();
  if (!q) return true;

  const lower = q.toLowerCase();
  const fields = [
    c.name,
    c.source,
    c.description,
    c.id,
    c.persona_id,
    c.faction,
    c.ship_type,
    c.rarity,
  ];
  if (fields.some((f) => f && f.toLowerCase().includes(lower))) {
    return true;
  }

  if (c.name) {
    const py = match(c.name, q, { precision: "every", continuous: true });
    if (py && py.length > 0) return true;
    const pyLoose = match(c.name, q, { precision: "start" });
    if (pyLoose && pyLoose.length > 0) return true;
  }

  return false;
}
