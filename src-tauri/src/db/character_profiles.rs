//! 角色资料与 Skill 草稿（人设工坊）

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterProfileData {
    pub name: String,
    #[serde(default)]
    pub source: String,
    /// 介绍 / 背景
    #[serde(default)]
    pub introduction: String,
    /// 性格要点
    #[serde(default)]
    pub personality: Vec<String>,
    /// 说话风格
    #[serde(default)]
    pub speech_style: String,
    /// 台词 / 口癖示例
    #[serde(default)]
    pub sample_lines: Vec<String>,
    /// 人际关系
    #[serde(default)]
    pub relationships: String,
    /// 禁忌 / 不要做的事
    #[serde(default)]
    pub taboos: Vec<String>,
    /// 其它键值（世界观、外貌等）
    #[serde(default)]
    pub extra: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterProfileRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub source: String,
    pub raw_text: String,
    pub profile_json: CharacterProfileData,
    pub skill_md: String,
    pub persona_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn migrate_character_profiles(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS character_profiles (\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             slug TEXT NOT NULL UNIQUE,\
             name TEXT NOT NULL,\
             source TEXT NOT NULL DEFAULT '',\
             raw_text TEXT NOT NULL DEFAULT '',\
             profile_json TEXT NOT NULL DEFAULT '{}',\
             skill_md TEXT NOT NULL DEFAULT '',\
             persona_id TEXT,\
             created_at TEXT NOT NULL,\
             updated_at TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_character_profiles_slug ON character_profiles(slug);",
    )
}

fn row_from_query(row: &rusqlite::Row<'_>) -> Result<CharacterProfileRow, rusqlite::Error> {
    let json_raw: String = row.get(5)?;
    let profile_json: CharacterProfileData =
        serde_json::from_str(&json_raw).unwrap_or_default();
    Ok(CharacterProfileRow {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        source: row.get(3)?,
        raw_text: row.get(4)?,
        profile_json,
        skill_md: row.get(6)?,
        persona_id: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

const SELECT_COLS: &str =
    "id, slug, name, source, raw_text, profile_json, skill_md, persona_id, created_at, updated_at";

pub fn list_profiles(db: &Connection) -> Result<Vec<CharacterProfileRow>, rusqlite::Error> {
    let mut stmt = db.prepare(&format!(
        "SELECT {SELECT_COLS} FROM character_profiles ORDER BY updated_at DESC"
    ))?;
    let rows = stmt.query_map([], row_from_query)?;
    rows.collect()
}

pub fn get_profile(db: &Connection, id: i64) -> Result<Option<CharacterProfileRow>, rusqlite::Error> {
    let mut stmt = db.prepare(&format!(
        "SELECT {SELECT_COLS} FROM character_profiles WHERE id = ?1"
    ))?;
    let mut rows = stmt.query([id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_from_query(&row)?))
    } else {
        Ok(None)
    }
}

pub fn insert_profile(
    db: &Connection,
    slug: &str,
    name: &str,
    source: &str,
    raw_text: &str,
    now: &str,
) -> Result<i64, rusqlite::Error> {
    db.execute(
        "INSERT INTO character_profiles (slug, name, source, raw_text, profile_json, skill_md, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, '{}', '', ?5, ?5)",
        rusqlite::params![slug, name, source, raw_text, now],
    )?;
    Ok(db.last_insert_rowid())
}

pub fn update_raw_text(
    db: &Connection,
    id: i64,
    raw_text: &str,
    now: &str,
) -> Result<(), rusqlite::Error> {
    db.execute(
        "UPDATE character_profiles SET raw_text = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![raw_text, now, id],
    )?;
    Ok(())
}

pub fn update_profile_json(
    db: &Connection,
    id: i64,
    data: &CharacterProfileData,
    now: &str,
) -> Result<(), rusqlite::Error> {
    let json = serde_json::to_string(data).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(e))
    })?;
    db.execute(
        "UPDATE character_profiles SET profile_json = ?1, name = ?2, source = ?3, updated_at = ?4 WHERE id = ?5",
        rusqlite::params![json, data.name, data.source, now, id],
    )?;
    Ok(())
}

pub fn update_skill_md(
    db: &Connection,
    id: i64,
    skill_md: &str,
    now: &str,
) -> Result<(), rusqlite::Error> {
    db.execute(
        "UPDATE character_profiles SET skill_md = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![skill_md, now, id],
    )?;
    Ok(())
}

pub fn set_persona_id(
    db: &Connection,
    id: i64,
    persona_id: &str,
    now: &str,
) -> Result<(), rusqlite::Error> {
    db.execute(
        "UPDATE character_profiles SET persona_id = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![persona_id, now, id],
    )?;
    Ok(())
}

pub fn delete_profile(db: &Connection, id: i64) -> Result<(), rusqlite::Error> {
    db.execute("DELETE FROM character_profiles WHERE id = ?1", [id])?;
    Ok(())
}

pub fn slug_exists(db: &Connection, slug: &str) -> Result<bool, rusqlite::Error> {
    let n: i64 = db.query_row(
        "SELECT COUNT(*) FROM character_profiles WHERE slug = ?1",
        [slug],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn find_by_persona_id(
    db: &Connection,
    persona_id: &str,
) -> Result<Option<CharacterProfileRow>, rusqlite::Error> {
    let mut stmt = db.prepare(&format!(
        "SELECT {SELECT_COLS} FROM character_profiles WHERE persona_id = ?1 ORDER BY updated_at DESC LIMIT 1"
    ))?;
    let mut rows = stmt.query([persona_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_from_query(&row)?))
    } else {
        Ok(None)
    }
}
