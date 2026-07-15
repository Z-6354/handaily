/**
 * Run: npx tsx scripts/test-tabcontainer-skins.ts
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  bindLinesToSkinSlots,
  extractIllustrationSkins,
} from "../src/scraper.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const html = readFileSync(
  path.join(here, "../fixtures/tabcontainer-cheshire.html"),
  "utf8"
);
const skins = extractIllustrationSkins(html);
const keys = skins.map((s) => s.key);
if (keys.includes("retrofit") || skins.some((s) => s.label.includes("改造"))) {
  console.error("改造 should be excluded", keys);
  process.exit(1);
}
if (!keys.includes("default") || !keys.includes("skin1") || !keys.includes("oath")) {
  console.error("expected default/skin1/oath", keys);
  process.exit(1);
}
if (skins.length !== 6) {
  console.error("expected 6 skins (通常+4换装+誓约), got", skins.length, keys);
  process.exit(1);
}
const bound = bindLinesToSkinSlots(skins, [
  { skin: "default", skin_kind: "default", lines: [{ key: "login", label: null, lang: "zh", text: "默认台", audioUrl: null }] },
  { skin: "某换装", skin_kind: "skin", lines: [{ key: "login", label: null, lang: "zh", text: "换1", audioUrl: null }] },
  { skin: "改造台词", skin_kind: "retrofit", lines: [{ key: "login", label: null, lang: "zh", text: "不该出现", audioUrl: null }] },
  { skin: "誓约名", skin_kind: "oath", lines: [{ key: "login", label: null, lang: "zh", text: "誓约台", audioUrl: null }] },
]);
if (bound.some((g) => g.lines.some((l) => l.text === "不该出现"))) {
  console.error("retrofit lines leaked");
  process.exit(1);
}
const def = bound.find((g) => g.slot_key === "default");
const oath = bound.find((g) => g.slot_key === "oath");
if (def?.lines[0]?.text !== "默认台" || oath?.lines[0]?.text !== "誓约台") {
  console.error("bind failed", bound);
  process.exit(1);
}
console.log("ok", keys.join(","));
