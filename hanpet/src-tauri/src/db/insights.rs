//! 活动语义洞察持久化

use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct InsightRow {
    pub started_at: String,
    pub ended_at: Option<String>,
    pub app_name: String,
    pub window_title: String,
    pub aggregation_key: String,
    pub source: String,
    pub category: String,
    pub summary: String,
    pub confidence: f32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsightPublic {
    pub id: i64,
    pub started_at: String,
    pub app_name: String,
    pub source: String,
    pub category: String,
    pub summary: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TodayAnalysisStats {
    pub text_count: u64,
    pub screenshot_count: u64,
    pub skipped_screenshot_count: u64,
    pub system_cpu_percent: f32,
}

pub fn migrate_insights(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS activity_insights (\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             started_at TEXT NOT NULL,\
             ended_at TEXT,\
             app_name TEXT NOT NULL,\
             window_title TEXT NOT NULL DEFAULT '',\
             aggregation_key TEXT NOT NULL DEFAULT '',\
             source TEXT NOT NULL,\
             category TEXT NOT NULL DEFAULT '',\
             summary TEXT NOT NULL,\
             confidence REAL NOT NULL DEFAULT 0,\
             created_at TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_insights_started ON activity_insights(started_at);",
    )?;
    Ok(())
}

pub fn insert_insight(db: &Connection, row: &InsightRow) -> Result<(), rusqlite::Error> {
    db.execute(
        "INSERT INTO activity_insights \
         (started_at, ended_at, app_name, window_title, aggregation_key, source, category, summary, confidence, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            row.started_at,
            row.ended_at,
            row.app_name,
            row.window_title,
            row.aggregation_key,
            row.source,
            row.category,
            row.summary,
            row.confidence,
            row.created_at,
        ],
    )?;
    Ok(())
}

pub fn list_today(db: &Connection, limit: i64) -> Result<Vec<InsightPublic>, rusqlite::Error> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let prefix = format!("{today}%");
    let mut stmt = db.prepare(
        "SELECT id, started_at, app_name, source, category, summary, confidence \
         FROM activity_insights WHERE started_at LIKE ?1 \
         ORDER BY started_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![prefix, limit], |row| {
        Ok(InsightPublic {
            id: row.get(0)?,
            started_at: row.get(1)?,
            app_name: row.get(2)?,
            source: row.get(3)?,
            category: row.get(4)?,
            summary: row.get(5)?,
            confidence: row.get(6)?,
        })
    })?;
    rows.collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct InsightDetail {
    pub source: String,
    pub category: String,
    pub summary: String,
    pub confidence: f32,
    pub window_title: String,
}

pub fn insights_for_segment(
    db: &Connection,
    start_iso: &str,
    end_iso: &str,
) -> Result<Vec<InsightDetail>, rusqlite::Error> {
    let mut stmt = db.prepare(
        "SELECT id, started_at, app_name, source, category, summary, confidence, window_title \
         FROM activity_insights \
         WHERE started_at >= ?1 AND started_at <= ?2 \
         ORDER BY started_at ASC LIMIT 8",
    )?;
    let rows = stmt.query_map(rusqlite::params![start_iso, end_iso], |row| {
        Ok(InsightDetail {
            source: row.get(3)?,
            category: row.get(4)?,
            summary: row.get(5)?,
            confidence: row.get(6)?,
            window_title: row.get(7)?,
        })
    })?;
    rows.collect()
}

/// 同应用近期洞察（用于补充「刚才在聊什么」等上下文）
pub fn recent_for_app(
    db: &Connection,
    aggregation_key: &str,
    before_iso: &str,
    limit: i64,
) -> Result<Vec<InsightDetail>, rusqlite::Error> {
    let mut stmt = db.prepare(
        "SELECT source, category, summary, confidence, window_title \
         FROM activity_insights \
         WHERE aggregation_key = ?1 AND started_at < ?2 \
         ORDER BY started_at DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![aggregation_key, before_iso, limit], |row| {
        Ok(InsightDetail {
            source: row.get(0)?,
            category: row.get(1)?,
            summary: row.get(2)?,
            confidence: row.get(3)?,
            window_title: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn today_stats(db: &Connection) -> Result<TodayAnalysisStats, rusqlite::Error> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let prefix = format!("{today}%");

    let text_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM activity_insights WHERE started_at LIKE ?1 AND source = 'text'",
        [&prefix],
        |row| row.get(0),
    )?;

    let screenshot_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM activity_insights WHERE started_at LIKE ?1 AND source = 'screenshot'",
        [&prefix],
        |row| row.get(0),
    )?;

    let skipped: i64 = db.query_row(
        "SELECT COUNT(*) FROM activity_insights WHERE started_at LIKE ?1 \
         AND source = 'text' AND summary LIKE '%已跳过截图%'",
        [&prefix],
        |row| row.get(0),
    )?;

    Ok(TodayAnalysisStats {
        text_count: text_count as u64,
        screenshot_count: screenshot_count as u64,
        skipped_screenshot_count: skipped as u64,
        system_cpu_percent: 0.0,
    })
}
