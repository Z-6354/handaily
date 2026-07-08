//! 应用运行时长与桌宠陪伴时长（按日累计，墙钟时间）

use chrono::{Local, NaiveDate};
use rusqlite::Connection;

fn migrate_table(db: &Connection, table: &str) -> Result<(), rusqlite::Error> {
    db.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {table} (\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             started_at TEXT NOT NULL,\
             ended_at TEXT,\
             duration_ms INTEGER NOT NULL DEFAULT 0\
         );\
         CREATE INDEX IF NOT EXISTS idx_{table}_started ON {table}(started_at);"
    ))
}

pub fn migrate_usage(db: &Connection) -> Result<(), rusqlite::Error> {
    migrate_table(db, "app_usage_sessions")?;
    migrate_table(db, "companion_sessions")?;
    Ok(())
}

pub fn recover_orphan_sessions(db: &Connection) -> Result<(), rusqlite::Error> {
    for table in ["app_usage_sessions", "companion_sessions"] {
        close_open_session(db, table)?;
    }
    Ok(())
}

pub fn open_app_session(db: &Connection) -> Result<(), rusqlite::Error> {
    open_session(db, "app_usage_sessions")
}

pub fn close_app_session(db: &Connection) -> Result<(), rusqlite::Error> {
    close_open_session(db, "app_usage_sessions")
}

pub fn open_companion_session(db: &Connection) -> Result<(), rusqlite::Error> {
    open_session(db, "companion_sessions")
}

pub fn close_companion_session(db: &Connection) -> Result<(), rusqlite::Error> {
    close_open_session(db, "companion_sessions")
}

fn open_session(db: &Connection, table: &str) -> Result<(), rusqlite::Error> {
    let open: i64 = db.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE ended_at IS NULL"),
        [],
        |row| row.get(0),
    )?;
    if open > 0 {
        return Ok(());
    }
    let started = Local::now().to_rfc3339();
    db.execute(
        &format!(
            "INSERT INTO {table} (started_at, ended_at, duration_ms) VALUES (?1, NULL, 0)"
        ),
        [&started],
    )?;
    Ok(())
}

fn close_open_session(db: &Connection, table: &str) -> Result<(), rusqlite::Error> {
    let now = Local::now().to_rfc3339();
    db.execute(
        &format!(
            "UPDATE {table} SET \
             ended_at = ?1, \
             duration_ms = CAST((julianday(?1) - julianday(started_at)) * 86400000 AS INTEGER) \
             WHERE ended_at IS NULL"
        ),
        [&now],
    )?;
    Ok(())
}

fn ms_for_date(db: &Connection, table: &str, date: NaiveDate) -> Result<u64, rusqlite::Error> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let mut total: i64 = 0;

    let mut stmt = db.prepare(&format!(
        "SELECT started_at, ended_at, duration_ms FROM {table} \
         WHERE substr(started_at, 1, 10) = ?1 \
         ORDER BY started_at DESC"
    ))?;
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

pub fn app_usage_ms_for_date(db: &Connection, date: NaiveDate) -> Result<u64, rusqlite::Error> {
    ms_for_date(db, "app_usage_sessions", date)
}

pub fn companion_ms_for_date(db: &Connection, date: NaiveDate) -> Result<u64, rusqlite::Error> {
    ms_for_date(db, "companion_sessions", date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        migrate_usage(&db).unwrap();
        db
    }

    #[test]
    fn recover_orphan_preserves_companion_duration() {
        let db = test_db();
        let started = (Local::now() - chrono::Duration::hours(2)).to_rfc3339();
        db.execute(
            "INSERT INTO companion_sessions (started_at, ended_at, duration_ms) VALUES (?1, NULL, 0)",
            [&started],
        )
        .unwrap();
        recover_orphan_sessions(&db).unwrap();
        let today = Local::now().date_naive();
        let ms = companion_ms_for_date(&db, today).unwrap();
        assert!(ms >= 3_600_000, "expected ~2h companion, got {ms}");
        let open: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM companion_sessions WHERE ended_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(open, 0);
    }
}
