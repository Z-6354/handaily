import { BlhxDatabase } from "../dist/db.js";
import { exportForHandaily } from "../dist/handaily.js";
import { fetchCatalog, fetchShipRecord } from "../dist/wiki.js";

const db = new BlhxDatabase();

console.log("sync catalog...");
const entries = await fetchCatalog();
console.log("catalog entries", entries.length);
const upsert = db.upsertCatalog(entries);
console.log("upsert", upsert);

console.log("fetch 欧根亲王...");
const record = await fetchShipRecord("欧根亲王", db.getCatalogEntry("欧根亲王") ?? undefined);
db.saveShip(record);
console.log({
  name: record.displayName,
  lines: record.lines.length,
  assets: record.assets.length,
  sections: record.sections.map((s) => s.id),
  cv: record.cv,
});

const exported = exportForHandaily(record);
console.log("export sample lines", exported.sampleLines.slice(0, 3));
console.log("stats", db.stats());
db.close();
