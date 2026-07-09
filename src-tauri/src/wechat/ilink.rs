//! WeChat iLink HTTP API（参考 HANAGENT channels/wechat）

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::random;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::account::{WechatAccount, DEFAULT_BASE_URL};

const MIN_SEND_INTERVAL_MS: u64 = 2500;
const QR_POLL_TIMEOUT_SECS: u64 = 65;

static ILINK_HTTP: OnceLock<Client> = OnceLock::new();

fn ilink_http() -> &'static Client {
    ILINK_HTTP.get_or_init(|| {
        Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .user_agent("xiaohan-daily-wechat/0.1")
            .build()
            .unwrap_or_default()
    })
}

fn format_transport_error(context: &str, err: &reqwest::Error) -> String {
    if err.is_timeout() {
        return format!("{context}超时（微信服务器长轮询，将自动重试）");
    }
    if err.is_connect() {
        return format!(
            "{context}连接失败：请确认本机可访问 ilinkai.weixin.qq.com，并检查系统代理/VPN"
        );
    }
    format!("{context}：{err}")
}

#[derive(Debug, Clone, Deserialize)]
struct QrCodeResponse {
    ret: i32,
    qrcode: Option<String>,
    qrcode_img_content: Option<String>,
    qrcode_img_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QrStatusResponse {
    pub ret: i32,
    pub status: String,
    pub retmsg: Option<String>,
    pub bot_token: Option<String>,
    pub ilink_bot_id: Option<String>,
    pub baseurl: Option<String>,
    pub ilink_user_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetUpdatesResponse {
    pub ret: Option<i32>,
    pub retmsg: Option<String>,
    pub sync_buf: Option<String>,
    pub get_updates_buf: Option<String>,
    pub msgs: Option<Vec<WeixinMessage>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeixinMessage {
    pub from_user_id: Option<String>,
    pub message_type: Option<i32>,
    pub message_state: Option<i32>,
    pub context_token: Option<String>,
    pub item_list: Option<Vec<MessageItem>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageItem {
    pub r#type: Option<i32>,
    pub text_item: Option<TextItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextItem {
    pub text: Option<String>,
}

pub struct IlinkClient {
    http: Client,
    token: String,
    base_url: String,
    uin: String,
    next_send: std::sync::Mutex<HashMap<String, Instant>>,
}

impl IlinkClient {
    pub fn from_account(account: &WechatAccount) -> Self {
        Self {
            http: ilink_http().clone(),
            token: account.bot_token.clone(),
            base_url: sanitize_base_url(&account.base_url),
            uin: B64.encode(random::<[u8; 4]>()),
            next_send: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn anonymous() -> Self {
        Self {
            http: ilink_http().clone(),
            token: String::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
            uin: B64.encode(random::<[u8; 4]>()),
            next_send: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        h.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        h.insert("AuthorizationType", "ilink_bot_token".parse().unwrap());
        h.insert("X-WECHAT-UIN", self.uin.parse().unwrap());
        h
    }

    async fn post_json(&self, path: &str, body: Value, timeout: Duration) -> Result<Value, String> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), path);
        let res = self
            .http
            .post(&url)
            .headers(self.headers())
            .timeout(timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("HTTP {status}: {text}"));
        }
        res.json().await.map_err(|e| e.to_string())
    }

    pub async fn start_qr_login(&self) -> Result<(String, String), String> {
        let url = format!("{DEFAULT_BASE_URL}/ilink/bot/get_bot_qrcode?bot_type=3");
        let res = self
            .http
            .get(&url)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|e| format_transport_error("获取绑定二维码", &e))?;
        if !res.status().is_success() {
            return Err(format!("获取二维码失败: HTTP {}", res.status()));
        }
        let data: QrCodeResponse = res.json().await.map_err(|e| e.to_string())?;
        if data.ret != 0 {
            return Err(format!("获取二维码失败 (ret={})", data.ret));
        }
        let qrcode_id = data.qrcode.ok_or("缺少 qrcode")?;
        let content = data
            .qrcode_img_url
            .or(data.qrcode_img_content)
            .ok_or("缺少 qrcode_img_content")?;
        Ok((qrcode_id, content))
    }

    pub async fn poll_qr_status(&self, qrcode_id: &str) -> Result<QrStatusResponse, String> {
        let res = self
            .http
            .get(format!("{DEFAULT_BASE_URL}/ilink/bot/get_qrcode_status"))
            .query(&[("qrcode", qrcode_id)])
            .timeout(Duration::from_secs(QR_POLL_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| format_transport_error("轮询二维码状态", &e))?;
        if !res.status().is_success() {
            return Err(format!("轮询二维码失败: HTTP {}", res.status()));
        }
        res.json().await.map_err(|e| e.to_string())
    }

    pub async fn get_updates(&self, sync_buf: &str) -> Result<GetUpdatesResponse, String> {
        let body = if sync_buf.is_empty() {
            json!({})
        } else {
            json!({ "get_updates_buf": sync_buf })
        };
        let val = self
            .post_json("ilink/bot/getupdates", body, Duration::from_secs(35))
            .await?;
        serde_json::from_value(val).map_err(|e| e.to_string())
    }

    pub async fn send_text(
        &self,
        bot_user_id: &str,
        to_user_id: &str,
        context_token: &str,
        text: &str,
    ) -> Result<(), String> {
        if context_token.is_empty() {
            return Err("缺少 context_token，请在微信 ClawBot 中先发一条消息".into());
        }
        self.rate_limit_wait(to_user_id).await;
        for segment in split_text(text, 1800) {
            let body = json!({
                "msg": {
                    "from_user_id": bot_user_id,
                    "to_user_id": to_user_id,
                    "client_id": new_client_id(),
                    "message_type": 2,
                    "message_state": 2,
                    "context_token": context_token,
                    "item_list": [{
                        "type": 1,
                        "text_item": { "text": segment }
                    }]
                },
                "base_info": {
                    "channel_version": "2.0.0",
                    "bot_agent": "xiaohan-daily-wechat"
                }
            });
            self.send_with_retry(body, to_user_id).await?;
        }
        Ok(())
    }

    async fn send_with_retry(&self, body: Value, _to_user_id: &str) -> Result<(), String> {
        let mut delay_ms = 4000u64;
        for attempt in 0..=4 {
            let val = self
                .post_json("ilink/bot/sendmessage", body.clone(), Duration::from_secs(15))
                .await?;
            match api_result_code(&val) {
                Ok(()) => return Ok(()),
                Err(code) if code == -2 => {
                    if attempt == 4 {
                        return Err("发送频率受限，请稍后再试".into());
                    }
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms = (delay_ms * 2).min(30_000);
                }
                Err(code) if is_session_timeout_code(code) => {
                    let msg = val
                        .get("errmsg")
                        .or_else(|| val.get("retmsg"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("session timeout");
                    return Err(format!("SESSION_TIMEOUT:{msg}"));
                }
                Err(code) => {
                    let msg = val
                        .get("errmsg")
                        .or_else(|| val.get("retmsg"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    return Err(format!("sendmessage ret={code} {msg}").trim().to_string());
                }
            }
        }
        Ok(())
    }

    async fn rate_limit_wait(&self, user_id: &str) {
        let wait = {
            let mut map = self.next_send.lock().unwrap();
            let now = Instant::now();
            let next = map.get(user_id).copied().unwrap_or(now);
            let send_at = if next > now { next } else { now };
            map.insert(
                user_id.to_string(),
                send_at + Duration::from_millis(MIN_SEND_INTERVAL_MS),
            );
            send_at.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}

/// iLink 成功：`ret==0` 或缺省；失败：显式 `ret`/`errcode` 非 0（例如 errcode:-14 session timeout）。
fn api_result_code(val: &Value) -> Result<(), i64> {
    if let Some(code) = val.get("errcode").and_then(|v| v.as_i64()) {
        if code != 0 {
            return Err(code);
        }
    }
    if let Some(code) = val.get("ret").and_then(|v| v.as_i64()) {
        if code != 0 {
            return Err(code);
        }
    }
    Ok(())
}

fn is_session_timeout_code(code: i64) -> bool {
    code == -14
}

pub fn is_rate_limited_error(err: &str) -> bool {
    err.contains("发送频率受限") || err.contains("rate-limited") || err.contains("ret=-2")
}

pub fn is_session_timeout_error(err: &str) -> bool {
    err.starts_with("SESSION_TIMEOUT:") || err.contains("session timeout")
}

fn sanitize_base_url(base_url: &str) -> String {
    if base_url.is_empty() {
        return DEFAULT_BASE_URL.to_string();
    }
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.starts_with("https://")
        && (trimmed.contains("weixin.qq.com") || trimmed.contains("wechat.com"))
    {
        return trimmed.to_string();
    }
    DEFAULT_BASE_URL.to_string()
}

fn new_client_id() -> String {
    format!("{:032x}", random::<u128>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_result_detects_errcode_session_timeout() {
        let val = json!({"errcode": -14, "errmsg": "session timeout"});
        assert_eq!(api_result_code(&val), Err(-14));
        assert!(is_session_timeout_code(-14));
    }

    #[test]
    fn api_result_ok_when_empty_or_ret_zero() {
        assert!(api_result_code(&json!({})).is_ok());
        assert!(api_result_code(&json!({"ret": 0})).is_ok());
        assert_eq!(api_result_code(&json!({"ret": -2})), Err(-2));
    }
}

fn split_text(text: &str, max_len: usize) -> Vec<String> {
    if text.chars().count() <= max_len {
        return vec![text.to_string()];
    }
    let mut parts = Vec::new();
    let mut rest = text;
    while rest.chars().count() > max_len {
        let cut_byte = rest
            .char_indices()
            .nth(max_len)
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let mut cut = rest[..cut_byte].rfind('\n').unwrap_or(cut_byte);
        if cut < max_len / 2 {
            cut = cut_byte;
        }
        parts.push(rest[..cut].trim_end().to_string());
        rest = rest[cut..].trim_start();
    }
    if !rest.is_empty() {
        parts.push(rest.to_string());
    }
    parts
}
