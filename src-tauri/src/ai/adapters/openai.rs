use crate::ai::catalog::VendorDefinition;
use crate::ai::config::ModelKind;
use crate::ai::urls;

#[derive(Debug, Clone, serde::Serialize)]
pub struct RemoteModel {
    pub id: String,
    pub name: String,
}

pub async fn list_models(
    def: &VendorDefinition,
    kind: ModelKind,
    api_key: Option<&str>,
) -> Result<Vec<RemoteModel>, String> {
    if def.requires_api_key {
        let key = api_key.filter(|k| !k.trim().is_empty()).ok_or("请先配置 API 密钥")?;
        let key = key.trim();
        match fetch_openai_models(def, key).await {
            Ok(models) if !models.is_empty() => Ok(filter_by_kind(models, kind)),
            Ok(_) if def.test.strategy == "openai_or_ping" => {
                ping_optional(def, key).await?;
                Ok(vec![])
            }
            Ok(_) => Err(format!(
                "模型列表为空。{}",
                def.hints.empty_models.as_deref().unwrap_or("请使用「手动添加」填写模型 ID")
            )),
            Err(e) if def.test.strategy == "openai_or_ping" => match ping_optional(def, key).await {
                Ok(()) => Ok(vec![]),
                Err(ping_err) => {
                    if e.contains("401") {
                        Err(auth_hint(def, &e))
                    } else {
                        Err(format!("{e}；Ping 检测：{ping_err}"))
                    }
                }
            },
            Err(e) => Err(e),
        }
    } else {
        Err("该供应商需要 API 密钥".into())
    }
}

pub async fn test_connection(
    def: &VendorDefinition,
    api_key: Option<&str>,
) -> Result<String, String> {
    let key = api_key.filter(|k| !k.trim().is_empty()).ok_or("请先配置 API 密钥")?;
    let key = key.trim();
    let plan = def
        .test
        .plan_label
        .as_deref()
        .unwrap_or(&def.name);
    match fetch_openai_models(def, key).await {
        Ok(models) if !models.is_empty() => {
            Ok(format!("连接成功，可用模型 {} 个", models.len()))
        }
        Ok(_) if def.test.strategy == "openai_or_ping" => {
            ping_optional(def, key).await?;
            Ok(format!(
                "连接成功，{} 密钥有效（模型列表为空，请导入或手动添加）",
                plan
            ))
        }
        Ok(_) => Ok(format!(
            "连接成功，但模型列表为空。{}",
            def.hints.empty_models.as_deref().unwrap_or("请使用「手动添加」")
        )),
        Err(e) if def.test.strategy == "openai_or_ping" => match ping_optional(def, key).await {
            Ok(()) => Ok(format!(
                "连接成功，{} 密钥有效（/models 不可用，请手动添加模型 ID）",
                plan
            )),
            Err(_) => Err(auth_hint(def, &e)),
        },
        Err(e) => Err(e),
    }
}

pub async fn chat_text_with_options(
    def: &VendorDefinition,
    api_key: &str,
    model: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
    options: super::ChatOptions,
) -> Result<String, String> {
    let url = urls::chat_completions_url(&def.base_url);
    let mut messages = Vec::new();
    if let Some(sys) = system_prompt.filter(|s| !s.trim().is_empty()) {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }
    messages.push(serde_json::json!({"role": "user", "content": user_prompt}));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": options.max_tokens,
        "messages": messages
    });
    post_chat(&url, api_key, &body, options).await
}

pub async fn chat_vision(
    def: &VendorDefinition,
    api_key: &str,
    model: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
    data_url: &str,
) -> Result<String, String> {
    let url = urls::chat_completions_url(&def.base_url);
    let mut messages: Vec<serde_json::Value> = Vec::new();
    if let Some(sys) = system_prompt.filter(|s| !s.trim().is_empty()) {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": [
            { "type": "text", "text": user_prompt },
            { "type": "image_url", "image_url": { "url": data_url } }
        ]
    }));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 200,
        "messages": messages
    });
    post_chat(&url, api_key, &body, super::ChatOptions::default()).await
}

async fn fetch_openai_models(def: &VendorDefinition, api_key: &str) -> Result<Vec<RemoteModel>, String> {
    let url = urls::models_list_url(&def.base_url);
    let client = http_client(60)?;
    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| format!("连接失败: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format_api_error(status.as_u16(), &body, def, &url));
    }
    let trimmed = body.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return Err(format!(
            "模型列表接口返回非 JSON（{}）。请检查供应商 Base URL 是否正确（{}）。",
            truncate(trimmed, 80),
            def.base_url
        ));
    }
    parse_models_json(&body).map_err(|e| format!("解析模型列表失败: {e}"))
}

async fn ping_optional(def: &VendorDefinition, api_key: &str) -> Result<(), String> {
    let url = def
        .test
        .ping_url
        .as_deref()
        .ok_or("未配置 ping_url")?;
    let client = http_client(30)?;
    let resp = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| format!("Ping 连接失败: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if status.is_success() {
        return Ok(());
    }
    Err(format!(
        "Ping HTTP {} · {}",
        status.as_u16(),
        truncate(body.trim(), 120)
    ))
}

fn format_request_error(url: &str, err: reqwest::Error) -> String {
    if err.is_timeout() {
        return format!(
            "API 请求超时（{url}）。解析参考文本可能需要 1～3 分钟，请稍后重试或缩短参考文本。"
        );
    }
    if err.is_connect() {
        return format!(
            "无法连接 API 服务器（{url}）：{err}。请检查网络、防火墙与系统代理设置。"
        );
    }
    format!("API 请求失败（{url}）：{err}")
}

fn retryable_request_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

async fn post_chat(
    url: &str,
    api_key: &str,
    body: &serde_json::Value,
    options: super::ChatOptions,
) -> Result<String, String> {
    let client = http_client(options.timeout_secs)?;

    async fn do_send(
        client: &reqwest::Client,
        url: &str,
        api_key: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let mut req = client.post(url).json(body);
        if !api_key.is_empty() {
            req = req.bearer_auth(api_key);
        }
        req.send().await
    }

    let resp = match do_send(&client, url, api_key, body).await {
        Ok(r) => r,
        Err(e) if options.is_long_running() && retryable_request_error(&e) => {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            do_send(&client, url, api_key, body)
                .await
                .map_err(|e2| format_request_error(url, e2))?
        }
        Err(e) => return Err(format_request_error(url, e)),
    };
    if !resp.status().is_success() {
        return Err(format!("API 错误: {}", resp.text().await.unwrap_or_default()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let message = json["choices"][0]["message"].clone();
    if message.is_null() {
        return Err("API 响应无效：缺少 message".into());
    }
    crate::ai::response::extract_openai_message(&message).ok_or_else(|| {
        let preview = serde_json::to_string(&message).unwrap_or_default();
        format!(
            "AI 返回内容为空。请检查思考模型是否支持 JSON 输出，或更换模型。响应摘要：{}",
            truncate(&preview, 200)
        )
    })
}

pub fn filter_by_kind(models: Vec<RemoteModel>, kind: ModelKind) -> Vec<RemoteModel> {
    if kind == ModelKind::Text || kind == ModelKind::Thinking {
        return models;
    }
    let vision: Vec<_> = models
        .iter()
        .filter(|m| likely_vision_model(&m.id))
        .cloned()
        .collect();
    if vision.is_empty() {
        models
    } else {
        vision
    }
}

fn likely_vision_model(id: &str) -> bool {
    let s = id.to_lowercase();
    [
        "vision", "-vl", "4v", "multimodal", "gpt-4o", "seed-1-8", "seed-1.8",
        "doubao-seed", "seedream", "glm-4v", "llava", "image",
    ]
    .iter()
    .any(|k| s.contains(k))
}

fn parse_models_json(body: &str) -> Result<Vec<RemoteModel>, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("无效 JSON: {e}"))?;
    let items = json["data"]
        .as_array()
        .or_else(|| json["models"].as_array())
        .or_else(|| json["result"].as_array());
    let Some(arr) = items else {
        return Err("响应中未找到 data/models 数组".into());
    };
    let mut out = Vec::new();
    for item in arr {
        let id = item["id"]
            .as_str()
            .or_else(|| item["model"].as_str())
            .or_else(|| item["name"].as_str());
        let Some(id) = id else { continue };
        let name = item["display_name"]
            .as_str()
            .or_else(|| item["name"].as_str())
            .unwrap_or(id);
        out.push(RemoteModel {
            id: id.to_string(),
            name: name.to_string(),
        });
    }
    Ok(out)
}

fn format_api_error(status: u16, body: &str, def: &VendorDefinition, url: &str) -> String {
    let detail = truncate(body.trim(), 200);
    let detail = if detail.is_empty() { "无响应体".into() } else { detail };
    let mut msg = format!("HTTP {status} · {detail}（{url}）");
    if let Some(hint) = &def.hints.auth_error {
        msg.push_str("。");
        msg.push_str(hint);
    }
    msg
}

fn auth_hint(def: &VendorDefinition, prefix: &str) -> String {
    def.hints
        .auth_error
        .as_deref()
        .map(|h| {
            if prefix.is_empty() {
                h.to_string()
            } else {
                format!("{prefix}{h}")
            }
        })
        .unwrap_or_else(|| prefix.to_string())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

pub fn http_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_openai_models_json() {
        let body = r#"{"data":[{"id":"gpt-4o","object":"model"}]}"#;
        let m = parse_models_json(body).unwrap();
        assert_eq!(m[0].id, "gpt-4o");
    }

    #[test]
    fn vision_filter() {
        let all = vec![
            RemoteModel { id: "deepseek-chat".into(), name: "chat".into() },
            RemoteModel { id: "doubao-seed-1-8".into(), name: "seed".into() },
        ];
        let v = filter_by_kind(all, ModelKind::Vision);
        assert_eq!(v.len(), 1);
    }
}
