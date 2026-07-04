//! 从模型回复中提取 JSON / 清理 Markdown 包裹

/// 去掉 ```lang ... ``` 或 ``` ... ``` 代码块（lang 可为 json、markdown 等）
pub fn strip_md_fence(s: &str) -> String {
    let t = s.trim();
    if !t.starts_with("```") {
        return t.to_string();
    }
    let mut inner = &t[3..];
    while let Some(c) = inner.chars().next() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            inner = &inner[c.len_utf8()..];
        } else {
            break;
        }
    }
    let inner = inner.trim_start_matches('\n').trim_end();
    let inner = if inner.ends_with("```") {
        inner[..inner.len() - 3].trim_end()
    } else {
        inner
    };
    inner.trim().to_string()
}

/// 从混合文本中提取第一个 JSON 对象 `{…}`
pub fn extract_json_object(raw: &str) -> String {
    let t = strip_md_fence(raw);
    if t.starts_with('{') {
        return t;
    }
    if let Some(start) = t.find('{') {
        if let Some(end) = t.rfind('}') {
            return t[start..=end].to_string();
        }
    }
    t
}

/// 从混合文本中提取第一个 JSON 数组 `[…]`
pub fn extract_json_array(raw: &str) -> String {
    let t = strip_md_fence(raw);
    if t.starts_with('[') {
        return t;
    }
    if let Some(start) = t.find('[') {
        if let Some(end) = t.rfind(']') {
            return t[start..=end].to_string();
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_fence() {
        let s = "```json\n{\"a\":1}\n```";
        assert_eq!(extract_json_object(s), "{\"a\":1}");
        let md = "```markdown\n# 标题\n\n正文\n```";
        assert_eq!(strip_md_fence(md), "# 标题\n\n正文");
    }

    #[test]
    fn extract_from_prose() {
        let s = "分析如下：{\"category\":\"开发\",\"summary\":\"写代码\",\"confidence\":0.8}";
        assert!(extract_json_object(s).contains("开发"));
    }
}
