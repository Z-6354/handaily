//! 数据库模块根
//!
//! - `open_and_migrate` — 打开连接 + WAL + 建表 + 幂等迁移
//! - `insert_segment` — 写一行 activity_segments
//! - `get_setting` / `set_setting` — app_settings 读写
//! - `stats` — 聚合查询与 TodayAggregator

pub mod sessions;
pub mod stats;
pub mod usage;
pub mod insights;
pub mod metrics;
pub mod periods;
pub mod reports;
pub mod timeline_cache;
pub mod character_profiles;

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// 获取 DB 锁；若此前线程 panic 导致 poison，尝试恢复以免整 app 无法读写
pub fn lock_conn(db: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>, String> {
    match db.lock() {
        Ok(guard) => Ok(guard),
        Err(poison) => {
            crate::log::warn("db mutex poisoned, attempting recovery");
            Ok(poison.into_inner())
        }
    }
}

/// 打开 SQLite 连接 + 开 WAL + 建表 + 幂等迁移
pub fn open_and_migrate(db_path: &Path) -> Result<Connection, rusqlite::Error> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let db = Connection::open(db_path)?;
    db.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = NORMAL;\
         PRAGMA foreign_keys = ON;",
    )?;
    migrate(&db)?;
    Ok(db)
}

/// 建表（幂等）
fn migrate(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS activity_segments (\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             started_at TEXT NOT NULL,\
             ended_at TEXT,\
             duration_ms INTEGER NOT NULL DEFAULT 0,\
             app_name TEXT NOT NULL,\
             exe_path TEXT NOT NULL,\
             window_title TEXT NOT NULL DEFAULT '',\
             is_idle INTEGER NOT NULL DEFAULT 0,\
             aggregation_key TEXT NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS app_settings (\
             key TEXT PRIMARY KEY,\
             value TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_segments_started ON activity_segments(started_at);\
         CREATE INDEX IF NOT EXISTS idx_segments_agg ON activity_segments(aggregation_key);\
         CREATE INDEX IF NOT EXISTS idx_segments_ended ON activity_segments(ended_at);\
         CREATE TABLE IF NOT EXISTS tracking_sessions (\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             started_at TEXT NOT NULL,\
             ended_at TEXT,\
             duration_ms INTEGER NOT NULL DEFAULT 0\
         );\
         CREATE INDEX IF NOT EXISTS idx_sessions_started ON tracking_sessions(started_at);\
         CREATE TABLE IF NOT EXISTS vault_config (\
             key TEXT PRIMARY KEY,\
             value BLOB NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS vault_entries (\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             name TEXT NOT NULL,\
             provider TEXT NOT NULL DEFAULT '',\
             note TEXT NOT NULL DEFAULT '',\
             nonce BLOB NOT NULL,\
             ciphertext BLOB NOT NULL,\
             created_at TEXT NOT NULL,\
             updated_at TEXT NOT NULL\
         );",
    )?;
    insights::migrate_insights(db)?;
    metrics::migrate_metrics(db)?;
    periods::migrate_periods(db)?;
    reports::migrate_reports(db)?;
    timeline_cache::migrate_timeline_cache(db)?;
    character_profiles::migrate_character_profiles(db)?;
    usage::migrate_usage(db)?;
    ensure_column(db, "activity_segments", "source_type", "TEXT NOT NULL DEFAULT 'foreground'")?;
    ensure_column(db, "activity_segments", "audio_activity", "TEXT NOT NULL DEFAULT ''")?;
    Ok(())
}

/// 幂等加列（为后续 Phase 留口子）
#[allow(dead_code)]
fn ensure_column(
    db: &Connection,
    table: &str,
    column: &str,
    ddl: &str,
) -> Result<(), rusqlite::Error> {
    let cols: Vec<String> = db
        .prepare(&format!("PRAGMA table_info({})", table))?
        .query_map([], |row| row.get::<_, String>("name"))?
        .filter_map(|r| r.ok())
        .collect();
    if !cols.iter().any(|c| c == column) {
        db.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, ddl),
            [],
        )?;
    }
    Ok(())
}

/// 写一行 segment
pub fn insert_segment(
    db: &Connection,
    seg: &crate::tracker::Segment,
) -> Result<(), rusqlite::Error> {
    db.execute(
        "INSERT INTO activity_segments \
         (started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key, source_type, audio_activity) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            seg.started_at,
            seg.ended_at,
            seg.duration_ms as i64,
            seg.app_name,
            seg.exe_path,
            seg.window_title,
            seg.is_idle as i64,
            seg.aggregation_key,
            seg.source_type,
            seg.audio_activity,
        ],
    )?;
    Ok(())
}

/// 读设置
pub fn get_setting(db: &Connection, key: &str) -> Option<String> {
    db.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

/// 写设置（upsert）
pub fn set_setting(db: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    db.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// 闭合崩溃遗留的 open segment：`ended_at` 设为恢复时刻并计算 `duration_ms`
pub fn recover_orphan_segments(db: &Connection, now_iso: &str) -> Result<(), rusqlite::Error> {
    let mut stmt =
        db.prepare("SELECT started_at FROM activity_segments WHERE ended_at IS NULL")?;
    let starts: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for started_at in starts {
        let duration_ms = segment_duration_ms(&started_at, now_iso);
        db.execute(
            "UPDATE activity_segments SET ended_at = ?1, duration_ms = ?2 \
             WHERE started_at = ?3 AND ended_at IS NULL",
            rusqlite::params![now_iso, duration_ms as i64, started_at],
        )?;
    }
    Ok(())
}

pub(crate) fn segment_duration_ms(started: &str, ended: &str) -> u64 {
    let start = match chrono::DateTime::parse_from_rfc3339(started) {
        Ok(d) => d.with_timezone(&chrono::Local),
        Err(_) => return 0,
    };
    let end = match chrono::DateTime::parse_from_rfc3339(ended) {
        Ok(d) => d.with_timezone(&chrono::Local),
        Err(_) => return 0,
    };
    (end - start).num_milliseconds().max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orphan_segment_recovery() {
        let dir = std::env::temp_dir().join(format!("xiaohan-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let db_path = dir.join("test.sqlite");
        let _ = std::fs::remove_file(&db_path);

        let db = open_and_migrate(&db_path).unwrap();
        db.execute(
            "INSERT INTO activity_segments \
             (started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key) \
             VALUES ('2026-07-02T10:00:00+08:00', NULL, 0, 'a', 'a.exe', '', 0, 'a.exe')",
            [],
        )
        .unwrap();

        let now = "2026-07-02T12:00:00+08:00";
        recover_orphan_segments(&db, now).unwrap();

        let (ended_at, duration_ms): (String, i64) = db
            .query_row(
                "SELECT ended_at, duration_ms FROM activity_segments",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(ended_at, now);
        assert_eq!(duration_ms, 7_200_000);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
