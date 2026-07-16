import { readFileSync } from "node:fs";
import Database from "better-sqlite3";
import { extractShipLinesBySkin, flattenShipLineGroups } from "../src/scraper.js";

const html = readFileSync(new URL("../_sample_ayanami.html", import.meta.url), "utf8");
const groups = extractShipLinesBySkin(html);
const flat = flattenShipLineGroups(groups);
const db = new Database(new URL("../data/blhx.sqlite", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1"));
const cols = db.prepare("PRAGMA table_info(ships)").all().map((r: { name: string }) => r.name);
if (!cols.includes("lines_by_skin_json")) {
  db.exec("ALTER TABLE ships ADD COLUMN lines_by_skin_json TEXT NOT NULL DEFAULT '[]'");
}
db.prepare(
  "UPDATE ships SET lines_by_skin_json=?, lines_json=? WHERE wiki_title=? OR display_name=?"
).run(JSON.stringify(groups), JSON.stringify(flat), "绫波", "绫波");
const row = db
  .prepare("select length(lines_by_skin_json) as n from ships where display_name=?")
  .get("绫波") as { n: number } | undefined;
console.log("updated groups", groups.length, "bytes", row?.n);
db.close();
