import { BlhxDatabase } from "../dist/db.js";
import { fetchCatalog, fetchShipRecord } from "../dist/wiki.js";

const db = new BlhxDatabase();
const started = Date.now();

function log(msg) {
  const elapsed = ((Date.now() - started) / 1000).toFixed(0);
  console.log(`[${elapsed}s] ${msg}`);
}

log("sync catalog...");
const entries = await fetchCatalog();
const upsert = db.upsertCatalog(entries);
log(`catalog: ${entries.length} entries (inserted ${upsert.inserted}, updated ${upsert.updated})`);

let ok = 0;
let fail = 0;
const errors = [];

while (true) {
  const pending = db.listPendingTitles(9999);
  if (pending.length === 0) break;

  log(`pending ${pending.length}, fetching...`);
  for (const entry of pending) {
    const name = entry.wikiTitle;
    try {
      const record = await fetchShipRecord(name, entry);
      db.saveShip(record);
      ok++;
      if (ok % 10 === 0 || ok === 1) {
        const stats = db.stats();
        log(`progress ${stats.fetchedTotal}/${stats.catalogTotal} (ok=${ok}, fail=${fail}) latest=${name}`);
      }
    } catch (e) {
      fail++;
      const msg = e instanceof Error ? e.message : String(e);
      errors.push({ name, error: msg });
      log(`FAIL ${name}: ${msg}`);
    }
  }
}

const stats = db.stats();
log(`DONE fetched=${stats.fetchedTotal}/${stats.catalogTotal} ok=${ok} fail=${fail}`);
if (errors.length) {
  log(`errors (${errors.length}):`);
  for (const e of errors.slice(0, 20)) log(`  - ${e.name}: ${e.error}`);
  if (errors.length > 20) log(`  ... and ${errors.length - 20} more`);
}

db.close();
