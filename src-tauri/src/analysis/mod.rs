//! 活动语义分析：混合检索（文本优先，低置信度时截图）

pub mod coordinator;
pub mod guard;
pub mod period;
pub mod period_scheduler;
pub mod text;
pub mod vision;

pub use coordinator::AnalysisCoordinator;
pub use period_scheduler::PeriodScheduler;

use crate::tracker::Segment;

#[derive(Debug, Clone)]
pub struct AnalysisSettings {
    pub hybrid_enabled: bool,
    pub screenshot_enabled: bool,
    pub confidence_threshold: f32,
    pub cpu_threshold_percent: f32,
    pub screenshot_min_interval_secs: u64,
    pub min_segment_ms: u64,
    pub vision_enabled: bool,
    pub vision_vault_entry_id: Option<i64>,
    pub excluded_exes: Vec<String>,
}

impl Default for AnalysisSettings {
    fn default() -> Self {
        Self {
            hybrid_enabled: true,
            screenshot_enabled: true,
            confidence_threshold: 0.45,
            cpu_threshold_percent: 75.0,
            screenshot_min_interval_secs: 120,
            min_segment_ms: 5000,
            vision_enabled: false,
            vision_vault_entry_id: None,
            excluded_exes: vec![
                "keepass".into(),
                "1password".into(),
                "bitwarden".into(),
            ],
        }
    }
}

impl AnalysisSettings {
    pub fn load(db: &rusqlite::Connection) -> Self {
        let mut s = Self::default();
        s.hybrid_enabled = get_bool(db, "analysis_hybrid_enabled", true);
        s.screenshot_enabled = get_bool(db, "analysis_screenshot_enabled", true);
        s.confidence_threshold = get_f32(db, "analysis_text_confidence_threshold", 0.45);
        s.cpu_threshold_percent = get_f32(db, "analysis_cpu_threshold_percent", 75.0);
        s.screenshot_min_interval_secs =
            get_u64(db, "analysis_screenshot_min_interval_secs", 120);
        s.min_segment_ms = get_u64(db, "analysis_min_segment_ms", 5000);
        s.vision_enabled = get_bool(db, "analysis_vision_enabled", false);
        s.vision_vault_entry_id = crate::db::get_setting(db, "analysis_vision_vault_entry_id")
            .and_then(|v| v.parse().ok())
            .filter(|&id| id > 0);
        if let Some(raw) = crate::db::get_setting(db, "analysis_excluded_exes") {
            s.excluded_exes = raw
                .split(',')
                .map(|x| x.trim().to_lowercase())
                .filter(|x| !x.is_empty())
                .collect();
        }
        s
    }
}

fn get_bool(db: &rusqlite::Connection, key: &str, default: bool) -> bool {
    crate::db::get_setting(db, key)
        .map(|v| v == "1")
        .unwrap_or(default)
}

fn get_f32(db: &rusqlite::Connection, key: &str, default: f32) -> f32 {
    crate::db::get_setting(db, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn get_u64(db: &rusqlite::Connection, key: &str, default: u64) -> u64 {
    crate::db::get_setting(db, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[derive(Debug, Clone)]
pub struct TextInsight {
    pub category: String,
    pub summary: String,
    pub confidence: f32,
    pub needs_screenshot: bool,
}

#[derive(Debug, Clone)]
pub struct AnalysisJob {
    pub segment: Segment,
}
