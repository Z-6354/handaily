//! 基于窗口标题与应用名的文本语义分析

use crate::analysis::TextInsight;
use crate::tracker::{activity_key, Segment};

const GENERIC_TITLES: &[&str] = &[
    "",
    "microsoft edge",
    "google chrome",
    "mozilla firefox",
    "visual studio code",
    "code",
    "settings",
    "设置",
    "program manager",
    "desktop",
    "explorer",
    "文件资源管理器",
    "windows 输入体验",
];

pub fn analyze(segment: &Segment) -> TextInsight {
    if segment.is_idle {
        return TextInsight {
            category: "idle".into(),
            summary: "空闲".into(),
            confidence: 1.0,
            needs_screenshot: false,
        };
    }

    if segment.source_type == "audio" {
        return analyze_audio(segment);
    }

    let title = segment.window_title.trim();
    let title_lower = title.to_lowercase();
    let app_lower = segment.app_name.to_lowercase();
    let key_lower = segment.aggregation_key.to_lowercase();

    let (category, base_confidence) = classify(&title_lower, &app_lower, &key_lower);

    let mut confidence = base_confidence;
    let mut summary = activity_key::human_activity_summary(segment);

    if title.is_empty() {
        confidence = 0.15;
        summary = format!("使用 {}（窗口标题不可见）", segment.app_name);
    } else if is_generic_title(&title_lower) {
        confidence = confidence.min(0.25);
        summary = format!("使用 {}（标题信息不足）", segment.app_name);
    } else if has_rich_title(title) {
        confidence = confidence.max(0.72);
    }

    let needs_screenshot = confidence < 0.45;

    TextInsight {
        category,
        summary,
        confidence,
        needs_screenshot,
    }
}

fn analyze_audio(segment: &Segment) -> TextInsight {
    let (category, activity) = match segment.audio_activity.as_str() {
        "music" => ("entertainment", "听歌"),
        "video" => ("entertainment", "看视频"),
        "chat" => ("communication", "聊天通话"),
        _ => ("general", "音频"),
    };
    TextInsight {
        category: category.into(),
        summary: activity_key::human_activity_summary(segment)
            .replace("播放音频", activity),
        confidence: 0.78,
        needs_screenshot: false,
    }
}

fn classify(title: &str, app: &str, key: &str) -> (String, f32) {
    if matches_dev(title, app, key) {
        return ("development".into(), 0.78);
    }
    if matches_doc(title, app) {
        return ("document".into(), 0.7);
    }
    if matches_meeting(title, app) {
        return ("meeting".into(), 0.75);
    }
    if matches_comm(title, app) {
        return ("communication".into(), 0.68);
    }
    if matches_design(title, app, key) {
        return ("design".into(), 0.72);
    }
    if matches_browser(app, key) {
        return ("browsing".into(), 0.35);
    }
    if matches_entertainment(title, app) {
        return ("entertainment".into(), 0.55);
    }
    ("general".into(), 0.4)
}

fn is_generic_title(title: &str) -> bool {
    GENERIC_TITLES.iter().any(|g| title == *g)
}

fn has_rich_title(title: &str) -> bool {
    title.len() >= 8
        && (title.contains(" - ")
            || title.contains(" | ")
            || title.contains('—')
            || title.contains('·')
            || title.contains('/')
            || title.contains('\\'))
}

fn matches_dev(title: &str, app: &str, key: &str) -> bool {
    let dev_apps = [
        "code", "cursor", "devenv", "idea", "pycharm", "goland", "webstorm",
        "terminal", "windowsterminal", "powershell", "cmd", "wt",
    ];
    dev_apps.iter().any(|a| key.contains(a) || app.contains(a))
        || title.ends_with(".rs")
        || title.ends_with(".ts")
        || title.ends_with(".tsx")
        || title.ends_with(".py")
        || title.contains("github")
        || title.contains("gitlab")
}

fn matches_doc(title: &str, app: &str) -> bool {
    app.contains("word")
        || app.contains("excel")
        || app.contains("powerpoint")
        || app.contains("wps")
        || app.contains("notion")
        || title.contains("docs.google")
        || title.contains("文档")
        || title.contains("docx")
}

fn matches_meeting(title: &str, app: &str) -> bool {
    app.contains("teams")
        || app.contains("zoom")
        || app.contains("feishu")
        || app.contains("lark")
        || app.contains("腾讯会议")
        || app.contains("钉钉")
        || title.contains("meeting")
        || title.contains("会议")
}

fn matches_comm(title: &str, app: &str) -> bool {
    app.contains("slack")
        || app.contains("discord")
        || app.contains("wechat")
        || app.contains("微信")
        || app.contains("qq")
        || app.contains("telegram")
        || title.contains("chat")
}

fn matches_design(title: &str, app: &str, key: &str) -> bool {
    key.contains("figma")
        || app.contains("figma")
        || app.contains("photoshop")
        || app.contains("illustrator")
        || app.contains("sketch")
        || title.contains("figma")
}

fn matches_browser(app: &str, key: &str) -> bool {
    let browsers = [
        "msedge", "chrome", "firefox", "brave", "opera", "safari",
    ];
    browsers.iter().any(|b| key.contains(b) || app.contains(b))
}

fn matches_entertainment(title: &str, app: &str) -> bool {
    title.contains("bilibili")
        || title.contains("youtube")
        || title.contains("netflix")
        || title.contains("抖音")
        || title.contains("游戏")
        || app.contains("steam")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::Segment;

    fn seg(title: &str, app: &str, key: &str) -> Segment {
        Segment {
            started_at: "2026-07-02T10:00:00+08:00".into(),
            ended_at: Some("2026-07-02T10:05:00+08:00".into()),
            duration_ms: 300_000,
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
    fn rich_title_high_confidence_no_screenshot() {
        let r = analyze(&seg(
            "main.rs - HANDAILY - Visual Studio Code",
            "Code",
            "code.exe",
        ));
        assert!(!r.needs_screenshot);
        assert!(r.confidence >= 0.45);
        assert!(!r.summary.starts_with("开发："));
    }

    #[test]
    fn generic_edge_needs_screenshot() {
        let r = analyze(&seg("Microsoft Edge", "Microsoft Edge", "msedge.exe"));
        assert!(r.needs_screenshot);
    }
}
