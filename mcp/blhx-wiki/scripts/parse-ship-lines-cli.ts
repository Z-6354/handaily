/**
 * CLI: parse wiki ship HTML → lines_by_skin JSON on stdout
 * Usage: tsx scripts/parse-ship-lines-cli.ts <html-file>
 */
import { readFileSync } from "node:fs";
import {
  extractShipLinesBySkin,
  flattenShipLineGroups,
} from "../src/scraper.js";

const file = process.argv[2];
if (!file) {
  console.error("usage: parse-ship-lines-cli.ts <html-file>");
  process.exit(2);
}
const html = readFileSync(file, "utf8");
const groups = extractShipLinesBySkin(html);
const lines = flattenShipLineGroups(groups);
process.stdout.write(JSON.stringify({ groups, lines }));
