//! 后台采集会话——统计「开始到结束」的墙钟时长（含空闲）

use chrono::{Local, NaiveDate};
use rusqlite::Connection;

/// 开启新的采集会话，返回 id
pub fn open_session(db: &Connection) -> Result<i64, rusqlite::Error> {
    let started = Local::now().to_rfc3339();
    db.execute(
        "INSERT INTO tracking_sessions (started_at, ended_at, duration_ms) VALUES (?1, NULL, 0)",
        [&started],
    )?;
    Ok(db.last_insert_rowid())
}

/// 闭合当前未结束的会话
pub fn close_open_session(db: &Connection) -> Result<(), rusqlite::Error> {
    let now = Local::now().to_rfc3339();
    db.execute(
        "UPDATE tracking_sessions SET \
         ended_at = ?1, \
         duration_ms = CAST((julianday(?1) - julianday(started_at)) * 86400000 AS INTEGER) \
         WHERE ended_at IS NULL",
        [&now],
    )?;
    Ok(())
}

/// 启动兜底：闭合孤儿会话
pub fn recover_orphan_sessions(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute(
        "UPDATE tracking_sessions SET ended_at = started_at, duration_ms = 0 WHERE ended_at IS NULL",
        [],
    )?;
    Ok(())
}

/// 今日后台总时长：从最新会话往回累计（含当前进行中会话）
pub fn background_ms_for_date(db: &Connection, date: NaiveDate) -> Result<u64, rusqlite::Error> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let mut total: i64 = 0;

    let mut stmt = db.prepare(
        "SELECT started_at, ended_at, duration_ms FROM tracking_sessions \
         WHERE substr(started_at, 1, 10) = ?1 \
         ORDER BY started_at DESC",
    )?;
    let rows = stmt.query_map([&date_str], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    for row in rows {
        let (started, ended, duration_ms) = row?;
        if ended.is_some() {
            total += duration_ms;
        } else if let Ok(s) = chrono::DateTime::parse_from_rfc3339(&started) {
            let start = s.with_timezone(&Local);
            if start.date_naive() == date {
                total += (Local::now() - start).num_milliseconds().max(0) as i64;
            }
        }
    }

    Ok(total.max(0) as u64)
}
