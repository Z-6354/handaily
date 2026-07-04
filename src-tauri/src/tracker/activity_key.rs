//! 活动内容标识：同一应用内按「在做什么」区分，而非仅看窗口标题字面量

use super::context_enrich::{enrich_segment, SegmentEnrichment};
use super::Segment;

/// 从 segment 提取活动内容键（用于切分、合并、时间线 AI 历史）
pub fn activity_key_for_segment(seg: &Segment) -> String {
    if seg.source_type == "audio" {
        let act = if seg.audio_activity.is_empty() {
            "other"
        } else {
            seg.audio_activity.as_str()
        };
        return format!("audio:{act}");
    }
    let enrichment = enrich_segment(seg);
    activity_key_from_enrichment(&enrichment, &seg.window_title)
}

/// 供 poller 在尚未构造 Segment 时使用
pub fn activity_key_for_fields(
    exe_path: &str,
    app_name: &str,
    window_title: &str,
    aggregation_key: &str,
) -> String {
    let seg = Segment {
        started_at: String::new(),
        ended_at: None,
        duration_ms: 0,
        app_name: app_name.to_string(),
        exe_path: exe_path.to_string(),
        window_title: window_title.to_string(),
        is_idle: false,
        aggregation_key: aggregation_key.to_string(),
        icon: None,
        source_type: "foreground".into(),
        audio_activity: String::new(),
        activity_label: None,
    };
    activity_key_for_segment(&seg)
}

/// 人类可读的一行活动描述（本地简介 / 混合分析，不用「开发：」前缀）
pub fn human_activity_summary(seg: &Segment) -> String {
    if seg.source_type == "audio" {
        let activity = match seg.audio_activity.as_str() {
            "music" => "听歌",
            "video" => "看视频",
            "chat" => "聊天通话",
            _ => "播放音频",
        };
        return format!("后台在 {} {}", seg.app_name, activity);
    }
    let e = enrich_segment(seg);
    human_summary_from_enrichment(&e, &seg.app_name)
}

/// 从 enrichment 生成自然语言摘要（保留原文大小写）
pub fn human_summary_from_enrichment(e: &SegmentEnrichment, app_name: &str) -> String {
    match e.app_kind.as_str() {
        "cursor" | "ide" => ide_human_summary(e, app_name),
        "browser" => field_raw(e, "page_title")
            .map(|page| format!("在浏览器看「{}」", truncate_display(&page, 36)))
            .unwrap_or_else(|| "在浏览器浏览".into()),
        "chat" => field_raw(e, "chat_target")
            .map(|target| {
                format!(
                    "在 {} 和「{}」聊天",
                    app_name,
                    truncate_display(&target, 24)
                )
            })
            .unwrap_or_else(|| format!("在使用 {}", app_name)),
        "terminal" => field_raw(e, "session_title")
            .map(|title| format!("在终端里：{}", truncate_display(&title, 40)))
            .unwrap_or_else(|| "在使用终端".into()),
        _ => {
            if let Some(primary) = field_raw(e, "primary") {
                return format!("在 {} · {}", app_name, truncate_display(&primary, 36));
            }
            if app_name.is_empty() {
                "在使用电脑".into()
            } else {
                format!("在使用 {}", app_name)
            }
        }
    }
}

fn ide_human_summary(e: &SegmentEnrichment, app_name: &str) -> String {
    let doc = field_raw(e, "open_document");
    let proj = field_raw(e, "project_or_context").or_else(|| field_raw(e, "project_name"));
    let editor = if app_name.eq_ignore_ascii_case("cursor") {
        "Cursor"
    } else {
        app_name
    };
    match (doc.as_deref(), proj.as_deref()) {
        (Some(d), Some(p)) => format!(
            "在 {} 里改「{}」（{} 项目）",
            editor,
            friendly_doc_name(d),
            p
        ),
        (Some(d), None) => format!("在 {} 里编辑「{}」", editor, friendly_doc_name(d)),
        (None, Some(p)) => format!("在 {} 项目里写代码", p),
        _ => format!("在 {} 里写代码", editor),
    }
}

fn field_raw(e: &SegmentEnrichment, key: &str) -> Option<String> {
    e.fields.get(key).cloned().filter(|s| !s.trim().is_empty())
}

fn friendly_doc_name(name: &str) -> String {
    let base = name.rsplit(['\\', '/']).next().unwrap_or(name).trim();
    let stripped = base
        .trim_end_matches(".plan.md")
        .trim_end_matches(".md")
        .trim_end_matches(".rs")
        .trim_end_matches(".ts")
        .trim_end_matches(".tsx");
    truncate_display(stripped, 32)
}

fn truncate_display(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(max).collect::<String>())
    }
}

/// 是否为旧版机器式简介（应重新生成）
pub fn is_machine_summary(summary: &str) -> bool {
    let s = summary.trim();
    s.starts_with("开发：")
        || s.starts_with("文档：")
        || s.starts_with("浏览：")
        || s.starts_with("[text·")
        || s.starts_with("[screenshot·")
        || (s.contains('·') && s.contains("窗口「"))
}

/// 人类可读的活动标签（时间线展示 / AI 历史）
pub fn activity_label_for_segment(seg: &Segment) -> String {
    if seg.source_type == "audio" {
        return match seg.audio_activity.as_str() {
            "music" => "听歌".into(),
            "video" => "看视频".into(),
            "chat" => "聊天通话".into(),
            _ => "后台音频".into(),
        };
    }
    let e = enrich_segment(seg);
    activity_label_from_enrichment(&e, seg)
}

fn activity_key_from_enrichment(e: &SegmentEnrichment, window_title: &str) -> String {
    let key = match e.app_kind.as_str() {
        "cursor" | "ide" => pick_project_activity(e),
        "browser" => pick_field(e, &["page_title", "site_hint"]),
        "chat" => pick_field(e, &["chat_target"]),
        "terminal" => pick_field(e, &["session_title", "cwd_hint"]),
        _ => pick_field(e, &["primary", "context"]),
    };

    key.or_else(|| pick_field(e, &["open_document"]))
        .or_else(|| pick_field(e, &["window_title_raw"]))
        .unwrap_or_else(|| normalize(window_title))
}

/// 优先用标题解析出的项目/上下文，忽略 exe 路径误推的 project_name
fn pick_project_activity(e: &SegmentEnrichment) -> Option<String> {
    if let Some(v) = pick_field(e, &["project_or_context"]) {
        return Some(v);
    }
    if let Some(v) = pick_field(e, &["project_name"]) {
        if !looks_like_exe_name(&v) {
            return Some(v);
        }
    }
    pick_field(e, &["workspace", "session_title"])
}

fn looks_like_exe_name(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.ends_with(".exe") || lower.ends_with(".app") || lower == "cursor" || lower == "code"
}

fn activity_label_from_enrichment(e: &SegmentEnrichment, seg: &Segment) -> String {
    if let Some(v) = pick_field(e, &["project_name", "project_or_context", "workspace"]) {
        return v;
    }
    if let Some(v) = pick_field(e, &["page_title"]) {
        return v;
    }
    if let Some(v) = pick_field(e, &["chat_target", "session_title"]) {
        return v;
    }
    if let Some(v) = pick_field(e, &["open_document"]) {
        return format!("{} · {}", seg.app_name, v);
    }
    let title = seg.window_title.trim();
    if title.is_empty() {
        seg.app_name.clone()
    } else {
        title.chars().take(48).collect()
    }
}

fn pick_field(e: &SegmentEnrichment, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| e.fields.get(*k).map(|v| normalize(v)))
        .filter(|s| !s.is_empty())
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(title: &str, app: &str, key: &str) -> Segment {
        Segment {
            started_at: String::new(),
            ended_at: None,
            duration_ms: 0,
            app_name: app.into(),
            exe_path: key.into(),
            window_title: title.into(),
            is_idle: false,
            aggregation_key: key.into(),
            icon: None,
            source_type: "foreground".into(),
            audio_activity: String::new(),
            activity_label: None,
        }
    }

    #[test]
    fn same_project_different_files_share_key() {
        let a = seg("main.rs - HANDAILY - Cursor", "Cursor", "cursor.exe");
        let b = seg("utils.rs - HANDAILY - Cursor", "Cursor", "cursor.exe");
        assert_eq!(
            activity_key_for_segment(&a),
            activity_key_for_segment(&b)
        );
    }

    #[test]
    fn different_projects_differ() {
        let a = seg("main.rs - HANDAILY - Cursor", "Cursor", "cursor.exe");
        let b = seg("index.ts - OTHER - Cursor", "Cursor", "cursor.exe");
        assert_ne!(
            activity_key_for_segment(&a),
            activity_key_for_segment(&b)
        );
    }

    #[test]
    fn browser_uses_page_title() {
        let a = seg("GitHub - Microsoft Edge", "Edge", "msedge.exe");
        let b = seg("Docs - Microsoft Edge", "Edge", "msedge.exe");
        assert_ne!(
            activity_key_for_segment(&a),
            activity_key_for_segment(&b)
        );
    }

    #[test]
    fn human_summary_cursor_plan() {
        let s = seg(
            "handaily_桌宠集成_4be8ad40.plan.md - live2d - Cursor",
            "Cursor",
            "cursor.exe",
        );
        let summary = human_activity_summary(&s);
        assert!(!summary.starts_with("开发："));
        assert!(!summary.contains("窗口「"));
        assert!(summary.contains("Cursor"));
    }

    #[test]
    fn machine_summary_detect() {
        assert!(is_machine_summary(
            "开发：handaily_桌宠集成_4be8ad40.plan.md - live2d - Cursor · 窗口「…」"
        ));
    }
}
