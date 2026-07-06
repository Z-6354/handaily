//! 报告生成：时段总结 / 完成记录

use std::path::Path;

use chrono::{Datelike, NaiveDate};

use crate::ai::{catalog::VendorCatalog, config::AiConfig, PreparedTextChat};
use crate::ai::json_util;
use crate::db::{periods, reports, stats, timeline_cache};
use crate::prompts;
use crate::tracker::{activity_key, display_name};
use crate::vault::VaultState;

pub const TEMPLATE_PERIOD_SUMMARY: &str = "period-summary";
pub const TEMPLATE_ACTIVITY_LOG: &str = "activity-log";

#[derive(Debug, Clone)]
pub struct ReportDraft {
    pub title: String,
    pub content: String,
    pub used_ai: bool,
}

/// 短临界区：只读 DB 收集原始数据
#[derive(Debug, Clone)]
pub struct ReportGathered {
    pub template_id: String,
    pub range_label: String,
    pub period_items: Vec<periods::PeriodSummaryPublic>,
    pub timeline_lines: String,
    pub app_lines: String,
    pub metrics: MetricsTuple,
}

pub fn gather(
    db: &rusqlite::Connection,
    template_id: &str,
    date_from: &str,
    date_to: &str,
) -> Result<ReportGathered, String> {
    validate_dates(date_from, date_to)?;
    let range_label = format_date_range_label(date_from, date_to);
    match template_id {
        TEMPLATE_PERIOD_SUMMARY => {
            let period_items =
                periods::list_period_summaries_in_range(db, date_from, date_to, 80)
                    .map_err(|e| e.to_string())?;
            let timeline_lines = if period_items.is_empty() {
                collect_timeline_lines(db, date_from, date_to)?
            } else {
                String::new()
            };
            Ok(ReportGathered {
                template_id: template_id.to_string(),
                range_label,
                period_items,
                timeline_lines,
                app_lines: String::new(),
                metrics: ("0".into(), "0".into(), "0".into(), "0".into()),
            })
        }
        TEMPLATE_ACTIVITY_LOG => Ok(ReportGathered {
            template_id: template_id.to_string(),
            range_label,
            period_items: Vec::new(),
            timeline_lines: collect_timeline_lines(db, date_from, date_to)?,
            app_lines: collect_app_lines(db, date_from, date_to)?,
            metrics: collect_metrics_line(db, date_from, date_to)?,
        }),
        _ => Err(format!("未知报告模板: {template_id}")),
    }
}

/// 构建 AI user prompt；无文本模型时返回 None
pub fn build_ai_user_prompt(data_dir: &Path, gathered: &ReportGathered) -> String {
    match gathered.template_id.as_str() {
        TEMPLATE_PERIOD_SUMMARY => {
            let period_lines = format_period_lines(&gathered.period_items);
            let timeline_section = if gathered.period_items.is_empty()
                && !gathered.timeline_lines.is_empty()
                && !gathered.timeline_lines.starts_with('（')
            {
                format!(
                    "时间线片段（时段小结为空时的补充）：\n{}",
                    gathered.timeline_lines
                )
            } else {
                String::new()
            };
            prompts::render(
                data_dir,
                "report-period-summary",
                &[
                    ("date_range", &gathered.range_label),
                    ("period_lines", &period_lines),
                    ("timeline_section", &timeline_section),
                ],
            )
        }
        TEMPLATE_ACTIVITY_LOG => prompts::render(
            data_dir,
            "report-activity-log",
            &[
                ("date_range", &gathered.range_label),
                ("app_lines", &gathered.app_lines),
                ("timeline_lines", &gathered.timeline_lines),
                ("mouse_clicks", &gathered.metrics.0),
                ("key_strokes", &gathered.metrics.1),
                ("files_created", &gathered.metrics.2),
                ("files_modified", &gathered.metrics.3),
            ],
        ),
        _ => String::new(),
    }
}

pub fn compose(gathered: &ReportGathered, ai_content: Option<String>) -> ReportDraft {
    let title = match gathered.template_id.as_str() {
        TEMPLATE_PERIOD_SUMMARY => format!("✨ 时段小记 · {}", gathered.range_label),
        _ => format!("📝 完成记录 · {}", gathered.range_label),
    };

    if let Some(content) = ai_content.filter(|s| !s.trim().is_empty()) {
        return ReportDraft {
            title,
            content: sanitize_report_markdown(&content),
            used_ai: true,
        };
    }

    let content = match gathered.template_id.as_str() {
        TEMPLATE_PERIOD_SUMMARY => {
            format_period_local(&gathered.range_label, &gathered.period_items)
        }
        _ => format_activity_local(
            &gathered.range_label,
            &gathered.timeline_lines,
            &gathered.app_lines,
            &gathered.metrics,
        ),
    };

    ReportDraft {
        title,
        content,
        used_ai: false,
    }
}

pub fn prepare_ai_chat(
    gathered: &ReportGathered,
    config: &AiConfig,
    catalog: &VendorCatalog,
    vault: &VaultState,
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<Option<PreparedTextChat>, String> {
    if config.text_model.trim().is_empty() {
        return Ok(None);
    }
    let prompt = build_ai_user_prompt(data_dir, gathered);
    match PreparedTextChat::prepare(config, catalog, vault, db, data_dir, prompt) {
        Ok(opt) => Ok(opt),
        Err(e) => {
            eprintln!("xiaohan-daily: report AI prep skipped: {e}");
            Ok(None)
        }
    }
}

pub fn save_generated(
    db: &rusqlite::Connection,
    template_id: &str,
    draft: &ReportDraft,
    date_from: &str,
    date_to: &str,
) -> Result<i64, String> {
    let now = chrono::Local::now().to_rfc3339();
    reports::insert_report(
        db,
        template_id,
        &draft.title,
        date_from,
        date_to,
        &draft.content,
        draft.used_ai,
        &now,
    )
    .map_err(|e| e.to_string())
}

fn validate_dates(from: &str, to: &str) -> Result<(), String> {
    let _ = NaiveDate::parse_from_str(from, "%Y-%m-%d").map_err(|_| "开始日期格式无效")?;
    let _ = NaiveDate::parse_from_str(to, "%Y-%m-%d").map_err(|_| "结束日期格式无效")?;
    if from > to {
        return Err("开始日期不能晚于结束日期".into());
    }
    Ok(())
}

fn format_period_lines(items: &[periods::PeriodSummaryPublic]) -> String {
    if items.is_empty() {
        return "（暂无时段小结）".into();
    }
    items
        .iter()
        .map(|p| {
            format!(
                "- {}–{} 【{}】 {}",
                fmt_clock(&p.started_at),
                fmt_clock(&p.ended_at),
                p.work_type,
                p.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_period_local(range_label: &str, items: &[periods::PeriodSummaryPublic]) -> String {
    if items.is_empty() {
        return format!(
            "# ✨ 时段小记 · {range_label}\n\n\
             这个时间段还没有攒够 AI 小结呢～\n\n\
             多使用一会儿电脑，小寒会在后台悄悄记下时段，过一会儿再来翻翻吧 (´▽`)"
        );
    }
    let mut out = format!("# ✨ 时段小记 · {range_label}\n\n");
    for p in items {
        out.push_str(&format!(
            "### 🕐 {} – {} · {}\n\n{}\n\n",
            fmt_clock(&p.started_at),
            fmt_clock(&p.ended_at),
            p.work_type,
            p.summary
        ));
    }
    out.trim_end().to_string()
}

fn collect_timeline_lines(db: &rusqlite::Connection, from: &str, to: &str) -> Result<String, String> {
    let cached = timeline_cache::list_by_date_range(db, from, to, 200).unwrap_or_default();
    let cache_by_started: std::collections::HashMap<String, timeline_cache::TimelineAiEntry> =
        cached
            .into_iter()
            .map(|e| (e.started_at.clone(), e))
            .collect();

    let mut lines = Vec::new();
    for date in date_iter(from, to)? {
        let merged = stats::query_timeline_merged_asc(db, date).map_err(|e| e.to_string())?;
        for seg in merged {
            let app = display_name::friendly_name(&seg.exe_path, &seg.app_name, &seg.window_title);
            let label = activity_key::activity_label_for_segment(&seg);
            let time = format_time_range(&seg);
            if let Some(entry) = cache_by_started.get(&seg.started_at) {
                lines.push(format!(
                    "- {} 【{}】{}",
                    time, entry.work_type, entry.summary
                ));
            } else {
                let detail = if label.is_empty() || label == app {
                    app.clone()
                } else {
                    format!("{app} · {label}")
                };
                lines.push(format!(
                    "- {} {}（{}）",
                    fmt_clock(&seg.started_at),
                    detail,
                    fmt_duration(seg.duration_ms)
                ));
            }
        }
    }
    if lines.is_empty() {
        Ok("（暂无超过 1 分钟的活动记录）".into())
    } else {
        Ok(lines.join("\n"))
    }
}

fn format_time_range(seg: &crate::tracker::Segment) -> String {
    let start = fmt_clock(&seg.started_at);
    let end = seg.ended_at.as_deref().map(fmt_clock);
    match end {
        Some(e) if e != start => format!("{start}–{e}"),
        _ => start,
    }
}

/// 清理 AI 返回：去代码块包裹、首尾空白
pub fn sanitize_report_markdown(raw: &str) -> String {
    let mut s = json_util::strip_md_fence(raw);
    while s.starts_with("# ") && s.matches('\n').count() < 2 {
        if let Some(idx) = s.find('\n') {
            s = s[idx + 1..].trim_start().to_string();
        } else {
            break;
        }
    }
    s.trim().to_string()
}

fn collect_app_lines(db: &rusqlite::Connection, from: &str, to: &str) -> Result<String, String> {
    let mut stmt = db
        .prepare(
            "SELECT aggregation_key, SUM(duration_ms) AS total \
             FROM activity_segments \
             WHERE is_idle = 0 AND substr(started_at, 1, 10) >= ?1 AND substr(started_at, 1, 10) <= ?2 \
             GROUP BY aggregation_key ORDER BY total DESC LIMIT 12",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![from, to], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
        })
        .map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    for row in rows.flatten() {
        if crate::tracker::title_parse::is_ignored_agg_key(&row.0) {
            continue;
        }
        lines.push(format!(
            "- {} · {}",
            display_name::friendly_from_key(&row.0),
            fmt_duration(row.1)
        ));
    }
    if lines.is_empty() {
        Ok("（暂无应用时长数据）".into())
    } else {
        Ok(lines.join("\n"))
    }
}

type MetricsTuple = (String, String, String, String);

fn collect_metrics_line(db: &rusqlite::Connection, from: &str, to: &str) -> Result<MetricsTuple, String> {
    let mut mouse = 0u64;
    let mut keys = 0u64;
    let mut created = 0u64;
    let mut modified = 0u64;
    for date in date_iter(from, to)? {
        let date_str = date.format("%Y-%m-%d").to_string();
        if let Ok(row) = db.query_row(
            "SELECT mouse_clicks, key_strokes, files_created, files_modified \
             FROM daily_metrics WHERE date = ?1",
            [&date_str],
            |r| {
                Ok((
                    r.get::<_, i64>(0)? as u64,
                    r.get::<_, i64>(1)? as u64,
                    r.get::<_, i64>(2)? as u64,
                    r.get::<_, i64>(3)? as u64,
                ))
            },
        ) {
            mouse += row.0;
            keys += row.1;
            created += row.2;
            modified += row.3;
        }
    }
    Ok((
        mouse.to_string(),
        keys.to_string(),
        created.to_string(),
        modified.to_string(),
    ))
}

fn format_activity_local(
    range_label: &str,
    timeline: &str,
    apps: &str,
    metrics: &MetricsTuple,
) -> String {
    let empty_timeline = timeline.starts_with("（暂无");
    let mut out = format!("# 📝 完成记录 · {range_label}\n\n");
    if empty_timeline {
        out.push_str("今天屏幕前的记录还不多，可能是出门玩去了？下次多开会儿电脑就能记下来啦～\n\n");
    } else {
        out.push_str("## 今天大概干了这些\n\n");
        out.push_str(timeline);
        out.push_str("\n\n");
    }
    out.push_str("## 应用时长 TOP\n\n");
    out.push_str(apps);
    out.push_str("\n\n");
    out.push_str(&format!(
        "---\n\n\
         🖱️ 点了 **{}** 次鼠标 · ⌨️ **{}** 次按键 · \
         📄 新建 **{}** 个文件 · ✏️ 改动 **{}** 个文件\n\n\
         *（未配置文本模型时由本地数据拼成，配置后会更像手账～）*",
        metrics.0, metrics.1, metrics.2, metrics.3
    ));
    out
}

fn fmt_clock(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|_| iso.chars().take(16).collect())
}

fn fmt_duration(ms: u64) -> String {
    let mins = ms / 60_000;
    if mins >= 60 {
        format!("{}小时{}分", mins / 60, mins % 60)
    } else if mins > 0 {
        format!("{mins}分钟")
    } else {
        "一会儿".into()
    }
}

fn format_date_range_label(from: &str, to: &str) -> String {
    if from == to {
        return format_short_date(from);
    }
    format!("{}～{}", format_short_date(from), format_short_date(to))
}

fn format_short_date(iso_date: &str) -> String {
    NaiveDate::parse_from_str(iso_date, "%Y-%m-%d")
        .map(|d| format!("{}月{}日", d.month(), d.day()))
        .unwrap_or_else(|_| iso_date.to_string())
}

fn date_iter(from: &str, to: &str) -> Result<Vec<NaiveDate>, String> {
    let start = NaiveDate::parse_from_str(from, "%Y-%m-%d").map_err(|_| "日期无效")?;
    let end = NaiveDate::parse_from_str(to, "%Y-%m-%d").map_err(|_| "日期无效")?;
    let mut out = Vec::new();
    let mut cur = start;
    while cur <= end {
        out.push(cur);
        cur = cur.succ_opt().ok_or("日期范围过大")?;
        if out.len() > 366 {
            return Err("日期范围不能超过一年".into());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_range_label() {
        assert_eq!(format_date_range_label("2026-07-02", "2026-07-02"), "7月2日");
        assert_eq!(
            format_date_range_label("2026-07-01", "2026-07-03"),
            "7月1日～7月3日"
        );
    }

    #[test]
    fn sanitize_strips_fence() {
        let raw = "```markdown\n# 标题\n\n正文\n```";
        assert_eq!(sanitize_report_markdown(raw), "# 标题\n\n正文");
    }
}
