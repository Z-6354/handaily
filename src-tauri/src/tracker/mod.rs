//! 前台活动采集模块
//!
//! - `win32` — 前台窗口快照（GetForegroundWindow + exe_path）
//! - `idle` — 空闲检测（GetLastInputInfo）
//! - `poller` — 采样循环 + segment 合并/切分
//! - `writer` — 延迟 flush + 短片段合并 + 60s checkpoint + 退出 flush

pub mod activity_key;
pub mod audio_classify;
pub mod audio_monitor;
pub mod context_enrich;
pub mod display_name;
pub mod file_watcher;
pub mod icon;
pub mod idle;
pub mod input_monitor;
pub mod title_parse;
pub mod poller;
pub mod win32;
pub mod writer;

use serde::{Deserialize, Serialize};

/// IPC 可序列化的前台快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForegroundPayload {
    pub app_name: String,
    pub exe_path: String,
    pub window_title: String,
    pub is_idle: bool,
    pub captured_at: String,
}

/// 一帧前台快照（采集原始数据）
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub pid: u32,
    pub exe_path: String,
    pub app_name: String,
    pub window_title: String,
    pub captured_at: chrono::DateTime<chrono::Local>,
    pub is_idle: bool,
}

impl Snapshot {
    pub fn to_payload(&self) -> ForegroundPayload {
        ForegroundPayload {
            app_name: display_name::friendly_name(
                &self.exe_path,
                &self.app_name,
                &self.window_title,
            ),
            exe_path: self.exe_path.clone(),
            window_title: self.window_title.clone(),
            is_idle: self.is_idle,
            captured_at: self.captured_at.to_rfc3339(),
        }
    }
}

/// 一个活动时间片（DB 行 + 内存表示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub started_at: String,       // ISO8601 本地
    pub ended_at: Option<String>, // None = 未闭合（内存态）；DB 里 NULL = 孤儿段
    pub duration_ms: u64,
    pub app_name: String,
    pub exe_path: String,
    pub window_title: String,
    pub is_idle: bool,
    pub aggregation_key: String,
    /// 应用图标（仅 IPC 返回时填充，不落库）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// foreground | audio（后台音频检测）
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// music | video | chat | other | ""
    #[serde(default)]
    pub audio_activity: String,
    /// 活动内容摘要（仅时间线 IPC 填充，不落库）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_label: Option<String>,
}

fn default_source_type() -> String {
    "foreground".into()
}

impl Segment {
    /// 从快照开启新段
    pub fn from_snapshot(snap: &Snapshot) -> Self {
        let started = snap.captured_at.to_rfc3339();
        let agg_key = derive_aggregation_key(&snap.exe_path, &snap.app_name, &snap.window_title);
        let agg_key = if agg_key.is_empty() {
            IDLE_AGG_KEY.to_string()
        } else {
            agg_key
        };
        let friendly = display_name::friendly_name(&snap.exe_path, &snap.app_name, &snap.window_title);
        Self {
            started_at: started,
            ended_at: None,
            duration_ms: 0,
            app_name: friendly,
            exe_path: snap.exe_path.clone(),
            window_title: snap.window_title.clone(),
            is_idle: snap.is_idle,
            aggregation_key: agg_key,
            icon: None,
            source_type: default_source_type(),
            audio_activity: String::new(),
            activity_label: None,
        }
    }
}

/// 聚合键：UWP 用 package family → exe_path → 标题解析名 → app_name(stem)
pub fn derive_aggregation_key(exe_path: &str, app_name: &str, window_title: &str) -> String {
    if app_name.eq_ignore_ascii_case("desktop") || window_title == "桌面" {
        return "desktop".to_string();
    }
    if let Some(pfn) = uwp_package_family(exe_path) {
        return pfn.to_lowercase();
    }
    if !exe_path.is_empty() {
        return exe_path.to_lowercase();
    }
    if !app_name.is_empty() {
        return app_name.to_lowercase();
    }
    if let Some(from_title) = title_parse::app_name_from_title(window_title) {
        return from_title.to_lowercase();
    }
    String::new()
}

/// 无前台窗口时的占位键（仅 idle 段使用，不计入排行）
pub const IDLE_AGG_KEY: &str = "__idle__";

/// UWP：SystemApps 与 WindowsApps 目录
fn uwp_package_family(exe_path: &str) -> Option<String> {
    let normalized = exe_path.replace('/', "\\");
    let lower = normalized.to_lowercase();

    if let Some(idx) = lower.find("\\windows\\systemapps\\") {
        let rest = &normalized[idx + "\\windows\\systemapps\\".len()..];
        let family = rest.split('\\').next()?.trim();
        if !family.is_empty() {
            return Some(family.to_string());
        }
    }

    if let Some(idx) = lower.find("\\windowsapps\\") {
        let rest = &normalized[idx + "\\windowsapps\\".len()..];
        let folder = rest.split('\\').next()?.trim();
        // Microsoft.ScreenSketch_1.2.3.0_x64__8wekyb3d8bbwe → Microsoft.ScreenSketch_8wekyb3d8bbwe
        if let Some((name, tail)) = folder.split_once("__") {
            if let Some((pkg, _arch)) = name.rsplit_once('_') {
                return Some(format!("{pkg}_{tail}"));
            }
            return Some(format!("{name}_{tail}"));
        }
        if !folder.is_empty() {
            return Some(folder.to_string());
        }
    }

    None
}

/// 从快照取聚合键（poller.rs 调用）
pub fn derive_agg_key_for_snap(snap: &Snapshot) -> String {
    let key = derive_aggregation_key(&snap.exe_path, &snap.app_name, &snap.window_title);
    if key.is_empty() {
        IDLE_AGG_KEY.to_string()
    } else {
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregation_key_prefers_exe_path() {
        let key = derive_aggregation_key(r"C:\Windows\explorer.exe", "explorer", "");
        assert_eq!(key, r"c:\windows\explorer.exe");
    }

    #[test]
    fn aggregation_key_uwp_package_family() {
        let key = derive_aggregation_key(
            r"C:\Windows\SystemApps\Microsoft.WindowsCalculator_8wekyb3d8bbwe\Calculator.exe",
            "Calculator",
            "",
        );
        assert_eq!(key, "microsoft.windowscalculator_8wekyb3d8bbwe");
    }

    #[test]
    fn aggregation_key_falls_back_to_app_name() {
        let key = derive_aggregation_key("", "Notepad", "");
        assert_eq!(key, "notepad");
    }

    #[test]
    fn aggregation_key_falls_back_to_title() {
        let key = derive_aggregation_key("", "", "main.rs - Visual Studio Code");
        assert_eq!(key, "visual studio code");
    }

    #[test]
    fn segment_from_snapshot_idle_flag() {
        let snap = Snapshot {
            pid: 1,
            exe_path: r"C:\Windows\System32\notepad.exe".into(),
            app_name: "notepad".into(),
            window_title: "Untitled".into(),
            captured_at: chrono::Local::now(),
            is_idle: true,
        };
        let seg = Segment::from_snapshot(&snap);
        assert!(seg.is_idle);
        assert_eq!(seg.aggregation_key, r"c:\windows\system32\notepad.exe");
    }
}
