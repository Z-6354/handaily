//! 聚合查询与内存缓存
//!
//! `TodayAggregator` 是今日活动的内存快照——segment 闭合时增量更新，
//! command 直接读它（O(1)），只在启动/跨日时从 DB 重建。

use std::collections::HashMap;

use chrono::{NaiveDate, Timelike};
use rusqlite::Connection;
use serde::Serialize;

use crate::tracker::{activity_key, Segment};

/// 今日聚合缓存（command 只读，后台线程增量写）
#[derive(Debug, Clone, Default, Serialize)]
pub struct TodayAggregator {
    /// 有效总时长（ms，is_idle=0）
    pub total_ms: u64,
    /// 切换次数（非 idle segment 数）
    pub switch_count: u64,
    /// 应用聚合：aggregation_key → 时长(ms)
    pub app_breakdown: HashMap<String, u64>,
    /// 小时分布：[00:00, 01:00, ... 23:00] 各小时有效时长
    pub hourly: [u64; 24],
    /// 日期（缓存归属日，用于跨日判断）
    pub date: Option<NaiveDate>,
}

impl TodayAggregator {
    /// 增量更新：segment 闭合时调用
    pub fn apply(&mut self, seg: &Segment) {
        if seg.is_idle {
            return; // idle 段不计入有效统计
        }
        self.total_ms += seg.duration_ms;
        self.switch_count += 1;
        // 小时桶：从 started_at 取本地小时
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&seg.started_at) {
            let h = dt.with_timezone(&chrono::Local).hour() as usize;
            if h < 24 {
                self.hourly[h] += seg.duration_ms;
            }
        }
        // app 聚合（忽略 unknown / idle 占位键）
        if !crate::tracker::title_parse::is_ignored_agg_key(&seg.aggregation_key) {
            *self
                .app_breakdown
                .entry(seg.aggregation_key.clone())
                .or_insert(0) += seg.duration_ms;
        }
    }
}

/// 从 DB 重建指定日期的聚合缓存
pub fn rebuild_aggregator(
    db: &Connection,
    date: NaiveDate,
) -> Result<TodayAggregator, rusqlite::Error> {
    let mut agg = TodayAggregator {
        date: Some(date),
        ..Default::default()
    };
    let date_str = date.format("%Y-%m-%d").to_string();
    let mut stmt = db.prepare(
        "SELECT started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key, \
                COALESCE(source_type, 'foreground'), COALESCE(audio_activity, '') \
         FROM activity_segments \
         WHERE substr(started_at, 1, 10) = ?1 AND is_idle = 0",
    )?;
    let rows = stmt.query_map([&date_str], |row| {
        Ok(Segment {
            started_at: row.get(0)?,
            ended_at: row.get(1)?,
            duration_ms: row.get::<_, i64>(2)? as u64,
            app_name: row.get(3)?,
            exe_path: row.get(4)?,
            window_title: row.get(5)?,
            is_idle: row.get::<_, i64>(6)? != 0,
            aggregation_key: row.get(7)?,
            icon: None,
            source_type: row.get(8)?,
            audio_activity: row.get(9)?,
            activity_label: None,
        })
    })?;
    for row in rows {
        let seg = row?;
        agg.apply(&seg);
    }
    Ok(agg)
}

/// 时间线最少展示时长：切换应用后需超过 1 分钟才显示
pub const TIMELINE_MIN_DURATION_MS: i64 = 60_000;

fn timeline_merge_key(seg: &Segment) -> (String, String, String, String) {
    (
        seg.source_type.clone(),
        seg.audio_activity.clone(),
        seg.app_name.clone(),
        activity_key::activity_key_for_segment(seg),
    )
}

/// 当日时间线合并结果（时间升序）
pub fn query_timeline_merged_asc(
    db: &Connection,
    date: NaiveDate,
) -> Result<Vec<Segment>, rusqlite::Error> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let mut stmt = db.prepare(
        "SELECT started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key, \
                COALESCE(source_type, 'foreground'), COALESCE(audio_activity, '') \
         FROM activity_segments \
         WHERE substr(started_at, 1, 10) = ?1 \
           AND is_idle = 0 \
           AND ((source_type = 'audio' AND duration_ms > 30000) OR (COALESCE(source_type, 'foreground') != 'audio' AND duration_ms > ?2)) \
         ORDER BY started_at ASC",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![&date_str, TIMELINE_MIN_DURATION_MS],
        |row| {
            let exe_path: String = row.get(4)?;
            let aggregation_key: String = row.get(7)?;
            let icon = crate::tracker::icon::resolve_icon_path(&aggregation_key, &exe_path)
                .and_then(|p| crate::tracker::icon::icon_data_url(&p));
            Ok(Segment {
                started_at: row.get(0)?,
                ended_at: row.get(1)?,
                duration_ms: row.get::<_, i64>(2)? as u64,
                app_name: row.get(3)?,
                exe_path,
                window_title: row.get(5)?,
                is_idle: row.get::<_, i64>(6)? != 0,
                aggregation_key,
                icon,
                source_type: row.get(8)?,
                audio_activity: row.get(9)?,
                activity_label: None,
            })
        },
    )?;
    let raw: Vec<Segment> = rows.filter_map(|r| r.ok()).collect();
    Ok(merge_timeline_segments(raw))
}

/// 将时间线上相邻、同一应用且同一活动内容的记录合并为一条
fn merge_timeline_segments(mut items: Vec<Segment>) -> Vec<Segment> {
    if items.len() <= 1 {
        return items;
    }
    items.sort_by(|a, b| a.started_at.cmp(&b.started_at));

    let mut merged: Vec<Segment> = Vec::with_capacity(items.len());
    for seg in items {
        let key = timeline_merge_key(&seg);
        if let Some(last) = merged.last_mut() {
            if timeline_merge_key(last) == key {
                last.ended_at = seg.ended_at.or(last.ended_at.clone());
                last.duration_ms += seg.duration_ms;
                continue;
            }
        }
        merged.push(seg);
    }

    merged
}

/// 按「最近 N 分钟」过滤 segment（`None` 表示不过滤）
pub fn filter_segments_since(segments: Vec<Segment>, since_minutes: Option<i64>) -> Vec<Segment> {
    let Some(mins) = since_minutes.filter(|m| *m > 0) else {
        return segments;
    };
    let cutoff = chrono::Local::now() - chrono::Duration::minutes(mins);
    segments
        .into_iter()
        .filter(|seg| {
            chrono::DateTime::parse_from_rfc3339(&seg.started_at)
                .map(|d| d.with_timezone(&chrono::Local) >= cutoff)
                .unwrap_or(false)
        })
        .collect()
}

/// 时间线分页查询（command 唯一需要走 DB 的查询）
/// 仅返回非 idle 且满足最小时长的记录；相邻同应用同活动内容会合并
pub fn query_timeline(
    db: &Connection,
    date: NaiveDate,
    limit: i64,
    offset: i64,
    since_minutes: Option<i64>,
) -> Result<TimelinePage, rusqlite::Error> {
    let merged = filter_segments_since(query_timeline_merged_asc(db, date)?, since_minutes);
    let total = merged.len() as i64;
    let items: Vec<Segment> = merged
        .into_iter()
        .rev()
        .map(|mut seg| {
            seg.activity_label = Some(activity_key::activity_label_for_segment(&seg));
            seg
        })
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    Ok(TimelinePage { total, items })
}

/// 时间线分页结果
#[derive(Debug, Serialize)]
pub struct TimelinePage {
    pub total: i64,
    pub items: Vec<Segment>,
}

/// 三日时段热力图（24 个 1 小时桶 + AI 工作类型）
#[derive(Debug, Serialize)]
pub struct HeatmapDay {
    pub label: String,
    pub date: String,
    pub total_ms: u64,
    pub segment_count: u64,
    pub slots: [u64; 24],
    pub work_types: [Option<String>; 24],
    pub summaries: [Option<String>; 24],
}

/// 查询今天 / 昨天 / 前天（从最新往回）的时段热力图
pub fn query_three_day_heatmap(db: &Connection) -> Result<Vec<HeatmapDay>, rusqlite::Error> {
    let today = chrono::Local::now().date_naive();
    let labels = ["今天", "昨天", "前天"];
    let mut days = Vec::with_capacity(3);

    for (offset, label) in [0i64, 1, 2].into_iter().zip(labels.iter()) {
        let date = today - chrono::Duration::days(offset);
        days.push(build_heatmap_day(db, date, label.to_string())?);
    }
    Ok(days)
}

fn build_heatmap_day(
    db: &Connection,
    date: NaiveDate,
    label: String,
) -> Result<HeatmapDay, rusqlite::Error> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let mut slots = [0u64; 24];
    let mut total_ms = 0u64;
    let mut segment_count = 0u64;

    let mut stmt = db.prepare(
        "SELECT started_at, duration_ms FROM activity_segments \
         WHERE substr(started_at, 1, 10) = ?1 AND is_idle = 0 \
         ORDER BY started_at DESC",
    )?;
    let rows = stmt.query_map([&date_str], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
    })?;

    for row in rows {
        let (started, ms) = row?;
        total_ms += ms;
        segment_count += 1;
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&started) {
            let h = dt.with_timezone(&chrono::Local).hour() as usize;
            if h < 24 {
                slots[h] += ms;
            }
        }
    }

    let hour_types = crate::db::periods::load_hour_types_for_date(db, date)?;
    let mut work_types: [Option<String>; 24] = std::array::from_fn(|_| None);
    let mut summaries: [Option<String>; 24] = std::array::from_fn(|_| None);
    for (i, h) in hour_types.iter().enumerate() {
        if let Some(wt) = h {
            work_types[i] = Some(wt.work_type.clone());
            summaries[i] = Some(wt.summary.clone());
        }
    }

    Ok(HeatmapDay {
        label,
        date: date_str,
        total_ms,
        segment_count,
        slots,
        work_types,
        summaries,
    })
}

/// 查询某 aggregation_key 最近使用的 exe 路径（用于图标）
pub fn latest_exe_path_for_key(db: &Connection, key: &str) -> Option<String> {
    db.query_row(
        "SELECT exe_path FROM activity_segments \
         WHERE aggregation_key = ?1 AND exe_path != '' \
         ORDER BY started_at DESC LIMIT 1",
        [key],
        |row| row.get(0),
    )
    .ok()
}

/// 批量查询各 aggregation_key 最近 exe 路径（一次 SQL，避免 N+1）
pub fn latest_exe_paths_for_keys(
    db: &Connection,
    keys: &[String],
) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    if keys.is_empty() {
        return HashMap::new();
    }
    let placeholders = (0..keys.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT aggregation_key, exe_path FROM activity_segments \
         WHERE aggregation_key IN ({placeholders}) AND exe_path != '' \
         ORDER BY started_at DESC"
    );
    let params: Vec<&dyn rusqlite::ToSql> = keys
        .iter()
        .map(|k| k as &dyn rusqlite::ToSql)
        .collect();
    let mut map = HashMap::with_capacity(keys.len());
    if let Ok(mut stmt) = db.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                map.entry(row.0).or_insert(row.1);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rusqlite::Connection;

    fn create_test_segments_table(db: &Connection) {
        db.execute_batch(
            "CREATE TABLE activity_segments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                app_name TEXT NOT NULL,
                exe_path TEXT NOT NULL,
                window_title TEXT NOT NULL DEFAULT '',
                is_idle INTEGER NOT NULL DEFAULT 0,
                aggregation_key TEXT NOT NULL,
                source_type TEXT NOT NULL DEFAULT 'foreground',
                audio_activity TEXT NOT NULL DEFAULT ''
            );",
        )
        .unwrap();
    }

    fn fixture_segment(started: &str, ended: &str, ms: u64, key: &str, idle: bool) -> Segment {
        Segment {
            started_at: started.into(),
            ended_at: Some(ended.into()),
            duration_ms: ms,
            app_name: key.into(),
            exe_path: key.into(),
            window_title: String::new(),
            is_idle: idle,
            aggregation_key: key.into(),
            icon: None,
            source_type: "foreground".into(),
            audio_activity: String::new(),
            activity_label: None,
        }
    }

    #[test]
    fn aggregator_skips_idle_segments() {
        let mut agg = TodayAggregator::default();
        agg.apply(&fixture_segment(
            "2026-07-02T10:00:00+08:00",
            "2026-07-02T10:05:00+08:00",
            300_000,
            "notepad.exe",
            true,
        ));
        assert_eq!(agg.total_ms, 0);
        assert_eq!(agg.switch_count, 0);
    }

    #[test]
    fn aggregator_rebuild_from_db() {
        let db = Connection::open_in_memory().unwrap();
        create_test_segments_table(&db);
        db.execute(
            "INSERT INTO activity_segments \
             (started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key) \
             VALUES (?1, ?2, ?3, ?4, ?5, '', 0, ?6)",
            rusqlite::params![
                "2026-07-02T10:00:00+08:00",
                "2026-07-02T10:10:00+08:00",
                600_000i64,
                "notepad",
                "notepad.exe",
                "notepad.exe",
            ],
        )
        .unwrap();

        let agg = rebuild_aggregator(&db, NaiveDate::from_ymd_opt(2026, 7, 2).unwrap()).unwrap();
        assert_eq!(agg.total_ms, 600_000);
        assert_eq!(agg.switch_count, 1);
        assert_eq!(agg.app_breakdown.get("notepad.exe"), Some(&600_000));
        assert_eq!(agg.hourly[10], 600_000);
    }

    #[test]
    fn timeline_excludes_short_and_idle_segments() {
        let db = Connection::open_in_memory().unwrap();
        create_test_segments_table(&db);

        let insert = |started: &str, ms: i64, app: &str, idle: i64| {
            db.execute(
                "INSERT INTO activity_segments \
                 (started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key) \
                 VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, ?7)",
                rusqlite::params![
                    started,
                    "2026-07-02T10:30:00+08:00",
                    ms,
                    app,
                    format!("{app}.exe"),
                    idle,
                    format!("{app}.exe"),
                ],
            )
            .unwrap();
        };

        insert("2026-07-02T10:00:00+08:00", 30_000, "quick", 0); // 30s — 过滤
        insert("2026-07-02T10:05:00+08:00", 90_000, "vscode", 0); // 90s — 保留
        insert("2026-07-02T10:20:00+08:00", 120_000, "idle", 1); // idle — 过滤

        let page =
            query_timeline(&db, NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(), 50, 0, None).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].app_name, "vscode");
        assert_eq!(page.items[0].duration_ms, 90_000);
    }

    #[test]
    fn timeline_merges_consecutive_same_app_and_title() {
        let db = Connection::open_in_memory().unwrap();
        create_test_segments_table(&db);

        let insert =
            |started: &str, ended: &str, ms: i64, app: &str, title: &str| {
                db.execute(
                    "INSERT INTO activity_segments \
                     (started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
                    rusqlite::params![
                        started,
                        ended,
                        ms,
                        app,
                        format!("{app}.exe"),
                        title,
                        format!("{app}.exe"),
                    ],
                )
                .unwrap();
            };

        insert(
            "2026-07-02T10:00:00+08:00",
            "2026-07-02T10:05:00+08:00",
            300_000,
            "Cursor",
            "main.rs",
        );
        insert(
            "2026-07-02T10:06:00+08:00",
            "2026-07-02T10:12:00+08:00",
            360_000,
            "Cursor",
            "main.rs",
        );
        insert(
            "2026-07-02T10:20:00+08:00",
            "2026-07-02T10:30:00+08:00",
            600_000,
            "chrome",
            "docs",
        );

        let page =
            query_timeline(&db, NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(), 50, 0, None).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].app_name, "chrome");
        assert_eq!(page.items[1].app_name, "Cursor");
        assert_eq!(page.items[1].window_title, "main.rs");
        assert_eq!(page.items[1].duration_ms, 660_000);
        assert_eq!(
            page.items[1].started_at,
            "2026-07-02T10:00:00+08:00"
        );
        assert_eq!(
            page.items[1].ended_at.as_deref(),
            Some("2026-07-02T10:12:00+08:00")
        );
    }

    #[test]
    fn timeline_splits_same_app_different_projects() {
        let db = Connection::open_in_memory().unwrap();
        create_test_segments_table(&db);

        let insert =
            |started: &str, ended: &str, ms: i64, title: &str| {
                db.execute(
                    "INSERT INTO activity_segments \
                     (started_at, ended_at, duration_ms, app_name, exe_path, window_title, is_idle, aggregation_key) \
                     VALUES (?1, ?2, ?3, 'Cursor', 'cursor.exe', ?4, 0, 'cursor.exe')",
                    rusqlite::params![started, ended, ms, title],
                )
                .unwrap();
            };

        insert(
            "2026-07-02T10:00:00+08:00",
            "2026-07-02T10:10:00+08:00",
            600_000,
            "main.rs - PROJECT_A - Cursor",
        );
        insert(
            "2026-07-02T10:11:00+08:00",
            "2026-07-02T10:25:00+08:00",
            840_000,
            "index.ts - PROJECT_B - Cursor",
        );
        insert(
            "2026-07-02T10:26:00+08:00",
            "2026-07-02T10:40:00+08:00",
            840_000,
            "lib.rs - PROJECT_A - Cursor",
        );

        let page =
            query_timeline(&db, NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(), 50, 0, None).unwrap();
        assert_eq!(page.total, 3, "PROJECT_A / PROJECT_B / 再次 PROJECT_A 应各一条");
        assert_eq!(page.items.len(), 3);
    }
}
