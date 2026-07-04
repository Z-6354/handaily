//! 从窗口标题等推断应用名（exe 路径不可用时的回退）

/// 从 Windows 窗口标题解析应用名（常见：`文档 - VS Code`）
pub fn app_name_from_title(title: &str) -> Option<String> {
    let t = title.trim();
    if t.is_empty() {
        return None;
    }
    // 取最后一个 " - " / " – " / " — " 之后作为应用名
    let app = t
        .rsplit_once(" - ")
        .or_else(|| t.rsplit_once(" – "))
        .or_else(|| t.rsplit_once(" — "))
        .map(|(_, a)| a.trim())
        .unwrap_or(t);
    if app.is_empty() || app.len() > 120 {
        return None;
    }
    // 过滤纯数字/时间戳类标题
    if app.chars().all(|c| c.is_ascii_digit() || c == ':' || c == '.') {
        return None;
    }
    Some(app.to_string())
}

/// 是否为应忽略的聚合键（不计入应用排行）
pub fn is_ignored_agg_key(key: &str) -> bool {
    let k = key.trim().to_lowercase();
    k.is_empty() || k == "unknown" || k == "__idle__"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_title_app_suffix() {
        assert_eq!(
            app_name_from_title("main.rs - HANDAILY - Visual Studio Code"),
            Some("Visual Studio Code".into())
        );
    }

    #[test]
    fn ignored_keys() {
        assert!(is_ignored_agg_key("unknown"));
        assert!(is_ignored_agg_key("__idle__"));
        assert!(!is_ignored_agg_key("msedge.exe"));
    }
}
