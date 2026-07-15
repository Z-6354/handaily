import Database from "better-sqlite3";
import fs from "node:fs";
import path from "node:path";
import type {
  CatalogEntry,
  ShipAsset,
  ShipLine,
  ShipLineGroup,
  ShipRecord,
  ShipSection,
  SyncStats,
} from "./types.js";
import { repoRoot } from "./repoRoot.js";

function defaultDbPath(): string {
  const env = process.env.BLHX_WIKI_DB_PATH?.trim();
  if (env) return path.resolve(env);
  const root = repoRoot();
  for (const rel of ["data/wiki/blhx.sqlite", "mcp/blhx-wiki/data/blhx.sqlite"]) {
    const candidate = path.join(root, rel);
    if (fs.existsSync(candidate)) return candidate;
  }
  return path.join(root, "data/wiki/blhx.sqlite");
}

const DEFAULT_DB = defaultDbPath();

export class BlhxDatabase {
  private db: Database.Database;

  constructor(dbPath = process.env.BLHX_WIKI_DB_PATH ?? DEFAULT_DB) {
    fs.mkdirSync(path.dirname(dbPath), { recursive: true });
    this.db = new Database(dbPath);
    this.db.pragma("journal_mode = WAL");
    this.initSchema();
  }

  private initSchema(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS catalog (
        wiki_title TEXT PRIMARY KEY,
        wiki_path TEXT NOT NULL,
        display_name TEXT NOT NULL,
        aliases_json TEXT NOT NULL DEFAULT '[]',
        avatar_url TEXT,
        rarity TEXT,
        faction TEXT,
        ship_type TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );

      CREATE TABLE IF NOT EXISTS ships (
        wiki_title TEXT PRIMARY KEY,
        wiki_url TEXT NOT NULL,
        display_name TEXT NOT NULL,
        aliases_json TEXT NOT NULL DEFAULT '[]',
        rarity TEXT,
        faction TEXT,
        ship_type TEXT,
        cv TEXT,
        character_info_json TEXT NOT NULL DEFAULT '[]',
        sections_json TEXT NOT NULL DEFAULT '[]',
        lines_json TEXT NOT NULL DEFAULT '[]',
        assets_json TEXT NOT NULL DEFAULT '[]',
        persona_reference TEXT NOT NULL DEFAULT '',
        html_hash TEXT NOT NULL DEFAULT '',
        fetched_at TEXT NOT NULL
      );

      CREATE TABLE IF NOT EXISTS sync_meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
      );

      CREATE INDEX IF NOT EXISTS idx_catalog_display ON catalog(display_name);
      CREATE INDEX IF NOT EXISTS idx_ships_display ON ships(display_name);

      CREATE TABLE IF NOT EXISTS live2d_mappings (
        folder TEXT PRIMARY KEY,
        wiki_title TEXT NOT NULL,
        display_name TEXT NOT NULL,
        skin_label TEXT NOT NULL DEFAULT '默认',
        score REAL NOT NULL DEFAULT 0,
        character_id TEXT,
        model_id TEXT,
        imported_at TEXT,
        updated_at TEXT NOT NULL
      );
    `);
    this.ensureColumn("ships", "lines_by_skin_json", "TEXT NOT NULL DEFAULT '[]'");
  }

  private ensureColumn(table: string, column: string, decl: string): void {
    const rows = this.db.prepare(`PRAGMA table_info(${table})`).all() as Array<{
      name: string;
    }>;
    if (rows.some((r) => r.name === column)) return;
    this.db.exec(`ALTER TABLE ${table} ADD COLUMN ${column} ${decl}`);
  }

  listAllCatalogTitles(): string[] {
    const rows = this.db
      .prepare("SELECT wiki_title FROM catalog ORDER BY display_name")
      .all() as Array<{ wiki_title: string }>;
    return rows.map((r) => r.wiki_title);
  }

  saveLive2dMapping(entry: {
    folder: string;
    wikiTitle: string;
    displayName: string;
    skinLabel: string;
    score: number;
    characterId?: string;
    modelId?: string;
    importedAt?: string;
  }): void {
    const now = new Date().toISOString();
    this.db
      .prepare(
        `
      INSERT INTO live2d_mappings (
        folder, wiki_title, display_name, skin_label, score,
        character_id, model_id, imported_at, updated_at
      ) VALUES (
        @folder, @wiki_title, @display_name, @skin_label, @score,
        @character_id, @model_id, @imported_at, @updated_at
      )
      ON CONFLICT(folder) DO UPDATE SET
        wiki_title = excluded.wiki_title,
        display_name = excluded.display_name,
        skin_label = excluded.skin_label,
        score = excluded.score,
        character_id = COALESCE(excluded.character_id, live2d_mappings.character_id),
        model_id = COALESCE(excluded.model_id, live2d_mappings.model_id),
        imported_at = COALESCE(excluded.imported_at, live2d_mappings.imported_at),
        updated_at = excluded.updated_at
    `
      )
      .run({
        folder: entry.folder,
        wiki_title: entry.wikiTitle,
        display_name: entry.displayName,
        skin_label: entry.skinLabel,
        score: entry.score,
        character_id: entry.characterId ?? null,
        model_id: entry.modelId ?? null,
        imported_at: entry.importedAt ?? null,
        updated_at: now,
      });
  }

  listLive2dMappings(limit = 500, offset = 0): Array<Record<string, unknown>> {
    return this.db
      .prepare(
        "SELECT * FROM live2d_mappings ORDER BY folder LIMIT @limit OFFSET @offset"
      )
      .all({ limit, offset }) as Array<Record<string, unknown>>;
  }

  upsertCatalog(entries: CatalogEntry[]): { inserted: number; updated: number } {
    const now = new Date().toISOString();
    const stmt = this.db.prepare(`
      INSERT INTO catalog (
        wiki_title, wiki_path, display_name, aliases_json, avatar_url,
        rarity, faction, ship_type, created_at, updated_at
      ) VALUES (
        @wiki_title, @wiki_path, @display_name, @aliases_json, @avatar_url,
        @rarity, @faction, @ship_type, @created_at, @updated_at
      )
      ON CONFLICT(wiki_title) DO UPDATE SET
        wiki_path = excluded.wiki_path,
        display_name = excluded.display_name,
        aliases_json = excluded.aliases_json,
        avatar_url = excluded.avatar_url,
        rarity = excluded.rarity,
        faction = excluded.faction,
        ship_type = excluded.ship_type,
        updated_at = excluded.updated_at
    `);

    let inserted = 0;
    let updated = 0;
    const tx = this.db.transaction(() => {
      for (const e of entries) {
        const exists = this.db
          .prepare("SELECT 1 FROM catalog WHERE wiki_title = ?")
          .get(e.wikiTitle);
        stmt.run({
          wiki_title: e.wikiTitle,
          wiki_path: e.wikiPath,
          display_name: e.displayName,
          aliases_json: JSON.stringify(e.aliases),
          avatar_url: e.avatarUrl,
          rarity: e.rarity,
          faction: e.faction,
          ship_type: e.shipType,
          created_at: now,
          updated_at: now,
        });
        if (exists) updated++;
        else inserted++;
      }
      this.setMeta("last_catalog_sync", now);
    });
    tx();
    return { inserted, updated };
  }

  saveShip(record: ShipRecord): void {
    this.db
      .prepare(
        `
      INSERT INTO ships (
        wiki_title, wiki_url, display_name, aliases_json, rarity, faction, ship_type,
        cv, character_info_json, sections_json, lines_json, lines_by_skin_json, assets_json,
        persona_reference, html_hash, fetched_at
      ) VALUES (
        @wiki_title, @wiki_url, @display_name, @aliases_json, @rarity, @faction, @ship_type,
        @cv, @character_info_json, @sections_json, @lines_json, @lines_by_skin_json, @assets_json,
        @persona_reference, @html_hash, @fetched_at
      )
      ON CONFLICT(wiki_title) DO UPDATE SET
        wiki_url = excluded.wiki_url,
        display_name = excluded.display_name,
        aliases_json = excluded.aliases_json,
        rarity = excluded.rarity,
        faction = excluded.faction,
        ship_type = excluded.ship_type,
        cv = excluded.cv,
        character_info_json = excluded.character_info_json,
        sections_json = excluded.sections_json,
        lines_json = excluded.lines_json,
        lines_by_skin_json = excluded.lines_by_skin_json,
        assets_json = excluded.assets_json,
        persona_reference = excluded.persona_reference,
        html_hash = excluded.html_hash,
        fetched_at = excluded.fetched_at
    `
      )
      .run({
        wiki_title: record.wikiTitle,
        wiki_url: record.wikiUrl,
        display_name: record.displayName,
        aliases_json: JSON.stringify(record.aliases),
        rarity: record.rarity,
        faction: record.faction,
        ship_type: record.shipType,
        cv: record.cv,
        character_info_json: JSON.stringify(record.characterInfo),
        sections_json: JSON.stringify(record.sections),
        lines_json: JSON.stringify(record.lines),
        lines_by_skin_json: JSON.stringify(record.linesBySkin ?? []),
        assets_json: JSON.stringify(record.assets),
        persona_reference: record.personaReference,
        html_hash: record.htmlHash,
        fetched_at: record.fetchedAt,
      });
  }

  hasShip(wikiTitle: string): boolean {
    return !!this.db.prepare("SELECT 1 FROM ships WHERE wiki_title = ?").get(wikiTitle);
  }

  getShip(wikiTitle: string): ShipRecord | null {
    const row = this.db
      .prepare("SELECT * FROM ships WHERE wiki_title = ?")
      .get(wikiTitle) as Record<string, unknown> | undefined;
    return row ? rowToShip(row) : null;
  }

  findShipByName(name: string): ShipRecord | null {
    const byTitle = this.getShip(name);
    if (byTitle) return byTitle;

    const byDisplay = this.db
      .prepare("SELECT * FROM ships WHERE display_name = ? LIMIT 1")
      .get(name) as Record<string, unknown> | undefined;
    if (byDisplay) return rowToShip(byDisplay);

    const like = `%${name}%`;
    const row = this.db
      .prepare(
        `
      SELECT * FROM ships
      WHERE display_name LIKE @like
         OR aliases_json LIKE @like
         OR wiki_title LIKE @like
      LIMIT 1
    `
      )
      .get({ like }) as Record<string, unknown> | undefined;
    return row ? rowToShip(row) : null;
  }

  searchShips(query: string, limit = 20): ShipRecord[] {
    const like = `%${query}%`;
    const rows = this.db
      .prepare(
        `
      SELECT * FROM ships
      WHERE display_name LIKE @like
         OR aliases_json LIKE @like
         OR wiki_title LIKE @like
      ORDER BY display_name
      LIMIT @limit
    `
      )
      .all({ like, limit }) as Record<string, unknown>[];
    return rows.map(rowToShip);
  }

  listCatalog(options: {
    fetched?: "all" | "yes" | "no";
    limit?: number;
    offset?: number;
  }): Array<CatalogEntry & { fetched: boolean }> {
    const limit = options.limit ?? 50;
    const offset = options.offset ?? 0;
    let where = "1=1";
    if (options.fetched === "yes") where = "s.wiki_title IS NOT NULL";
    if (options.fetched === "no") where = "s.wiki_title IS NULL";

    const rows = this.db
      .prepare(
        `
      SELECT c.*, CASE WHEN s.wiki_title IS NULL THEN 0 ELSE 1 END AS fetched_flag
      FROM catalog c
      LEFT JOIN ships s ON s.wiki_title = c.wiki_title
      WHERE ${where}
      ORDER BY c.display_name
      LIMIT @limit OFFSET @offset
    `
      )
      .all({ limit, offset }) as Record<string, unknown>[];

    return rows.map((r) => ({
      wikiTitle: String(r.wiki_title),
      wikiPath: String(r.wiki_path),
      displayName: String(r.display_name),
      aliases: JSON.parse(String(r.aliases_json)) as string[],
      avatarUrl: (r.avatar_url as string | null) ?? null,
      rarity: (r.rarity as string | null) ?? null,
      faction: (r.faction as string | null) ?? null,
      shipType: (r.ship_type as string | null) ?? null,
      fetched: Number(r.fetched_flag) === 1,
    }));
  }

  listPendingTitles(limit = 20): CatalogEntry[] {
    const rows = this.db
      .prepare(
        `
      SELECT c.*
      FROM catalog c
      LEFT JOIN ships s ON s.wiki_title = c.wiki_title
      WHERE s.wiki_title IS NULL
      ORDER BY c.display_name
      LIMIT @limit
    `
      )
      .all({ limit }) as Record<string, unknown>[];

    return rows.map(catalogRowToEntry);
  }

  getCatalogEntry(wikiTitle: string): CatalogEntry | null {
    const row = this.db
      .prepare("SELECT * FROM catalog WHERE wiki_title = ?")
      .get(wikiTitle) as Record<string, unknown> | undefined;
    return row ? catalogRowToEntry(row) : null;
  }

  stats(): SyncStats {
    const catalogTotal = (
      this.db.prepare("SELECT COUNT(*) AS c FROM catalog").get() as { c: number }
    ).c;
    const fetchedTotal = (
      this.db.prepare("SELECT COUNT(*) AS c FROM ships").get() as { c: number }
    ).c;
    return {
      catalogTotal,
      fetchedTotal,
      pendingTotal: Math.max(0, catalogTotal - fetchedTotal),
      lastCatalogSync: this.getMeta("last_catalog_sync"),
    };
  }

  private setMeta(key: string, value: string): void {
    this.db
      .prepare(
        "INSERT INTO sync_meta(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
      )
      .run(key, value);
  }

  private getMeta(key: string): string | null {
    const row = this.db
      .prepare("SELECT value FROM sync_meta WHERE key = ?")
      .get(key) as { value: string } | undefined;
    return row?.value ?? null;
  }

  close(): void {
    this.db.close();
  }
}

function catalogRowToEntry(r: Record<string, unknown>): CatalogEntry {
  return {
    wikiTitle: String(r.wiki_title),
    wikiPath: String(r.wiki_path),
    displayName: String(r.display_name),
    aliases: JSON.parse(String(r.aliases_json)) as string[],
    avatarUrl: (r.avatar_url as string | null) ?? null,
    rarity: (r.rarity as string | null) ?? null,
    faction: (r.faction as string | null) ?? null,
    shipType: (r.ship_type as string | null) ?? null,
  };
}

function rowToShip(r: Record<string, unknown>): ShipRecord {
  let linesBySkin: ShipLineGroup[] = [];
  try {
    linesBySkin = JSON.parse(String(r.lines_by_skin_json ?? "[]")) as ShipLineGroup[];
  } catch {
    linesBySkin = [];
  }
  return {
    wikiTitle: String(r.wiki_title),
    wikiUrl: String(r.wiki_url),
    displayName: String(r.display_name),
    aliases: JSON.parse(String(r.aliases_json)) as string[],
    rarity: (r.rarity as string | null) ?? null,
    faction: (r.faction as string | null) ?? null,
    shipType: (r.ship_type as string | null) ?? null,
    cv: (r.cv as string | null) ?? null,
    characterInfo: JSON.parse(String(r.character_info_json)),
    sections: JSON.parse(String(r.sections_json)) as ShipSection[],
    lines: JSON.parse(String(r.lines_json)) as ShipLine[],
    linesBySkin: Array.isArray(linesBySkin) ? linesBySkin : [],
    assets: JSON.parse(String(r.assets_json)) as ShipAsset[],
    personaReference: String(r.persona_reference),
    fetchedAt: String(r.fetched_at),
    htmlHash: String(r.html_hash),
  };
}
