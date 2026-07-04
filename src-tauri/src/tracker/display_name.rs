//! 应用友好显示名（msedge -> Microsoft Edge 等）

use std::path::Path;

use super::title_parse;

/// 已知 exe stem / 进程名 -> 友好名称
fn known_name(stem: &str) -> Option<&'static str> {
    match stem.to_lowercase().as_str() {
        "msedge" => Some("Microsoft Edge"),
        "chrome" => Some("Google Chrome"),
        "firefox" => Some("Mozilla Firefox"),
        "brave" => Some("Brave"),
        "opera" => Some("Opera"),
        "vivaldi" => Some("Vivaldi"),
        "arc" => Some("Arc"),
        "code" | "code - insiders" => Some("VS Code"),
        "cursor" => Some("Cursor"),
        "devenv" => Some("Visual Studio"),
        "windowsterminal" | "wt" => Some("Windows Terminal"),
        "powershell" | "pwsh" => Some("PowerShell"),
        "cmd" => Some("命令提示符"),
        "explorer" => Some("资源管理器"),
        "notepad" => Some("记事本"),
        "wechat" | "weixin" => Some("微信"),
        "dingtalk" => Some("钉钉"),
        "feishu" => Some("飞书"),
        "qq" => Some("QQ"),
        "discord" => Some("Discord"),
        "slack" => Some("Slack"),
        "teams" => Some("Microsoft Teams"),
        "zoom" => Some("Zoom"),
        "spotify" => Some("Spotify"),
        "figma" => Some("Figma"),
        "photoshop" => Some("Adobe Photoshop"),
        "illustrator" => Some("Adobe Illustrator"),
        "excel" => Some("Microsoft Excel"),
        "winword" => Some("Microsoft Word"),
        "powerpnt" => Some("Microsoft PowerPoint"),
        "outlook" => Some("Microsoft Outlook"),
        "onedrive" => Some("OneDrive"),
        "clion" | "idea64" | "pycharm64" | "webstorm64" | "goland64" => Some("JetBrains IDE"),
        "xiaohan-daily" => Some("小寒日报"),
        "desktop" => Some("桌面"),
        _ => None,
    }
}

/// 从 exe 路径、进程名或窗口标题解析友好显示名
pub fn friendly_name(exe_path: &str, app_name: &str, window_title: &str) -> String {
    let stem = if !app_name.is_empty() {
        app_name.to_string()
    } else if !exe_path.is_empty() {
        Path::new(exe_path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else if let Some(from_title) = title_parse::app_name_from_title(window_title) {
        from_title
    } else {
        String::new()
    };

    if let Some(name) = known_name(&stem) {
        return name.to_string();
    }

    // UWP package family: Microsoft.WindowsCalculator_...
    if stem.contains('.') && stem.contains('_') {
        let part = stem.split('_').next().unwrap_or(&stem);
        if part.starts_with("Microsoft.") || part.contains('.') {
            return part
                .trim_start_matches("Microsoft.")
                .replace('.', " ");
        }
    }

    if stem.is_empty() {
        "未知应用".to_string()
    } else {
        stem.to_string()
    }
}

/// 从聚合键（多为 exe 全路径）取显示名
pub fn friendly_from_key(aggregation_key: &str) -> String {
    if aggregation_key == "desktop" {
        return "桌面".to_string();
    }
    if title_parse::is_ignored_agg_key(aggregation_key) {
        return "未知应用".to_string();
    }
    let name = Path::new(aggregation_key)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| aggregation_key.to_string());
    friendly_name(aggregation_key, &name, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msedge_display() {
        assert_eq!(
            friendly_name(
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                "msedge",
                ""
            ),
            "Microsoft Edge"
        );
    }
}
