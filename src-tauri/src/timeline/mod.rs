//! 时间线 AI 简介生成

pub mod describe;
pub mod json_log;
pub mod scheduler;

use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;

use crate::db::{insights, stats, timeline_cache};
use crate::prompts;
use crate::tracker::{activity_key, context_enrich::SegmentEnrichment, display_name, Segment};
use crate::work_type::WorkTypeConfig;

#[derive(Debug, Clone, Serialize)]
pub struct TimelineDescribeChunkEvent {
    pub offset: i64,
    pub limit: i64,
    pub entries: Vec<timeline_cache::TimelineAiEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SegmentContext {
    pub id: String,
    pub cache_key: String,
    pub started_at: String,
    pub time_label: String,
    pub app_name: String,
    pub window_title: String,
    pub exe_path: String,
    pub aggregation_key: String,
    pub duration_ms: u64,
    /// foreground | audio
    pub source_type: String,
    /// music | video | chat | other | ""
    pub audio_activity: String,
    /// 从标题/路径解析的结构化上下文（Cursor 项目/会话、Edge 网页等）
    pub enrichment: SegmentEnrichment,
    pub insight_lines: Vec<String>,
    pub insights: Vec<insights::InsightDetail>,
    pub recent_app_insights: Vec<insights::InsightDetail>,
    pub period_hint: Option<String>,
    pub activity_key: String,
    pub activity_label: String,
    /// 同一应用内、本时段之前出现过的不同活动内容
    pub prior_activities: Vec<PriorActivityInApp>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PriorActivityInApp {
    pub activity_label: String,
    pub time_label: String,
}

fn build_all_contexts(
    db: &Connection,
    date: NaiveDate,
    since_minutes: Option<i64>,
) -> Result<Vec<SegmentContext>, String> {
    let merged = stats::filter_segments_since(
        stats::query_timeline_merged_asc(db, date).map_err(|e| e.to_string())?,
        since_minutes,
    );
    let mut history: HashMap<String, Vec<PriorActivityInApp>> = HashMap::new();
    let mut last_activity_key: HashMap<String, String> = HashMap::new();
    let mut all_contexts = Vec::with_capacity(merged.len());

    for seg in &merged {
        let prior = history
            .get(&seg.aggregation_key)
            .cloned()
            .unwrap_or_default();
        all_contexts.push(build_context(db, seg, prior));

        let ak = activity_key::activity_key_for_segment(seg);
        let label = activity_key::activity_label_for_segment(seg);
        if last_activity_key.get(&seg.aggregation_key) != Some(&ak) {
            history
                .entry(seg.aggregation_key.clone())
                .or_default()
                .push(PriorActivityInApp {
                    activity_label: label,
                    time_label: format_time_range(seg),
                });
            last_activity_key.insert(seg.aggregation_key.clone(), ak);
        }
    }

    all_contexts.reverse();
    Ok(all_contexts)
}

/// 今日（或指定日）所有尚未有效缓存的片段上下文
pub fn list_uncached_for_date(
    db: &Connection,
    date: NaiveDate,
) -> Result<Vec<SegmentContext>, String> {
    let all_contexts = build_all_contexts(db, date, None)?;
    if all_contexts.is_empty() {
        return Ok(Vec::new());
    }
    let keys: Vec<String> = all_contexts.iter().map(|c| c.cache_key.clone()).collect();
    let cached = timeline_cache::get_cached(db, &keys).map_err(|e| e.to_string())?;
    let cached_keys: std::collections::HashSet<_> = filter_valid_cache(cached)
        .into_iter()
        .map(|c| c.cache_key)
        .collect();
    Ok(all_contexts
        .into_iter()
        .filter(|c| !cached_keys.contains(&c.cache_key))
        .collect())
}

pub fn plan_describe(
    db: &Connection,
    date: NaiveDate,
    limit: i64,
    offset: i64,
    since_minutes: Option<i64>,
) -> Result<(Vec<SegmentContext>, Vec<timeline_cache::TimelineAiEntry>), String> {
    let all_contexts = build_all_contexts(db, date, since_minutes)?;
    let page_contexts: Vec<SegmentContext> = all_contexts
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    let keys: Vec<String> = page_contexts.iter().map(|c| c.cache_key.clone()).collect();
    let cached = timeline_cache::get_cached(db, &keys).map_err(|e| e.to_string())?;
    Ok((page_contexts, filter_valid_cache(cached)))
}

pub fn build_prompt(data_dir: &Path, work_types: &WorkTypeConfig, chunk: &[SegmentContext]) -> String {
    let segments_json =
        serde_json::to_string(&chunk_json(chunk)).unwrap_or_else(|_| "[]".into());
    let type_list = work_types.type_names().join("、");
    prompts::render(
        data_dir,
        "timeline-segment-describe",
        &[
            ("type_list", &type_list),
            ("segments_json", &segments_json),
        ],
    )
}

pub fn finalize_chunk(
    chunk: &[SegmentContext],
    ai_map: Option<&HashMap<String, (String, String)>>,
    work_types: &WorkTypeConfig,
) -> Vec<(SegmentContext, String, String, bool)> {
    chunk
        .iter()
        .map(|ctx| {
            if let Some(map) = ai_map {
                if let Some((w, s)) = map.get(&ctx.id) {
                    if !activity_key::is_machine_summary(s) {
                        return (
                            ctx.clone(),
                            work_types.normalize_type_name(w),
                            s.clone(),
                            true,
                        );
                    }
                }
            }
            let fb = fallback_entry(ctx, work_types);
            (ctx.clone(), fb.work_type, fb.summary, false)
        })
        .collect()
}

pub fn persist_entries(
    db: &Connection,
    rows: &[(SegmentContext, String, String, bool)],
) -> Result<Vec<timeline_cache::TimelineAiEntry>, String> {
    let now = chrono::Local::now().to_rfc3339();
    let mut out = Vec::with_capacity(rows.len());
    for (ctx, wt, summary, used_ai) in rows {
        timeline_cache::upsert_cache(
            db,
            &ctx.cache_key,
            &ctx.started_at,
            wt,
            summary,
            *used_ai,
            &now,
        )
        .map_err(|e| e.to_string())?;
        out.push(timeline_cache::TimelineAiEntry {
            cache_key: ctx.cache_key.clone(),
            started_at: ctx.started_at.clone(),
            work_type: wt.clone(),
            summary: summary.clone(),
            used_ai: *used_ai,
        });
    }
    Ok(out)
}

/// 过滤掉机器格式简介，避免旧 cache 被当作有效结果复用
pub fn filter_valid_cache(entries: Vec<timeline_cache::TimelineAiEntry>) -> Vec<timeline_cache::TimelineAiEntry> {
    entries
        .into_iter()
        .filter(|e| !activity_key::is_machine_summary(&e.summary))
        .collect()
}

pub fn parse_ai_map(raw: &str, chunk: &[SegmentContext]) -> Option<HashMap<String, (String, String)>> {
    let json_str = crate::ai::json_util::extract_json_array(raw);
    let v: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    let arr = v.as_array()?;
    let mut map = HashMap::new();
    for (i, item) in arr.iter().enumerate() {
        let wt = item["work_type"].as_str()?.to_string();
        let summary = item["summary"].as_str()?.trim().to_string();
        if summary.is_empty() || activity_key::is_machine_summary(&summary) {
            continue;
        }
        let id = resolve_ai_item_id(item, chunk, i)?;
        map.insert(id, (wt, summary));
    }
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// 模型常返回 "0"/"1" 或与 started_at 不一致的 id，按序号回退到 chunk
fn resolve_ai_item_id(
    item: &serde_json::Value,
    chunk: &[SegmentContext],
    index: usize,
) -> Option<String> {
    if let Some(id) = item["id"].as_str() {
        let id = id.trim();
        if chunk.iter().any(|c| c.id == id) {
            return Some(id.to_string());
        }
        if let Ok(idx) = id.parse::<usize>() {
            if idx < chunk.len() {
                return Some(chunk[idx].id.clone());
            }
        }
    }
    chunk.get(index).map(|c| c.id.clone())
}

fn build_context(
    db: &Connection,
    seg: &Segment,
    prior_activities: Vec<PriorActivityInApp>,
) -> SegmentContext {
    let ended = seg.ended_at.as_deref().unwrap_or(&seg.started_at);
    let act_key = activity_key::activity_key_for_segment(seg);
    let act_label = activity_key::activity_label_for_segment(seg);
    let key = timeline_cache::cache_key(
        &seg.started_at,
        ended,
        &seg.aggregation_key,
        &act_key,
    );
    let app = display_name::friendly_name(&seg.exe_path, &seg.app_name, &seg.window_title);
    let enrichment = crate::tracker::context_enrich::enrich_segment(seg);

    let insight_list =
        insights::insights_for_segment(db, &seg.started_at, ended).unwrap_or_default();
    let insight_lines: Vec<String> = insight_list
        .iter()
        .map(|i| format!("[{}] {}", i.source, i.summary.trim()))
        .collect();

    let recent_app_insights =
        insights::recent_for_app(db, &seg.aggregation_key, &seg.started_at, 3)
            .unwrap_or_default();

    let period_hint = periods_hint(db, &seg.started_at, ended);

    SegmentContext {
        id: seg.started_at.clone(),
        cache_key: key,
        started_at: seg.started_at.clone(),
        time_label: format_time_range(seg),
        app_name: app,
        window_title: seg.window_title.clone(),
        exe_path: seg.exe_path.clone(),
        aggregation_key: seg.aggregation_key.clone(),
        duration_ms: seg.duration_ms,
        source_type: seg.source_type.clone(),
        audio_activity: seg.audio_activity.clone(),
        enrichment,
        insight_lines,
        insights: insight_list,
        recent_app_insights,
        period_hint,
        activity_key: act_key,
        activity_label: act_label,
        prior_activities,
    }
}

fn periods_hint(db: &Connection, start: &str, end: &str) -> Option<String> {
    let date_from = &start[..10.min(start.len())];
    let items = crate::db::periods::list_period_summaries_in_range(db, date_from, date_from, 50)
        .ok()?;
    let t0 = chrono::DateTime::parse_from_rfc3339(start).ok()?;
    let t1 = chrono::DateTime::parse_from_rfc3339(end).ok()?;
    for p in items {
        let ps = chrono::DateTime::parse_from_rfc3339(&p.started_at).ok()?;
        let pe = chrono::DateTime::parse_from_rfc3339(&p.ended_at).ok()?;
        if (t0 >= ps && t0 <= pe) || (t1 >= ps && t1 <= pe) {
            return Some(format!("【{}】{}", p.work_type, p.summary));
        }
    }
    None
}

fn chunk_json(chunk: &[SegmentContext]) -> Vec<serde_json::Value> {
    chunk
        .iter()
        .map(|c| {
            let recent: Vec<_> = c
                .recent_app_insights
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "source": i.source,
                        "category": i.category,
                        "summary": i.summary,
                        "window_title": i.window_title,
                    })
                })
                .collect();
            serde_json::json!({
                "id": c.id,
                "time": c.time_label,
                "duration": format_duration(c.duration_ms),
                "app": c.app_name,
                "window_title": c.window_title,
                "exe_path": c.exe_path,
                "aggregation_key": c.aggregation_key,
                "source_type": c.source_type,
                "audio_activity": c.audio_activity,
                "activity_key": c.activity_key,
                "activity_label": c.activity_label,
                "prior_activities_in_app": c.prior_activities,
                "app_kind": c.enrichment.app_kind,
                "parsed_context": c.enrichment.fields,
                "context_hints": c.enrichment.hints,
                "hybrid_insights": c.insight_lines,
                "recent_same_app_insights": recent,
                "period_hint": c.period_hint,
            })
        })
        .collect()
}

struct FallbackEntry {
    work_type: String,
    summary: String,
}

fn fallback_entry(ctx: &SegmentContext, work_types: &WorkTypeConfig) -> FallbackEntry {
    if ctx.source_type == "audio" {
        let activity = match ctx.audio_activity.as_str() {
            "music" => "听歌",
            "video" => "看视频",
            "chat" => "聊天通话",
            _ => "音频",
        };
        let title = ctx.window_title.trim();
        let summary = if title.is_empty() || title.starts_with("后台") {
            format!(
                "后台在 {} {}，大约{}。",
                ctx.app_name,
                activity,
                format_duration(ctx.duration_ms)
            )
        } else {
            format!(
                "后台{} · {} ·「{}」，大约{}。",
                activity,
                ctx.app_name,
                title.chars().take(40).collect::<String>(),
                format_duration(ctx.duration_ms)
            )
        };
        let wt = match ctx.audio_activity.as_str() {
            "music" | "video" => "其他",
            "chat" => "沟通",
            _ => guess_type(&ctx.app_name),
        };
        return FallbackEntry {
            work_type: work_types.normalize_type_name(wt),
            summary,
        };
    }
    if let Some(hint) = &ctx.period_hint {
        if let Some((wt, rest)) = hint.split_once('】') {
            let wt = wt.trim_start_matches('【').trim();
            return FallbackEntry {
                work_type: work_types.normalize_type_name(wt),
                summary: rest.to_string(),
            };
        }
    }
    let human = activity_key::human_summary_from_enrichment(&ctx.enrichment, &ctx.app_name);
    if !human.is_empty() && human != format!("在使用 {}", ctx.app_name) {
        return FallbackEntry {
            work_type: work_types.normalize_type_name(guess_type_from_enrichment(&ctx.enrichment)),
            summary: human,
        };
    }
    if !ctx.activity_label.is_empty() && ctx.activity_label != ctx.app_name {
        return FallbackEntry {
            work_type: work_types.normalize_type_name(guess_type_from_enrichment(&ctx.enrichment)),
            summary: format!("在 {} · {}", ctx.app_name, ctx.activity_label),
        };
    }
    if let Some(hint) = ctx.enrichment.hints.iter().find(|h| !h.starts_with("应用：")) {
        return FallbackEntry {
            work_type: work_types.normalize_type_name(guess_type_from_enrichment(&ctx.enrichment)),
            summary: hint.clone(),
        };
    }
    let title = ctx.window_title.trim();
    let summary = if title.is_empty() {
        format!(
            "在 {} 待了{}。",
            ctx.app_name,
            format_duration(ctx.duration_ms)
        )
    } else {
        format!(
            "在 {} 里看「{}」，大约{}。",
            ctx.app_name,
            title.chars().take(40).collect::<String>(),
            format_duration(ctx.duration_ms)
        )
    };
    FallbackEntry {
        work_type: work_types.normalize_type_name(guess_type(&ctx.app_name)),
        summary,
    }
}

fn guess_type(app: &str) -> &str {
    let a = app.to_lowercase();
    if a.contains("cursor") || a.contains("code") || a.contains("rust") {
        "开发"
    } else if a.contains("微信") || a.contains("teams") || a.contains("slack") {
        "沟通"
    } else {
        "其他"
    }
}

fn guess_type_from_enrichment(e: &SegmentEnrichment) -> &str {
    match e.app_kind.as_str() {
        "cursor" | "ide" | "terminal" => "开发",
        "browser" => "文档",
        "chat" => "沟通",
        _ => "其他",
    }
}

fn format_time_range(seg: &Segment) -> String {
    let start = fmt_clock(&seg.started_at);
    let end = seg.ended_at.as_deref().map(fmt_clock);
    match end {
        Some(e) if e != start => format!("{start}–{e}"),
        _ => start,
    }
}

fn fmt_clock(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|_| iso.chars().skip(11).take(5).collect())
}

fn format_duration(ms: u64) -> String {
    let mins = ms / 60_000;
    if mins >= 60 {
        format!("{}小时{}分", mins / 60, mins % 60)
    } else if mins > 0 {
        format!("{mins}分钟")
    } else {
        "一会儿".into()
    }
}

pub fn parse_timeline_date(date: Option<&str>) -> Result<NaiveDate, String> {
    match date {
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| "日期格式无效".into()),
        None => Ok(chrono::Local::now().date_naive()),
    }
}

pub fn ai_map_to_parsed(map: &HashMap<String, (String, String)>) -> Vec<json_log::TimelineAiParsedItem> {
    let mut items: Vec<_> = map
        .iter()
        .map(|(id, (wt, summary))| json_log::TimelineAiParsedItem {
            id: id.clone(),
            work_type: wt.clone(),
            summary: summary.clone(),
        })
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

pub fn finalized_to_log(
    rows: &[(SegmentContext, String, String, bool)],
) -> Vec<json_log::TimelineAiFinalizedLog> {
    rows.iter()
        .map(|(ctx, wt, summary, used_ai)| json_log::TimelineAiFinalizedLog {
            started_at: ctx.started_at.clone(),
            cache_key: ctx.cache_key.clone(),
            work_type: wt.clone(),
            summary: summary.clone(),
            used_ai: *used_ai,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(id: &str) -> SegmentContext {
        SegmentContext {
            id: id.into(),
            cache_key: format!("key-{id}"),
            started_at: id.into(),
            time_label: "10:00".into(),
            app_name: "Cursor".into(),
            window_title: String::new(),
            exe_path: String::new(),
            aggregation_key: String::new(),
            duration_ms: 60_000,
            source_type: "foreground".into(),
            audio_activity: String::new(),
            enrichment: Default::default(),
            insight_lines: vec![],
            insights: vec![],
            recent_app_insights: vec![],
            period_hint: None,
            activity_key: String::new(),
            activity_label: String::new(),
            prior_activities: vec![],
        }
    }

    #[test]
    fn parse_ai_map_numeric_id_fallback() {
        let chunk = vec![ctx("2026-07-03T10:00:00+08:00"), ctx("2026-07-03T11:00:00+08:00")];
        let raw = r#"[{"id":"0","work_type":"开发","summary":"在改桌宠计划呢~"},{"id":"1","work_type":"文档","summary":"查文档中"}]"#;
        let map = parse_ai_map(raw, &chunk).unwrap();
        assert_eq!(
            map.get("2026-07-03T10:00:00+08:00").map(|(_, s)| s.as_str()),
            Some("在改桌宠计划呢~")
        );
    }

    #[test]
    fn parse_ai_map_rejects_machine_summary() {
        let chunk = vec![ctx("2026-07-03T10:00:00+08:00")];
        let raw = r#"[{"id":"0","work_type":"开发","summary":"开发：foo.plan.md - Cursor · 窗口「foo」"}]"#;
        assert!(parse_ai_map(raw, &chunk).is_none());
    }
}
