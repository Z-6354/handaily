//! 用户生成的报告存档

use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedReport {
    pub id: i64,
    pub template_id: String,
    pub title: String,
    pub date_from: String,
    pub date_to: String,
    pub content: String,
    pub used_ai: bool,
    pub created_at: String,
}

pub fn migrate_reports(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS generated_reports (\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             template_id TEXT NOT NULL,\
             title TEXT NOT NULL,\
             date_from TEXT NOT NULL,\
             date_to TEXT NOT NULL,\
             content TEXT NOT NULL,\
             used_ai INTEGER NOT NULL DEFAULT 0,\
             created_at TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_reports_created ON generated_reports(created_at DESC);",
    )
}

pub fn insert_report(
    db: &Connection,
    template_id: &str,
    title: &str,
    date_from: &str,
    date_to: &str,
    content: &str,
    used_ai: bool,
    created_at: &str,
) -> Result<i64, rusqlite::Error> {
    db.execute(
        "INSERT INTO generated_reports (template_id, title, date_from, date_to, content, used_ai, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            template_id,
            title,
            date_from,
            date_to,
            content,
            used_ai as i64,
            created_at
        ],
    )?;
    Ok(db.last_insert_rowid())
}

pub fn list_reports(db: &Connection, limit: i64) -> Result<Vec<GeneratedReport>, rusqlite::Error> {
    let mut stmt = db.prepare(
        "SELECT id, template_id, title, date_from, date_to, content, used_ai, created_at \
         FROM generated_reports ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(GeneratedReport {
            id: row.get(0)?,
            template_id: row.get(1)?,
            title: row.get(2)?,
            date_from: row.get(3)?,
            date_to: row.get(4)?,
            content: row.get(5)?,
            used_ai: row.get::<_, i64>(6)? != 0,
            created_at: row.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn get_report(db: &Connection, id: i64) -> Result<Option<GeneratedReport>, rusqlite::Error> {
    let mut stmt = db.prepare(
        "SELECT id, template_id, title, date_from, date_to, content, used_ai, created_at \
         FROM generated_reports WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], |row| {
        Ok(GeneratedReport {
            id: row.get(0)?,
            template_id: row.get(1)?,
            title: row.get(2)?,
            date_from: row.get(3)?,
            date_to: row.get(4)?,
            content: row.get(5)?,
            used_ai: row.get::<_, i64>(6)? != 0,
            created_at: row.get(7)?,
        })
    })?;
    rows.next().transpose()
}

pub fn delete_report(db: &Connection, id: i64) -> Result<bool, rusqlite::Error> {
    let n = db.execute("DELETE FROM generated_reports WHERE id = ?1", [id])?;
    Ok(n > 0)
}
