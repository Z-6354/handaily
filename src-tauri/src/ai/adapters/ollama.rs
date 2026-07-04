use super::openai::filter_by_kind;
use crate::ai::catalog::VendorDefinition;
use crate::ai::config::ModelKind;

pub async fn list_models(
    def: &VendorDefinition,
    kind: ModelKind,
) -> Result<Vec<super::RemoteModel>, String> {
    let all = fetch_tags(&def.base_url).await?;
    Ok(filter_by_kind(all, kind))
}

pub async fn test_connection(def: &VendorDefinition) -> Result<String, String> {
    let models = fetch_tags(&def.base_url).await?;
    Ok(format!("连接成功，发现 {} 个本地模型", models.len()))
}

pub async fn chat_text_with_options(
    def: &VendorDefinition,
    model: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
    options: super::ChatOptions,
) -> Result<String, String> {
    let url = format!("{}/api/chat", def.base_url.trim_end_matches('/'));
    let mut messages = Vec::new();
    if let Some(sys) = system_prompt.filter(|s| !s.trim().is_empty()) {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }
    messages.push(serde_json::json!({"role": "user", "content": user_prompt}));
    let body = serde_json::json!({
        "model": model,
        "stream": false,
        "messages": messages,
        "options": {
            "num_predict": options.max_tokens
        }
    });
    let client = super::openai::http_client(options.timeout_secs)?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!(
                    "Ollama 请求超时（{url}）。解析参考文本可能需要较长时间，请稍后重试或缩短参考文本。"
                )
            } else {
                format!("Ollama 请求失败: {e}")
            }
        })?;
    if !resp.status().is_success() {
        return Err(format!("Ollama 错误: {}", resp.text().await.unwrap_or_default()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let message = json["message"].clone();
    if message.is_null() {
        return Err("Ollama 响应无效：缺少 message".into());
    }
    crate::ai::response::extract_ollama_message(&message).ok_or_else(|| {
        "AI 返回内容为空。请检查本地模型是否正常，或更换思考模型。".into()
    })
}

pub async fn chat_vision(
    def: &VendorDefinition,
    model: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
    data_url: &str,
) -> Result<String, String> {
    let url = format!("{}/api/chat", def.base_url.trim_end_matches('/'));
    let mut messages: Vec<serde_json::Value> = Vec::new();
    if let Some(sys) = system_prompt.filter(|s| !s.trim().is_empty()) {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": user_prompt,
        "images": [data_url.strip_prefix("data:image/jpeg;base64,").unwrap_or(data_url)]
    }));
    let body = serde_json::json!({
        "model": model,
        "stream": false,
        "messages": messages
    });
    let client = super::openai::http_client(120)?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama 视觉请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Ollama 错误: {}", resp.text().await.unwrap_or_default()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    json["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Ollama 响应无效".into())
}

async fn fetch_tags(base_url: &str) -> Result<Vec<super::RemoteModel>, String> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let client = super::openai::http_client(30)?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Ollama 请求失败: {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(json["models"]
        .as_array()
        .ok_or("Ollama 响应无效")?
        .iter()
        .filter_map(|m| {
            let name = m["name"].as_str()?;
            Some(super::RemoteModel {
                id: name.to_string(),
                name: name.to_string(),
            })
        })
        .collect())
}
