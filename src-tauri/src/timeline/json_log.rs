//! 时间线 AI 调用审计：原数据 + 回复写入 JSON
//!
//! - **当日**：仍按批次写入 `{date}/{HHMMSS}-off{n}-n{m}.json`（逻辑不变）
//! - **历史日**：启动时合并为 `{date}/day.json`，去重后删除零散批次文件

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

use super::SegmentContext;

pub const DAY_CONSOLIDATED_FILENAME: &str = "day.json";
const DAY_LOG_VERSION: u32 = 1;

pub fn logs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("timeline-ai")
}

pub fn ensure_logs_dir(data_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = logs_dir(data_dir);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineAiRequestLog {
    pub vendor_id: String,
    pub vendor_name: String,
    pub model: String,
    pub persona_id: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub work_type_options: Vec<String>,
    /// 发送给模型的片段原数据
    pub segments: Vec<SegmentContext>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineAiParsedItem {
    pub id: String,
    pub work_type: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineAiResponseLog {
    pub used_ai: bool,
    /// 模型原始回复全文
    pub raw: Option<String>,
    pub parsed: Option<Vec<TimelineAiParsedItem>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineAiFinalizedLog {
    pub started_at: String,
    pub cache_key: String,
    pub work_type: String,
    pub summary: String,
    pub used_ai: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineAiBatchLog {
    pub created_at: String,
    pub date: String,
    pub limit: i64,
    pub offset: i64,
    pub batch_size: usize,
    pub request: TimelineAiRequestLog,
    pub response: TimelineAiResponseLog,
    pub finalized: Vec<TimelineAiFinalizedLog>,
}

/// 合并后的单日摘要（仅保留有用字段，不含完整 prompt / segments）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineAiDayLog {
    pub version: u32,
    pub date: String,
    pub consolidated_at: String,
    pub source_file_count: usize,
    pub entries: Vec<TimelineAiDayEntry>,
    #[serde(default)]
    pub batches: Vec<TimelineAiBatchSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimelineAiDayEntry {
    pub started_at: String,
    pub cache_key: String,
    pub work_type: String,
    pub summary: String,
    pub used_ai: bool,
    pub last_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineAiBatchSummary {
    pub created_at: String,
    pub offset: i64,
    pub batch_size: usize,
    pub vendor_id: String,
    pub model: String,
    pub used_ai: bool,
    pub error: Option<String>,
    pub finalized_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ConsolidateDayResult {
    pub date: String,
    pub source_files: usize,
    pub entries_kept: usize,
    pub entries_dropped: usize,
    pub consolidated: bool,
}

struct ParsedBatch {
    created_at: String,
    offset: i64,
    batch_size: usize,
    vendor_id: String,
    model: String,
    used_ai: bool,
    error: Option<String>,
    finalized: Vec<TimelineAiFinalizedLog>,
}

pub fn save_batch_log(
    data_dir: &Path,
    date: &str,
    limit: i64,
    offset: i64,
    request: TimelineAiRequestLog,
    response: TimelineAiResponseLog,
    finalized: Vec<TimelineAiFinalizedLog>,
) -> Result<PathBuf, String> {
    ensure_logs_dir(data_dir).map_err(|e| e.to_string())?;
    let day_dir = logs_dir(data_dir).join(date);
    fs::create_dir_all(&day_dir).map_err(|e| e.to_string())?;

    let now = Local::now();
    let created_at = now.to_rfc3339();
    let batch_size = request.segments.len();
    let record = TimelineAiBatchLog {
        created_at: created_at.clone(),
        date: date.to_string(),
        limit,
        offset,
        batch_size,
        request,
        response,
        finalized,
    };

    let stamp = format!(
        "{}-{:03}",
        now.format("%H%M%S"),
        now.timestamp_subsec_millis()
    );
    let name = format!("{stamp}-off{offset}-n{batch_size}.json");
    let path = day_dir.join(name);
    let json = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

/// 启动时整理所有「非今日」的 timeline-ai 目录（含前一日与更早历史）
pub fn consolidate_past_days_on_startup(data_dir: &Path) -> Result<Vec<ConsolidateDayResult>, String> {
    let today = Local::now().date_naive();
    let root = logs_dir(data_dir);
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut dates: Vec<String> = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| parse_date_dir(name).is_some())
        .collect();
    dates.sort();

    let mut results = Vec::new();
    for date in dates {
        let Some(day) = parse_date_dir(&date) else {
            continue;
        };
        if day >= today {
            continue;
        }
        match consolidate_day(data_dir, &date) {
            Ok(r) if r.consolidated => {
                eprintln!(
                    "xiaohan-daily: timeline-ai consolidated {} ({} files → {} entries)",
                    r.date, r.source_files, r.entries_kept
                );
                results.push(r);
            }
            Ok(_) => {}
            Err(e) => eprintln!("xiaohan-daily: timeline-ai consolidate {date} failed: {e}"),
        }
    }
    Ok(results)
}

/// 合并指定日期的零散批次 JSON 为 `day.json`（不处理今日）
pub fn consolidate_day(data_dir: &Path, date: &str) -> Result<ConsolidateDayResult, String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    if date == today {
        return Ok(ConsolidateDayResult {
            date: date.to_string(),
            ..Default::default()
        });
    }

    let day_dir = logs_dir(data_dir).join(date);
    if !day_dir.is_dir() {
        return Ok(ConsolidateDayResult {
            date: date.to_string(),
            ..Default::default()
        });
    }

    let loose_files = list_loose_batch_files(&day_dir)?;
    if loose_files.is_empty() {
        return Ok(ConsolidateDayResult {
            date: date.to_string(),
            ..Default::default()
        });
    }

    let mut entries = Vec::new();
    let mut batches = Vec::new();
    let loose_count = loose_files.len();

    let day_path = day_dir.join(DAY_CONSOLIDATED_FILENAME);
    if day_path.is_file() {
        if let Ok(existing) = read_day_log(&day_path) {
            entries.extend(existing.entries);
            batches.extend(existing.batches);
        }
    }

    for path in &loose_files {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let Some(batch) = parse_batch_file(&raw) else {
            continue;
        };
        batches.push(TimelineAiBatchSummary {
            created_at: batch.created_at.clone(),
            offset: batch.offset,
            batch_size: batch.batch_size,
            vendor_id: batch.vendor_id,
            model: batch.model,
            used_ai: batch.used_ai,
            error: batch.error,
            finalized_count: batch.finalized.len(),
        });
        for item in batch.finalized {
            entries.push(finalized_to_day_entry(&item, &batch.created_at));
        }
    }

    let before = entries.len();
    let entries = merge_and_dedupe_entries(entries);
    let entries_kept = entries.len();
    let entries_dropped = before.saturating_sub(entries_kept);

    batches.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    batches.dedup_by(|a, b| {
        a.created_at == b.created_at && a.offset == b.offset && a.batch_size == b.batch_size
    });

    let day_log = TimelineAiDayLog {
        version: DAY_LOG_VERSION,
        date: date.to_string(),
        consolidated_at: Local::now().to_rfc3339(),
        source_file_count: loose_count,
        entries,
        batches,
    };

    let json = serde_json::to_string_pretty(&day_log).map_err(|e| e.to_string())?;
    fs::write(&day_path, json).map_err(|e| e.to_string())?;

    for path in loose_files {
        let _ = fs::remove_file(path);
    }

    Ok(ConsolidateDayResult {
        date: date.to_string(),
        source_files: loose_count,
        entries_kept,
        entries_dropped,
        consolidated: true,
    })
}

fn list_loose_batch_files(day_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for entry in fs::read_dir(day_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".json") || name == DAY_CONSOLIDATED_FILENAME {
            continue;
        }
        out.push(path);
    }
    out.sort();
    Ok(out)
}

fn read_day_log(path: &Path) -> Result<TimelineAiDayLog, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn parse_batch_file(raw: &str) -> Option<ParsedBatch> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let created_at = v.get("created_at")?.as_str()?.to_string();
    let offset = v.get("offset").and_then(|x| x.as_i64()).unwrap_or(0);
    let batch_size = v
        .get("batch_size")
        .and_then(|x| x.as_u64())
        .unwrap_or(0) as usize;
    let request = v.get("request");
    let response = v.get("response");
    let vendor_id = request
        .and_then(|r| r.get("vendor_id"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let model = request
        .and_then(|r| r.get("model"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let used_ai = response
        .and_then(|r| r.get("used_ai"))
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let error = response
        .and_then(|r| r.get("error"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());
    let finalized: Vec<TimelineAiFinalizedLog> = v
        .get("finalized")
        .and_then(|f| serde_json::from_value(f.clone()).ok())
        .unwrap_or_default();
    Some(ParsedBatch {
        created_at,
        offset,
        batch_size,
        vendor_id,
        model,
        used_ai,
        error,
        finalized,
    })
}

fn finalized_to_day_entry(item: &TimelineAiFinalizedLog, batch_created_at: &str) -> TimelineAiDayEntry {
    TimelineAiDayEntry {
        started_at: item.started_at.clone(),
        cache_key: item.cache_key.clone(),
        work_type: item.work_type.clone(),
        summary: item.summary.clone(),
        used_ai: item.used_ai,
        last_updated_at: batch_created_at.to_string(),
    }
}

fn is_useful_summary(summary: &str) -> bool {
    let t = summary.trim();
    !t.is_empty() && !crate::tracker::activity_key::is_machine_summary(t)
}

fn merge_and_dedupe_entries(items: Vec<TimelineAiDayEntry>) -> Vec<TimelineAiDayEntry> {
    let mut by_cache_key: HashMap<String, TimelineAiDayEntry> = HashMap::new();
    for entry in items {
        if !is_useful_summary(&entry.summary) {
            continue;
        }
        match by_cache_key.get(&entry.cache_key) {
            None => {
                by_cache_key.insert(entry.cache_key.clone(), entry);
            }
            Some(existing) => {
                let prefer_new = entry.last_updated_at > existing.last_updated_at
                    || (entry.last_updated_at == existing.last_updated_at && entry.used_ai && !existing.used_ai);
                if prefer_new {
                    by_cache_key.insert(entry.cache_key.clone(), entry);
                }
            }
        }
    }

    let mut out: Vec<TimelineAiDayEntry> = by_cache_key.into_values().collect();
    out.sort_by(|a, b| a.started_at.cmp(&b.started_at).then_with(|| a.cache_key.cmp(&b.cache_key)));

    // 同质化：同一时段 + 相同简介 + 相同工作类型只保留最优一条
    let mut best_by_triple: HashMap<(String, String, String), TimelineAiDayEntry> = HashMap::new();
    for entry in out {
        let key = (
            entry.started_at.clone(),
            entry.summary.clone(),
            entry.work_type.clone(),
        );
        match best_by_triple.get(&key) {
            None => {
                best_by_triple.insert(key, entry);
            }
            Some(existing) => {
                let prefer = entry.used_ai && !existing.used_ai
                    || (entry.used_ai == existing.used_ai
                        && entry.last_updated_at > existing.last_updated_at);
                if prefer {
                    best_by_triple.insert(key, entry);
                }
            }
        }
    }

    let mut final_out: Vec<_> = best_by_triple.into_values().collect();
    final_out.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    final_out
}

fn parse_date_dir(name: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(name, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn write_loose_batch(base: &Path, date: &str, name: &str, finalized: Vec<TimelineAiFinalizedLog>) {
        let dir = logs_dir(base).join(date);
        fs::create_dir_all(&dir).unwrap();
        let batch = TimelineAiBatchLog {
            created_at: "2026-07-03T10:00:00+08:00".into(),
            date: date.into(),
            limit: 50,
            offset: 0,
            batch_size: finalized.len(),
            request: TimelineAiRequestLog {
                vendor_id: "ollama".into(),
                vendor_name: "Ollama".into(),
                model: "test".into(),
                persona_id: "default".into(),
                system_prompt: "sys".into(),
                user_prompt: "user".into(),
                work_type_options: vec!["开发".into()],
                segments: vec![],
            },
            response: TimelineAiResponseLog {
                used_ai: true,
                raw: Some("[]".into()),
                parsed: None,
                error: None,
            },
            finalized,
        };
        let json = serde_json::to_string_pretty(&batch).unwrap();
        fs::write(dir.join(name), json).unwrap();
    }

    #[test]
    fn save_log_file() {
        let base = env::temp_dir().join(format!("xiaohan-tl-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let date = "2026-07-02";
        let path = save_batch_log(
            &base,
            date,
            50,
            0,
            TimelineAiRequestLog {
                vendor_id: "ollama".into(),
                vendor_name: "Ollama".into(),
                model: "test".into(),
                persona_id: "default".into(),
                system_prompt: "sys".into(),
                user_prompt: "user".into(),
                work_type_options: vec!["开发".into()],
                segments: vec![],
            },
            TimelineAiResponseLog {
                used_ai: true,
                raw: Some("[]".into()),
                parsed: None,
                error: None,
            },
            vec![],
        )
        .unwrap();
        assert!(path.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn consolidate_day_merges_and_dedupes() {
        let base = env::temp_dir().join(format!("xiaohan-tl-cons-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let date = "2026-07-01";

        write_loose_batch(
            &base,
            date,
            "a.json",
            vec![TimelineAiFinalizedLog {
                started_at: "2026-07-01T09:00:00+08:00".into(),
                cache_key: "k1".into(),
                work_type: "开发".into(),
                summary: "写代码".into(),
                used_ai: true,
            }],
        );
        write_loose_batch(
            &base,
            date,
            "b.json",
            vec![
                TimelineAiFinalizedLog {
                    started_at: "2026-07-01T09:00:00+08:00".into(),
                    cache_key: "k1".into(),
                    work_type: "开发".into(),
                    summary: "写代码".into(),
                    used_ai: true,
                },
                TimelineAiFinalizedLog {
                    started_at: "2026-07-01T10:00:00+08:00".into(),
                    cache_key: "k2".into(),
                    work_type: "开发".into(),
                    summary: "写代码".into(),
                    used_ai: false,
                },
            ],
        );

        let result = consolidate_day(&base, date).unwrap();
        assert!(result.consolidated);
        assert_eq!(result.entries_kept, 2);
        assert!(list_loose_batch_files(&logs_dir(&base).join(date)).unwrap().is_empty());
        assert!(logs_dir(&base).join(date).join(DAY_CONSOLIDATED_FILENAME).is_file());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn consolidate_skips_today() {
        let base = env::temp_dir().join(format!("xiaohan-tl-today-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let today = Local::now().format("%Y-%m-%d").to_string();
        write_loose_batch(
            &base,
            &today,
            "live.json",
            vec![TimelineAiFinalizedLog {
                started_at: "x".into(),
                cache_key: "k".into(),
                work_type: "开发".into(),
                summary: "进行中".into(),
                used_ai: true,
            }],
        );

        let result = consolidate_day(&base, &today).unwrap();
        assert!(!result.consolidated);
        assert_eq!(list_loose_batch_files(&logs_dir(&base).join(&today)).unwrap().len(), 1);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dedupe_drops_empty_and_homogeneous() {
        let items = vec![
            TimelineAiDayEntry {
                started_at: "t1".into(),
                cache_key: "a".into(),
                work_type: "开发".into(),
                summary: "   ".into(),
                used_ai: false,
                last_updated_at: "1".into(),
            },
            TimelineAiDayEntry {
                started_at: "t1".into(),
                cache_key: "b".into(),
                work_type: "开发".into(),
                summary: "相同".into(),
                used_ai: false,
                last_updated_at: "1".into(),
            },
            TimelineAiDayEntry {
                started_at: "t1".into(),
                cache_key: "c".into(),
                work_type: "开发".into(),
                summary: "相同".into(),
                used_ai: true,
                last_updated_at: "2".into(),
            },
        ];
        let out = merge_and_dedupe_entries(items);
        assert_eq!(out.len(), 1);
        assert!(out[0].used_ai);
        assert_eq!(out[0].summary, "相同");
    }
}
