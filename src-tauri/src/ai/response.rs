//! 从各供应商 chat 响应中提取 assistant 文本

use serde_json::Value;

/// 从 OpenAI 兼容 `choices[0].message` 提取文本
pub fn extract_openai_message(message: &Value) -> Option<String> {
    message
        .get("content")
        .and_then(extract_content_value)
        .or_else(|| message.get("reasoning_content").and_then(extract_string))
        .or_else(|| message.get("reasoning").and_then(extract_string))
}

/// 从 Ollama `message` 对象提取文本
pub fn extract_ollama_message(message: &Value) -> Option<String> {
    message
        .get("content")
        .and_then(extract_content_value)
        .or_else(|| message.get("thinking").and_then(extract_string))
}

fn extract_content_value(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return non_empty(s);
    }
    if let Some(parts) = value.as_array() {
        let mut out = String::new();
        for part in parts {
            if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                out.push_str(text);
            }
        }
        return non_empty(&out);
    }
    None
}

fn extract_string(value: &Value) -> Option<String> {
    value.as_str().and_then(non_empty)
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn openai_string_content() {
        let msg = json!({"content": "{\"name\":\"测试\"}"});
        assert!(extract_openai_message(&msg).unwrap().contains("测试"));
    }

    #[test]
    fn openai_array_content() {
        let msg = json!({"content": [{"type": "text", "text": "hello"}]});
        assert_eq!(extract_openai_message(&msg).unwrap(), "hello");
    }

    #[test]
    fn openai_reasoning_fallback() {
        let msg = json!({
            "content": "",
            "reasoning_content": "{\"name\":\"柴郡\"}"
        });
        assert!(extract_openai_message(&msg).unwrap().contains("柴郡"));
    }

    #[test]
    fn ollama_thinking_fallback() {
        let msg = json!({
            "content": "",
            "thinking": "分析中…\n{\"name\":\"本地\"}"
        });
        assert!(extract_ollama_message(&msg).unwrap().contains("本地"));
    }
}
