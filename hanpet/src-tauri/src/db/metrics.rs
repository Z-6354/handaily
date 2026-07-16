//! 今日输入与文件操作指标（键鼠、文件写入/修改）

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct DailyMetrics {
    pub date: String,
    pub mouse_clicks: u64,
    pub key_strokes: u64,
    pub keyboard_text: String,
    pub files_created: u64,
    pub files_modified: u64,
}

pub fn migrate_metrics(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS daily_metrics (\
             date TEXT PRIMARY KEY,\
             mouse_clicks INTEGER NOT NULL DEFAULT 0,\
             key_strokes INTEGER NOT NULL DEFAULT 0,\
             keyboard_text TEXT NOT NULL DEFAULT '',\
             files_created INTEGER NOT NULL DEFAULT 0,\
             files_modified INTEGER NOT NULL DEFAULT 0\
         );",
    )
}

pub fn today_string() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn load_today(db: &Connection) -> Result<DailyMetrics, rusqlite::Error> {
    let date = today_string();
    db.query_row(
        "SELECT date, mouse_clicks, key_strokes, keyboard_text, files_created, files_modified \
         FROM daily_metrics WHERE date = ?1",
        [&date],
        |row| {
            Ok(DailyMetrics {
                date: row.get(0)?,
                mouse_clicks: row.get::<_, i64>(1)? as u64,
                key_strokes: row.get::<_, i64>(2)? as u64,
                keyboard_text: row.get(3)?,
                files_created: row.get::<_, i64>(4)? as u64,
                files_modified: row.get::<_, i64>(5)? as u64,
            })
        },
    )
    .or_else(|e| {
        if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
            Ok(DailyMetrics {
                date,
                ..Default::default()
            })
        } else {
            Err(e)
        }
    })
}

/// 将内存计数合并写入 DB（增量 upsert）
pub fn upsert_delta(
    db: &Connection,
    mouse_delta: u64,
    key_delta: u64,
    text_append: &str,
    files_created_delta: u64,
    files_modified_delta: u64,
) -> Result<(), rusqlite::Error> {
    if mouse_delta == 0
        && key_delta == 0
        && text_append.is_empty()
        && files_created_delta == 0
        && files_modified_delta == 0
    {
        return Ok(());
    }
    let date = today_string();
    db.execute(
        "INSERT INTO daily_metrics (date, mouse_clicks, key_strokes, keyboard_text, files_created, files_modified) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(date) DO UPDATE SET \
           mouse_clicks = mouse_clicks + excluded.mouse_clicks,\
           key_strokes = key_strokes + excluded.key_strokes,\
           keyboard_text = keyboard_text || excluded.keyboard_text,\
           files_created = files_created + excluded.files_created,\
           files_modified = files_modified + excluded.files_modified",
        rusqlite::params![
            date,
            mouse_delta as i64,
            key_delta as i64,
            text_append,
            files_created_delta as i64,
            files_modified_delta as i64,
        ],
    )?;
  // 截断过长文本（保留最近 20k 字符）
    db.execute(
        "UPDATE daily_metrics SET keyboard_text = substr(keyboard_text, -20000) \
         WHERE date = ?1 AND length(keyboard_text) > 20000",
        [date],
    )?;
    Ok(())
}
