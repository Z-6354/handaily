import { slugifyPersonaId } from "./scraper.js";
import type { HandailyExport, ShipRecord } from "./types.js";

export function exportForHandaily(record: ShipRecord): HandailyExport {
  const characterInfo: Record<string, string> = {};
  for (const f of record.characterInfo) characterInfo[f.field] = f.value;

  const sections: Record<string, string> = {};
  for (const s of record.sections) sections[s.id] = s.text;

  const sampleLines = record.lines
    .filter((l) => l.lang === "zh" && l.text.length >= 4)
    .slice(0, 12)
    .map((l) => l.text);

  return {
    wikiTitle: record.wikiTitle,
    wikiUrl: record.wikiUrl,
    name: record.displayName,
    source: "碧蓝航线",
    personaReference: record.personaReference,
    sampleLines,
    characterInfo,
    sections,
    assets: record.assets,
    suggestedPersonaId: slugifyPersonaId(record.displayName),
  };
}

export function handailyImportGuide(exportData: HandailyExport): string {
  return [
    "## HANDAILY 导入指引（开发者）",
    "",
    "1. 在小寒日报 **人物 → 性格** 中使用「Wiki 导入」或「文本导入」。",
    "2. 将下方 `personaReference` 粘贴为参考文本，或使用 Wiki URL：",
    `   ${exportData.wikiUrl}`,
    "3. 建议 persona id：`" + exportData.suggestedPersonaId + "`",
    "4. 桌宠皮肤/Spine 模型需另行导入；本 MCP 主要提供性格与台词资料。",
    "5. 台词 JSON 可用 `lines` 字段批量写入桌宠 remark lines。",
    "",
    exportData.personaReference,
  ].join("\n");
}
