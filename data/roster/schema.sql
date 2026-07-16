-- handaily roster schema (local private DB + bundled preview subset share this schema)
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS characters (
  id TEXT PRIMARY KEY,
  name_zh TEXT NOT NULL,
  name_en TEXT NOT NULL DEFAULT '',
  wiki_title TEXT NOT NULL DEFAULT '',
  cv TEXT NOT NULL DEFAULT '',
  faction TEXT NOT NULL DEFAULT '',
  ship_type TEXT NOT NULL DEFAULT '',
  rarity TEXT NOT NULL DEFAULT '',
  persona_id TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT '',
  meta_json TEXT NOT NULL DEFAULT '{}',
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS skins (
  id TEXT PRIMARY KEY,
  character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  name_zh TEXT NOT NULL,
  name_en TEXT NOT NULL DEFAULT '',
  skin_index INTEGER,
  pet_model_id TEXT NOT NULL DEFAULT '',
  kanmusu_dir TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0,
  is_default INTEGER NOT NULL DEFAULT 0,
  meta_json TEXT NOT NULL DEFAULT '{}',
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_skins_character ON skins(character_id);

CREATE TABLE IF NOT EXISTS skin_lines (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  skin_id TEXT NOT NULL REFERENCES skins(id) ON DELETE CASCADE,
  wiki_key TEXT NOT NULL DEFAULT '',
  label TEXT NOT NULL DEFAULT '',
  lang TEXT NOT NULL DEFAULT '',
  text TEXT NOT NULL,
  animation TEXT NOT NULL DEFAULT '',
  audio_url TEXT NOT NULL DEFAULT '',
  audio_relpath TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_skin_lines_skin ON skin_lines(skin_id);
