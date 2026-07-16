//! 时间线片段 AI 简介缓存

use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TimelineAiEntry {
    pub cache_key: String,
    pub started_at: String,
    pub work_type: String,
    pub summary: String,
    pub used_ai: bool,
}

pub fn migrate_timeline_cache(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS timeline_ai_cache (\
             cache_key TEXT PRIMARY KEY,\
             started_at TEXT NOT NULL,\
             work_type TEXT NOT NULL,\
             summary TEXT NOT NULL,\
             used_ai INTEGER NOT NULL DEFAULT 0,\
             created_at TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_timeline_ai_started ON timeline_ai_cache(started_at);",
    )
}

pub fn cache_key(
    started_at: &str,
    ended_at: &str,
    aggregation_key: &str,
    activity_key: &str,
) -> String {
    format!("{started_at}|{ended_at}|{aggregation_key}|{activity_key}")
}

pub fn get_cached(
    db: &Connection,
    keys: &[String],
) -> Result<Vec<TimelineAiEntry>, rusqlite::Error> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for key in keys {
        if let Ok(row) = db.query_row(
            "SELECT cache_key, started_at, work_type, summary, used_ai \
             FROM timeline_ai_cache WHERE cache_key = ?1",
            [key],
            |r| {
                Ok(TimelineAiEntry {
                    cache_key: r.get(0)?,
                    started_at: r.get(1)?,
                    work_type: r.get(2)?,
                    summary: r.get(3)?,
                    used_ai: r.get::<_, i64>(4)? != 0,
                })
            },
        ) {
            out.push(row);
        }
    }
    Ok(out)
}

/// 按 started_at 日期范围列出时间线 AI 简介（升序）
pub fn list_by_date_range(
    db: &Connection,
    date_from: &str,
    date_to: &str,
    limit: i64,
) -> Result<Vec<TimelineAiEntry>, rusqlite::Error> {
    let mut stmt = db.prepare(
        "SELECT cache_key, started_at, work_type, summary, used_ai \
         FROM timeline_ai_cache \
         WHERE substr(started_at, 1, 10) >= ?1 AND substr(started_at, 1, 10) <= ?2 \
         ORDER BY started_at ASC LIMIT ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![date_from, date_to, limit], |r| {
        Ok(TimelineAiEntry {
            cache_key: r.get(0)?,
            started_at: r.get(1)?,
            work_type: r.get(2)?,
            summary: r.get(3)?,
            used_ai: r.get::<_, i64>(4)? != 0,
        })
    })?;
    rows.collect()
}

pub fn upsert_cache(
    db: &Connection,
    key: &str,
    started_at: &str,
    work_type: &str,
    summary: &str,
    used_ai: bool,
    created_at: &str,
) -> Result<(), rusqlite::Error> {
    db.execute(
        "INSERT INTO timeline_ai_cache (cache_key, started_at, work_type, summary, used_ai, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(cache_key) DO UPDATE SET \
           work_type = excluded.work_type,\
           summary = excluded.summary,\
           used_ai = excluded.used_ai,\
           created_at = excluded.created_at",
        rusqlite::params![key, started_at, work_type, summary, used_ai as i64, created_at],
    )?;
    Ok(())
}
