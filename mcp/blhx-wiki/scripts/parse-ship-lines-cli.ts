import { readFileSync } from "node:fs";
import {
  bindLinesToSkinSlots,
  extractIllustrationSkins,
  extractShipLinesBySkin,
  flattenShipLineGroups,
} from "../src/scraper.js";

const file = process.argv[2];
if (!file) {
  console.error("usage: parse-ship-lines-cli.ts <html-file>");
  process.exit(2);
}
const html = readFileSync(file, "utf8");
const skins = extractIllustrationSkins(html);
const groups = bindLinesToSkinSlots(skins, extractShipLinesBySkin(html));
const lines = flattenShipLineGroups(groups);
process.stdout.write(JSON.stringify({ skins, groups, lines }));
