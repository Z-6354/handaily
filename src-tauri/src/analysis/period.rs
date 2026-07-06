//! 时段 AI 分析：根据活动记录判断工作类型与总结

use std::path::Path;

use crate::ai::{self, AiConfig, PreparedTextChat};
use crate::prompts;
use crate::tracker::Segment;
use crate::vault::VaultState;
use crate::work_type::WorkTypeConfig;

#[derive(Debug, Clone)]
pub struct PeriodAnalysisResult {
    pub work_type: String,
    pub summary: String,
}

pub fn prepare_period_chat(
    segments: &[Segment],
    work_types: &WorkTypeConfig,
    config: &AiConfig,
    vault: &VaultState,
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<Option<PreparedTextChat>, String> {
    if segments.is_empty() || config.text_model.trim().is_empty() {
        return Ok(None);
    }

    let type_list = work_types.type_names().join("、");
    let mut lines = String::new();
    for seg in segments.iter().take(40) {
        let end = seg.ended_at.as_deref().unwrap_or(&seg.started_at);
        let dur_min = seg.duration_ms / 60_000;
        lines.push_str(&format!(
            "- {} ~ {} · {} · {} · 「{}」\n",
            &seg.started_at[..19.min(seg.started_at.len())],
            &end[..19.min(end.len())],
            seg.app_name,
            format!("{dur_min}分钟"),
            seg.window_title.chars().take(60).collect::<String>(),
        ));
    }

    let prompt = prompts::render(
        data_dir,
        "period-analysis",
        &[
            ("type_list", &type_list),
            ("activity_lines", &lines),
        ],
    );

    match PreparedTextChat::prepare(
        config,
        &ai::load_catalog(data_dir),
        vault,
        db,
        data_dir,
        prompt,
    ) {
        Ok(opt) => Ok(opt),
        Err(e) => {
            eprintln!("xiaohan-daily: period AI prep skipped: {e}");
            Ok(None)
        }
    }
}

pub fn finalize_period_analysis(
    segments: &[Segment],
    work_types: &WorkTypeConfig,
    ai_raw: Option<String>,
) -> PeriodAnalysisResult {
    if let Some(raw) = ai_raw {
        if let Ok(parsed) = parse_period_json(&raw, work_types) {
            return parsed;
        }
    }
    rule_based_period(segments, work_types)
}

pub fn analyze_period(
    segments: &[Segment],
    work_types: &WorkTypeConfig,
    config: &AiConfig,
    vault: &VaultState,
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<PeriodAnalysisResult, String> {
    if segments.is_empty() {
        return Ok(PeriodAnalysisResult {
            work_type: work_types.normalize_type_name("其他"),
            summary: "该时段无有效前台活动".into(),
        });
    }

    let prep = prepare_period_chat(segments, work_types, config, vault, db, data_dir)?;
    let ai_raw = match prep {
        Some(p) => match p.run_sync() {
            Ok(raw) => Some(raw),
            Err(e) => {
                eprintln!("xiaohan-daily: period AI failed: {e}");
                None
            }
        },
        None => None,
    };
    Ok(finalize_period_analysis(segments, work_types, ai_raw))
}

fn parse_period_json(raw: &str, work_types: &WorkTypeConfig) -> Result<PeriodAnalysisResult, String> {
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with('{') {
        trimmed.to_string()
    } else if let Some(start) = trimmed.find('{') {
        trimmed[start..]
            .split('}')
            .next()
            .map(|s| format!("{s}}}"))
            .unwrap_or_else(|| trimmed.to_string())
    } else {
        return Ok(rule_based_period(&[], work_types));
    };
    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap_or(serde_json::json!({}));
    let wt = work_types.normalize_type_name(v["work_type"].as_str().unwrap_or("其他"));
    let summary = v["summary"].as_str().unwrap_or("时段活动").to_string();
    Ok(PeriodAnalysisResult {
        work_type: wt,
        summary,
    })
}

fn rule_based_period(segments: &[Segment], work_types: &WorkTypeConfig) -> PeriodAnalysisResult {
    if segments.is_empty() {
        return PeriodAnalysisResult {
            work_type: work_types.normalize_type_name("其他"),
            summary: "无活动".into(),
        };
    }
    let top = segments.iter().max_by_key(|s| s.duration_ms).unwrap();
    let wt = if top.app_name.to_lowercase().contains("code")
        || top.window_title.contains("rust")
        || top.window_title.contains(".ts")
    {
        "开发"
    } else if top.app_name.contains("微信") || top.app_name.contains("Teams") {
        "沟通"
    } else {
        "其他"
    };
    PeriodAnalysisResult {
        work_type: work_types.normalize_type_name(wt),
        summary: format!("主要使用 {}", top.app_name),
    }
}
