//! 微信推送会话：context_token 与待发队列

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::account::wechat_dir;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WechatSession {
    pub owner_wx_user_id: String,
    pub last_context_token: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOutbound {
    pub messages: Vec<String>,
}

fn session_path(data_dir: &Path) -> std::path::PathBuf {
    wechat_dir(data_dir).join("session.json")
}

fn pending_path(data_dir: &Path) -> std::path::PathBuf {
    wechat_dir(data_dir).join("pending.json")
}

fn sync_buf_path(data_dir: &Path) -> std::path::PathBuf {
    wechat_dir(data_dir).join("sync-buf.txt")
}

pub fn load_session(data_dir: &Path) -> Option<WechatSession> {
    let raw = fs::read_to_string(session_path(data_dir)).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_session(data_dir: &Path, session: &WechatSession) -> Result<(), String> {
    fs::create_dir_all(wechat_dir(data_dir)).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(session).map_err(|e| e.to_string())?;
    fs::write(session_path(data_dir), json).map_err(|e| e.to_string())
}

pub fn ensure_owner_session(data_dir: &Path, owner_wx_user_id: &str) -> Result<(), String> {
    if load_session(data_dir).is_some() {
        return Ok(());
    }
    save_session(
        data_dir,
        &WechatSession {
            owner_wx_user_id: owner_wx_user_id.to_string(),
            last_context_token: None,
            updated_at: chrono::Local::now().to_rfc3339(),
        },
    )
}

pub fn update_context_token(data_dir: &Path, token: &str) -> Result<(), String> {
    let mut session = load_session(data_dir).unwrap_or(WechatSession {
        owner_wx_user_id: String::new(),
        last_context_token: None,
        updated_at: chrono::Local::now().to_rfc3339(),
    });
    session.last_context_token = Some(token.to_string());
    session.updated_at = chrono::Local::now().to_rfc3339();
    save_session(data_dir, &session)
}

pub fn clear_context_token(data_dir: &Path) -> Result<(), String> {
    let Some(mut session) = load_session(data_dir) else {
        return Ok(());
    };
    session.last_context_token = None;
    session.updated_at = chrono::Local::now().to_rfc3339();
    save_session(data_dir, &session)
}

pub fn load_sync_buf(data_dir: &Path) -> String {
    fs::read_to_string(sync_buf_path(data_dir)).unwrap_or_default()
}

pub fn save_sync_buf(data_dir: &Path, buf: &str) -> Result<(), String> {
    fs::create_dir_all(wechat_dir(data_dir)).map_err(|e| e.to_string())?;
    fs::write(sync_buf_path(data_dir), buf).map_err(|e| e.to_string())
}

pub fn clear_pending(data_dir: &Path) -> Result<(), String> {
    save_pending(data_dir, &PendingOutbound { messages: vec![] })
}

pub fn enqueue_pending(data_dir: &Path, text: &str) -> Result<(), String> {
    let mut pending = load_pending(data_dir);
    if pending.messages.iter().any(|m| m == text) {
        return Ok(());
    }
    const MAX_PENDING: usize = 5;
    if pending.messages.len() >= MAX_PENDING {
        pending.messages.remove(0);
    }
    pending.messages.push(text.to_string());
    save_pending(data_dir, &pending)
}

pub fn load_pending(data_dir: &Path) -> PendingOutbound {
    let raw = fs::read_to_string(pending_path(data_dir)).ok();
    raw.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(PendingOutbound { messages: vec![] })
}

pub fn save_pending(data_dir: &Path, pending: &PendingOutbound) -> Result<(), String> {
    fs::create_dir_all(wechat_dir(data_dir)).map_err(|e| e.to_string())?;
    if pending.messages.is_empty() {
        let path = pending_path(data_dir);
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let json = serde_json::to_string_pretty(pending).map_err(|e| e.to_string())?;
    fs::write(pending_path(data_dir), json).map_err(|e| e.to_string())
}

pub fn take_pending(data_dir: &Path) -> Vec<String> {
    let pending = load_pending(data_dir);
    let msgs = pending.messages;
    let _ = save_pending(data_dir, &PendingOutbound { messages: vec![] });
    msgs
}

pub fn clear_session_data(data_dir: &Path) -> Result<(), String> {
    for name in ["session.json", "pending.json", "sync-buf.txt"] {
        let path = wechat_dir(data_dir).join(name);
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
