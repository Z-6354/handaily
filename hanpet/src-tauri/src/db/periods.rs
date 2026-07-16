//! 时段 AI 总结与工作类型（按小时）

use chrono::{NaiveDate, Timelike};
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct PeriodSummaryPublic {
    pub id: i64,
    pub started_at: String,
    pub ended_at: String,
    pub work_type: String,
    pub summary: String,
    pub trigger: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct HourWorkType {
    pub work_type: String,
    pub summary: String,
}

pub fn migrate_periods(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS period_summaries (\
             id INTEGER PRIMARY KEY AUTOINCREMENT,\
             started_at TEXT NOT NULL,\
             ended_at TEXT NOT NULL,\
             work_type TEXT NOT NULL,\
             summary TEXT NOT NULL,\
             trigger TEXT NOT NULL,\
             created_at TEXT NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_period_started ON period_summaries(started_at);\
         CREATE TABLE IF NOT EXISTS hour_work_types (\
             date TEXT NOT NULL,\
             hour INTEGER NOT NULL,\
             work_type TEXT NOT NULL,\
             summary TEXT NOT NULL DEFAULT '',\
             updated_at TEXT NOT NULL,\
             PRIMARY KEY (date, hour)\
         );",
    )
}

pub fn insert_period_summary(
    db: &Connection,
    started_at: &str,
    ended_at: &str,
    work_type: &str,
    summary: &str,
    trigger: &str,
    created_at: &str,
) -> Result<i64, rusqlite::Error> {
    db.execute(
        "INSERT INTO period_summaries (started_at, ended_at, work_type, summary, trigger, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![started_at, ended_at, work_type, summary, trigger, created_at],
    )?;
    Ok(db.last_insert_rowid())
}

pub fn upsert_hour_work_type(
    db: &Connection,
    date: &str,
    hour: u32,
    work_type: &str,
    summary: &str,
    updated_at: &str,
) -> Result<(), rusqlite::Error> {
    db.execute(
        "INSERT INTO hour_work_types (date, hour, work_type, summary, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(date, hour) DO UPDATE SET \
           work_type = excluded.work_type,\
           summary = excluded.summary,\
           updated_at = excluded.updated_at",
        rusqlite::params![date, hour as i64, work_type, summary, updated_at],
    )?;
    Ok(())
}

pub fn load_hour_types_for_date(
    db: &Connection,
    date: NaiveDate,
) -> Result<[Option<HourWorkType>; 24], rusqlite::Error> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let mut out: [Option<HourWorkType>; 24] = std::array::from_fn(|_| None);
    let mut stmt = db.prepare(
        "SELECT hour, work_type, summary FROM hour_work_types WHERE date = ?1",
    )?;
    let rows = stmt.query_map([&date_str], |row| {
        Ok((
            row.get::<_, i64>(0)? as usize,
            HourWorkType {
                work_type: row.get(1)?,
                summary: row.get(2)?,
            },
        ))
    })?;
    for row in rows {
        let (h, wt) = row?;
        if h < 24 {
            out[h] = Some(wt);
        }
    }
    Ok(out)
}

pub fn list_period_summaries_today(
    db: &Connection,
    limit: i64,
) -> Result<Vec<PeriodSummaryPublic>, rusqlite::Error> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = db.prepare(
        "SELECT id, started_at, ended_at, work_type, summary, trigger, created_at \
         FROM period_summaries WHERE substr(started_at, 1, 10) = ?1 \
         ORDER BY started_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![today, limit], |row| {
        Ok(PeriodSummaryPublic {
            id: row.get(0)?,
            started_at: row.get(1)?,
            ended_at: row.get(2)?,
            work_type: row.get(3)?,
            summary: row.get(4)?,
            trigger: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    rows.collect()
}

/// 区间去重：该 [start,end] 区间是否已有时段总结（避免重复调用 AI）
pub fn period_summary_exists(
    db: &Connection,
    start_iso: &str,
    end_iso: &str,
) -> Result<bool, rusqlite::Error> {
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM period_summaries WHERE started_at = ?1 AND ended_at = ?2",
        rusqlite::params![start_iso, end_iso],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// 今日已生成的时段总结数（用于每日 AI 预算门）
pub fn count_period_summaries_today(db: &Connection) -> Result<u64, rusqlite::Error> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM period_summaries WHERE substr(started_at, 1, 10) = ?1",
        rusqlite::params![today],
        |row| row.get(0),
    )?;
    Ok(count as u64)
}

pub fn list_period_summaries_in_range(
    db: &Connection,
    date_from: &str,
    date_to: &str,
    limit: i64,
) -> Result<Vec<PeriodSummaryPublic>, rusqlite::Error> {
    let mut stmt = db.prepare(
        "SELECT id, started_at, ended_at, work_type, summary, trigger, created_at \
         FROM period_summaries \
         WHERE substr(started_at, 1, 10) >= ?1 AND substr(started_at, 1, 10) <= ?2 \
         ORDER BY started_at ASC LIMIT ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![date_from, date_to, limit], |row| {
        Ok(PeriodSummaryPublic {
            id: row.get(0)?,
            started_at: row.get(1)?,
            ended_at: row.get(2)?,
            work_type: row.get(3)?,
            summary: row.get(4)?,
            trigger: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    rows.collect()
}

/// 查询时间范围内的非 idle 活动段（用于 AI 分析）
pub fn query_segments_in_range(
    db: &Connection,
    start_iso: &str,
    end_iso: &str,
) -> Result<Vec<crate::tracker::Segment>, rusqlite::Error> {
    let mut stmt = db.prepare(
        "SELECT started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key \
         FROM activity_segments \
         WHERE is_idle = 0 AND started_at >= ?1 AND started_at < ?2 \
         ORDER BY started_at ASC LIMIT 200",
    )?;
    let rows = stmt.query_map([start_iso, end_iso], |row| {
        Ok(crate::tracker::Segment {
            started_at: row.get(0)?,
            ended_at: row.get(1)?,
            duration_ms: row.get::<_, i64>(2)? as u64,
            app_name: row.get(3)?,
            exe_path: row.get(4)?,
            window_title: row.get(5)?,
            is_idle: row.get::<_, i64>(6)? != 0,
            aggregation_key: row.get(7)?,
            icon: None,
            source_type: "foreground".into(),
            audio_activity: String::new(),
            activity_label: None,
        })
    })?;
    rows.collect()
}

/// 将时段工作类型写入该时段覆盖的小时桶
pub fn apply_work_type_to_hours(
    db: &Connection,
    started_at: &str,
    ended_at: &str,
    work_type: &str,
    summary: &str,
    updated_at: &str,
) -> Result<(), rusqlite::Error> {
    let start = chrono::DateTime::parse_from_rfc3339(started_at)
        .map(|d| d.with_timezone(&chrono::Local))
        .ok();
    let end = chrono::DateTime::parse_from_rfc3339(ended_at)
        .map(|d| d.with_timezone(&chrono::Local))
        .ok();
    let (Some(s), Some(e)) = (start, end) else {
        return Ok(());
    };
    let mut h = s.hour();
    let end_h = e.hour();
    let date = s.date_naive().format("%Y-%m-%d").to_string();
    loop {
        upsert_hour_work_type(db, &date, h, work_type, summary, updated_at)?;
        if h == end_h {
            break;
        }
        h = (h + 1) % 24;
        if h == 0 {
            break;
        }
    }
    Ok(())
}
