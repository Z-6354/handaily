//! 微信 iLink Bot 账号持久化

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_BASE_URL: &str = "https://ilinkai.weixin.qq.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HanagentAccountFile {
    bot_token: String,
    account_id: String,
    base_url: String,
    user_id: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WechatAccount {
    pub bot_token: String,
    pub account_id: String,
    pub base_url: String,
    pub user_id: String,
    pub created_at: String,
    /// `qr` 扫码绑定；`hanagent` 自 HANAGENT 导入（可能与其它 Agent 共用 Bot）
    #[serde(default)]
    pub source: String,
}

pub fn wechat_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("wechat")
}

fn account_path(data_dir: &Path) -> PathBuf {
    wechat_dir(data_dir).join("account.json")
}

pub fn hanagent_wechat_dir() -> Option<PathBuf> {
    std::env::var("HANAGENT_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("USERPROFILE").ok().map(|p| PathBuf::from(p).join(".hanagent")))
        .map(|home| home.join("wechat-ilink"))
}

pub fn load_account(data_dir: &Path) -> Option<WechatAccount> {
    let path = account_path(data_dir);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// 识别绑定来源（兼容旧版无 `source` 字段的账号）
pub fn account_bind_source(data_dir: &Path, account: &WechatAccount) -> String {
    if !account.source.is_empty() {
        return account.source.clone();
    }
    if let Some(ha_dir) = hanagent_wechat_dir() {
        let account_file = ha_dir.join("accounts").join("default.json");
        if let Ok(raw) = fs::read_to_string(&account_file) {
            if let Ok(ha) = serde_json::from_str::<HanagentAccountFile>(&raw) {
                if ha.account_id == account.account_id {
                    return "hanagent".into();
                }
            }
        }
    }
    let _ = data_dir;
    "qr".into()
}

pub fn save_account(data_dir: &Path, account: &WechatAccount) -> Result<(), String> {
    let dir = wechat_dir(data_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = account_path(data_dir);
    let json = serde_json::to_string_pretty(account).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn clear_account(data_dir: &Path) -> Result<(), String> {
    let path = account_path(data_dir);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 从 HANAGENT `~/.hanagent/wechat-ilink` 导入账号与会话（若本地尚未绑定）
pub fn try_import_from_hanagent(data_dir: &Path) -> Result<bool, String> {
    if load_account(data_dir).is_some() {
        return Ok(false);
    }
    let Some(src) = hanagent_wechat_dir() else {
        return Ok(false);
    };
    let account_file = src.join("accounts").join("default.json");
    if !account_file.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(&account_file).map_err(|e| e.to_string())?;
    let ha: HanagentAccountFile = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if ha.bot_token.is_empty() || ha.user_id.is_empty() {
        return Ok(false);
    }
    let account = WechatAccount {
        bot_token: ha.bot_token,
        account_id: ha.account_id,
        base_url: if ha.base_url.is_empty() {
            DEFAULT_BASE_URL.to_string()
        } else {
            ha.base_url
        },
        user_id: ha.user_id.clone(),
        created_at: ha.created_at,
        source: "hanagent".into(),
    };
    save_account(data_dir, &account)?;

    // 同步 sync-buf（不导入 context_token：通常已过期且与其它 Agent 冲突）
    let sync_src = src.join("sync-buf.txt");
    if sync_src.exists() {
        if let Ok(buf) = fs::read_to_string(&sync_src) {
            let _ = super::session::save_sync_buf(data_dir, &buf);
        }
    }

    let _ = super::session::ensure_owner_session(data_dir, &account.user_id);

    crate::log::info("已从 HANAGENT 导入微信 iLink 账号（需在本应用 ClawBot 会话中重新激活推送）");
    Ok(true)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HanagentSessionFile {
    session_id: Option<String>,
    last_context_token: Option<String>,
    updated_at: Option<String>,
}

fn owner_from_hanagent_session(parsed: &HanagentSessionFile, fallback: &str) -> String {
    parsed
        .session_id
        .as_deref()
        .and_then(|id| id.strip_prefix("wx_"))
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

#[allow(dead_code)]
fn import_hanagent_session_token(data_dir: &Path, src: &Path, account: &WechatAccount) {
    if let Ok(entries) = fs::read_dir(src.join("sessions")) {
        let safe_id = sanitize_wx_id(&account.user_id);
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.contains(&safe_id) {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(entry.path()) {
                if let Ok(parsed) = serde_json::from_str::<HanagentSessionFile>(&raw) {
                    let owner = owner_from_hanagent_session(&parsed, &account.user_id);
                    if let Some(token) = parsed.last_context_token.filter(|t| !t.is_empty()) {
                        let _ = super::session::save_session(
                            data_dir,
                            &super::session::WechatSession {
                                owner_wx_user_id: owner,
                                last_context_token: Some(token),
                                updated_at: parsed.updated_at.unwrap_or_else(|| {
                                    chrono::Local::now().to_rfc3339()
                                }),
                            },
                        );
                        break;
                    }
                }
            }
        }
    }
}

fn sanitize_wx_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
