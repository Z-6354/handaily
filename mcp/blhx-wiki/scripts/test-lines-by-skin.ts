/**
 * Quick unit smoke for extractShipLinesBySkin (run: npx tsx scripts/test-lines-by-skin.ts)
 */
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { extractShipLinesBySkin } from "../src/scraper.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const sample = path.join(here, "..", "_sample_ayanami.html");
if (!existsSync(sample)) {
  console.error("missing sample HTML; download 绫波 page to _sample_ayanami.html");
  process.exit(1);
}
const html = readFileSync(sample, "utf8");
const groups = extractShipLinesBySkin(html);
if (groups.length < 2) {
  console.error("expected multiple skin groups, got", groups.length);
  process.exit(1);
}
const def = groups.find((g) => g.skin === "default");
if (!def || def.lines.length < 5) {
  console.error("default group missing/thin", def?.lines.length);
  process.exit(1);
}
const skin = groups.find((g) => g.skin_kind === "skin" && g.lines.length > 0);
if (!skin) {
  console.error("no skin group");
  process.exit(1);
}
const dLogin = def.lines.find((l) => l.key === "login")?.text;
const sLogin = skin.lines.find((l) => l.key === "login")?.text;
if (dLogin && sLogin && dLogin === sLogin) {
  console.error("default and skin login identical — grouping may be wrong");
  process.exit(1);
}
console.log(
  "ok",
  groups.length,
  "groups; default lines",
  def.lines.length,
  "sample skin",
  skin.skin,
  skin.lines.length
);
