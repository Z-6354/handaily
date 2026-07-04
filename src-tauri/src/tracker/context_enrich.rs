//! 从窗口标题 / 路径解析结构化上下文，供时间线 AI 等使用

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use super::Segment;

#[derive(Debug, Clone, Serialize, Default)]
pub struct SegmentEnrichment {
    /// cursor / browser / ide / terminal / chat / office / other
    pub app_kind: String,
    pub fields: HashMap<String, String>,
    /// 给 AI 读的要点（已整理）
    pub hints: Vec<String>,
}

pub fn enrich_segment(seg: &Segment) -> SegmentEnrichment {
    let key = seg.aggregation_key.to_lowercase();
    let app = seg.app_name.to_lowercase();
    let title = seg.window_title.trim();

    if title.is_empty() && seg.exe_path.is_empty() {
        return SegmentEnrichment {
            app_kind: "other".into(),
            hints: vec![format!("应用：{}", seg.app_name)],
            ..Default::default()
        };
    }

    if is_cursor(&key, &app) {
        return enrich_cursor(seg, title);
    }
    if is_browser(&key, &app) {
        return enrich_browser(seg, title);
    }
    if is_vscode(&key, &app) {
        return enrich_ide(seg, title, "vscode");
    }
    if is_terminal(&key, &app) {
        return enrich_terminal(seg, title);
    }
    if is_wechat(&key, &app) {
        return enrich_chat(seg, title, "微信");
    }
    if is_qq(&key, &app) {
        return enrich_chat(seg, title, "QQ");
    }
    enrich_generic(seg, title)
}

fn is_cursor(key: &str, app: &str) -> bool {
    key.contains("cursor") || app.contains("cursor")
}

fn is_vscode(key: &str, app: &str) -> bool {
    key.contains("code.exe") || app.contains("visual studio code") || app == "code"
}

fn is_browser(key: &str, app: &str) -> bool {
    ["msedge", "chrome", "firefox", "brave", "opera", "safari"]
        .iter()
        .any(|b| key.contains(b) || app.contains(b))
}

fn is_terminal(key: &str, app: &str) -> bool {
    ["windowsterminal", "powershell", "cmd.exe", "wt.exe", "terminal"]
        .iter()
        .any(|t| key.contains(t) || app.contains(t))
}

fn is_wechat(key: &str, app: &str) -> bool {
    key.contains("wechat") || app.contains("微信")
}

fn is_qq(key: &str, app: &str) -> bool {
    key.contains("qq.exe") || app.contains("qq")
}

fn enrich_cursor(seg: &Segment, title: &str) -> SegmentEnrichment {
    let mut fields = HashMap::new();
    let mut hints = vec!["应用：Cursor（AI 代码编辑器）".to_string()];

    if let Some(ws) = workspace_from_exe(&seg.exe_path) {
        fields.insert("workspace_path".into(), ws.clone());
        if let Some(name) = workspace_folder_name(&ws) {
            fields.insert("project_name".into(), name.clone());
            hints.push(format!("项目目录：{name}"));
        }
    }

    let mut parts = split_title_parts(title);
    strip_suffix_app(&mut parts, &["cursor"]);

    match parts.len() {
        0 => {}
        1 => {
            let p = &parts[0];
            if looks_like_filename(p) {
                fields.insert("open_document".into(), p.clone());
                hints.push(format!("打开文件：{p}"));
            } else if p != "Cursor" {
                fields.insert("session_title".into(), p.clone());
                hints.push(format!("当前会话/标题：{p}"));
            }
        }
        2 => {
            let a = &parts[0];
            let b = &parts[1];
            if looks_like_filename(a) {
                fields.insert("open_document".into(), a.clone());
                hints.push(format!("打开文件：{a}"));
                fields.insert("project_or_context".into(), b.clone());
                hints.push(format!("项目/上下文：{b}"));
            } else {
                fields.insert("session_title".into(), a.clone());
                hints.push(format!("会话或提问主题：{a}"));
                fields.insert("project_name".into(), b.clone());
                hints.push(format!("项目：{b}"));
            }
        }
        _ => {
            if let Some(doc) = parts.first() {
                if looks_like_filename(doc) {
                    fields.insert("open_document".into(), doc.clone());
                    hints.push(format!("打开文件：{doc}"));
                }
            }
            if parts.len() >= 2 {
                let project = &parts[parts.len() - 2];
                if !project.eq_ignore_ascii_case("cursor") {
                    fields.insert("project_name".into(), project.clone());
                    hints.push(format!("项目：{project}"));
                }
            }
            let session = parts[..parts.len().saturating_sub(2)]
                .join(" - ");
            if !session.is_empty() && !looks_like_filename(&session) {
                fields.insert("session_title".into(), session.clone());
                hints.push(format!("会话/聊天标题：{session}"));
            }
        }
    }

    if title.len() > 3 {
        fields.insert("window_title_raw".into(), title.to_string());
    }

    SegmentEnrichment {
        app_kind: "cursor".into(),
        fields,
        hints,
    }
}

fn enrich_browser(seg: &Segment, title: &str) -> SegmentEnrichment {
    let mut fields = HashMap::new();
    let mut hints = vec![format!("应用：{}", friendly_browser_name(seg))];

    let mut parts = split_title_parts(title);
    strip_suffix_app(
        &mut parts,
        &[
            "microsoft edge",
            "google chrome",
            "mozilla firefox",
            "brave",
            "opera",
        ],
    );
    strip_profile_suffix(&mut parts);

    let page = if parts.is_empty() {
        title.to_string()
    } else if parts.len() == 1 {
        parts[0].clone()
    } else {
        // Edge: "页签标题 - 和另外 1 个页面 - 用户配置 1"
        let joined = parts.join(" - ");
        if joined.contains("和另外") || joined.contains("more page") {
            parts.first().cloned().unwrap_or(joined)
        } else {
            joined
        }
    };

    if !page.is_empty() {
        fields.insert("page_title".into(), page.clone());
        hints.push(format!("网页标题：{page}"));
    }

    if let Some(site) = infer_site_from_title(title) {
        fields.insert("site_hint".into(), site.clone());
        hints.push(format!("站点/域名线索：{site}"));
    }

    if title.contains("http://") || title.contains("https://") {
        if let Some(url) = extract_url_fragment(title) {
            fields.insert("url_in_title".into(), url);
            hints.push(format!("标题中含链接：{}", fields["url_in_title"]));
        }
    }

    fields.insert("window_title_raw".into(), title.to_string());

    SegmentEnrichment {
        app_kind: "browser".into(),
        fields,
        hints,
    }
}

fn enrich_ide(seg: &Segment, title: &str, kind: &str) -> SegmentEnrichment {
    let mut fields = HashMap::new();
    let label = if kind == "vscode" {
        "Visual Studio Code"
    } else {
        "IDE"
    };
    let mut hints = vec![format!("应用：{label}")];

    let mut parts = split_title_parts(title);
    strip_suffix_app(&mut parts, &["visual studio code", "code"]);

    match parts.len() {
        0 => {}
        1 => {
            if looks_like_filename(&parts[0]) {
                fields.insert("open_document".into(), parts[0].clone());
                hints.push(format!("打开文件：{}", parts[0]));
            } else {
                fields.insert("workspace".into(), parts[0].clone());
                hints.push(format!("工作区：{}", parts[0]));
            }
        }
        _ => {
            fields.insert("open_document".into(), parts[0].clone());
            hints.push(format!("打开文件：{}", parts[0]));
            let project = parts[parts.len() - 1].clone();
            fields.insert("project_name".into(), project.clone());
            hints.push(format!("项目：{project}"));
        }
    }

    if let Some(ws) = workspace_from_exe(&seg.exe_path) {
        fields.insert("exe_path".into(), ws);
    }
    fields.insert("window_title_raw".into(), title.to_string());

    SegmentEnrichment {
        app_kind: "ide".into(),
        fields,
        hints,
    }
}

fn enrich_terminal(seg: &Segment, title: &str) -> SegmentEnrichment {
    let mut fields = HashMap::new();
    let mut hints = vec![format!("应用：终端（{}）", seg.app_name)];

    if !title.is_empty() {
        fields.insert("session_title".into(), title.to_string());
        hints.push(format!("终端标题/路径：{title}"));
    }
    if let Some(ws) = workspace_from_exe(&seg.exe_path) {
        fields.insert("cwd_hint".into(), ws);
    }

    SegmentEnrichment {
        app_kind: "terminal".into(),
        fields,
        hints,
    }
}

fn enrich_chat(_seg: &Segment, title: &str, app_label: &str) -> SegmentEnrichment {
    let mut fields = HashMap::new();
    let mut hints = vec![format!("应用：{app_label}")];

    if !title.is_empty() && !title.eq_ignore_ascii_case(app_label) {
        fields.insert("chat_target".into(), title.to_string());
        hints.push(format!("聊天对象/群名：{title}"));
    }
    fields.insert("window_title_raw".into(), title.to_string());

    SegmentEnrichment {
        app_kind: "chat".into(),
        fields,
        hints,
    }
}

fn enrich_generic(seg: &Segment, title: &str) -> SegmentEnrichment {
    let mut fields = HashMap::new();
    let mut hints = vec![format!("应用：{}", seg.app_name)];

    if !title.is_empty() {
        fields.insert("window_title_raw".into(), title.to_string());
        let parts = split_title_parts(title);
        if parts.len() >= 2 {
            fields.insert("primary".into(), parts[0].clone());
            fields.insert("context".into(), parts[1..].join(" - "));
            hints.push(format!("窗口主标题：{}", parts[0]));
            hints.push(format!("附加上下文：{}", parts[1..].join(" - ")));
        } else {
            hints.push(format!("窗口标题：{title}"));
        }
    }

    if !seg.exe_path.is_empty() {
        fields.insert(
            "exe_path".into(),
            Path::new(&seg.exe_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&seg.exe_path)
                .to_string(),
        );
    }

    SegmentEnrichment {
        app_kind: "other".into(),
        fields,
        hints,
    }
}

fn split_title_parts(title: &str) -> Vec<String> {
    title
        .split(" - ")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn strip_suffix_app(parts: &mut Vec<String>, app_names: &[&str]) {
    if let Some(last) = parts.last() {
        let l = last.to_lowercase();
        if app_names.iter().any(|a| l == *a) {
            parts.pop();
        }
    }
}

fn strip_profile_suffix(parts: &mut Vec<String>) {
    if let Some(last) = parts.last() {
        let l = last.to_lowercase();
        if l.contains("profile") || l.contains("用户配置") || l.contains("inprivate") || l.contains("inprivate")
        {
            parts.pop();
        }
    }
    if let Some(last) = parts.last() {
        if last.contains("和另外") || last.to_lowercase().contains("more page") {
            parts.pop();
        }
    }
}

fn looks_like_filename(s: &str) -> bool {
    let exts = [
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".cpp", ".c", ".h",
        ".md", ".json", ".toml", ".yaml", ".yml", ".vue", ".css", ".html", ".sql",
    ];
    let lower = s.to_lowercase();
    exts.iter().any(|e| lower.ends_with(e))
}

fn workspace_from_exe(exe_path: &str) -> Option<String> {
    if exe_path.is_empty() {
        return None;
    }
    Some(exe_path.replace('\\', "/"))
}

fn workspace_folder_name(path: &str) -> Option<String> {
    let p = path.trim_end_matches('/');
    Path::new(p)
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .or_else(|| {
            Path::new(p)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
}

fn friendly_browser_name(seg: &Segment) -> String {
    let k = seg.aggregation_key.to_lowercase();
    if k.contains("msedge") {
        "Microsoft Edge".into()
    } else if k.contains("chrome") {
        "Google Chrome".into()
    } else if k.contains("firefox") {
        "Firefox".into()
    } else {
        seg.app_name.clone()
    }
}

fn infer_site_from_title(title: &str) -> Option<String> {
    let t = title.to_lowercase();
    const SITES: &[(&str, &str)] = &[
        ("github.com", "GitHub"),
        ("gitlab", "GitLab"),
        ("stackoverflow", "Stack Overflow"),
        ("bilibili", "哔哩哔哩"),
        ("youtube", "YouTube"),
        ("zhihu", "知乎"),
        ("douban", "豆瓣"),
        ("notion", "Notion"),
        ("figma", "Figma"),
        ("volcengine", "火山引擎"),
        ("console", "控制台"),
        ("docs.google", "Google 文档"),
        ("localhost", "本地开发"),
    ];
    for (needle, label) in SITES {
        if t.contains(needle) {
            return Some(format!("{label} ({needle})"));
        }
    }
    None
}

fn extract_url_fragment(title: &str) -> Option<String> {
    let start = title.find("http")?;
    let rest = &title[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == '」')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
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
    fn cursor_project_and_file() {
        let e = enrich_segment(&seg(
            "mod.rs - HANDAILY - Cursor",
            "Cursor",
            "cursor.exe",
        ));
        assert_eq!(e.app_kind, "cursor");
        assert!(e.fields.contains_key("open_document"));
        assert!(e.hints.iter().any(|h| h.contains("HANDAILY")));
    }

    #[test]
    fn cursor_session_title() {
        let e = enrich_segment(&seg(
            "优化时间线 AI 注入 - HANDAILY - Cursor",
            "Cursor",
            "cursor.exe",
        ));
        assert!(e.fields.contains_key("session_title") || e.fields.contains_key("project_name"));
    }

    #[test]
    fn edge_page_title() {
        let e = enrich_segment(&seg(
            "火山方舟控制台 - 豆包大模型 - Microsoft Edge",
            "Edge",
            "msedge.exe",
        ));
        assert_eq!(e.app_kind, "browser");
        assert!(e.fields.contains_key("page_title"));
    }
}
