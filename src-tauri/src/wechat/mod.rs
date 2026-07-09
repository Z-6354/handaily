//! 微信 iLink 绑定与定时推送（参考 HANAGENT channels/wechat）

mod account;
mod ilink;
mod push;
mod qr;
mod scheduler;
mod session;

use std::sync::Arc;

use chrono::Local;
use serde::Serialize;

use crate::state::AppState;

pub use scheduler::{spawn, WechatRuntime};

#[derive(Debug, Clone, Serialize)]
pub struct WechatStatus {
    pub bound: bool,
    pub push_enabled: bool,
    pub account_id: Option<String>,
    pub user_id: Option<String>,
    pub session_ready: bool,
    pub pending_count: usize,
    /// 已绑定但通道未就绪，通常需重新扫码（尤其与其它 Agent 共用 Bot 时）
    pub needs_rebind: bool,
    pub bind_source: Option<String>,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WechatQrStart {
    pub qrcode_id: String,
    pub qrcode_data_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WechatQrPoll {
    pub status: String,
    pub bound: bool,
    pub retmsg: Option<String>,
}

pub fn get_status(st: &AppState) -> Result<WechatStatus, String> {
    let data_dir = st.data_dir();
    let account = account::load_account(data_dir);
    let db = st.lock_db()?;
    let push_enabled = push::is_push_enabled(&db);
    let session_ready = push::session_ready(data_dir);
    let pending = session::load_pending(data_dir).messages.len();
    let bound = account.is_some();
    let bind_source = account
        .as_ref()
        .map(|a| account::account_bind_source(data_dir, a));
    let needs_rebind = bound && !session_ready;
    let hint = if !bound {
        "未绑定：扫码登录微信 ClawBot 插件后即可推送".into()
    } else if !session_ready {
        if bind_source.as_deref() == Some("hanagent") {
            "当前绑定来自外部导入，无法激活推送。请点「重新扫码绑定」为小寒日报单独绑定 ClawBot。".into()
        } else {
            "推送通道未激活：绑定成功后请在微信 ClawBot 发任意消息；若仍无效，请重新扫码绑定。".into()
        }
    } else if pending > 0 {
        format!("有 {pending} 条消息等待发送")
    } else {
        "推送通道已就绪".into()
    };
    Ok(WechatStatus {
        bound,
        push_enabled,
        account_id: account.as_ref().map(|a| a.account_id.clone()),
        user_id: account.as_ref().map(|a| a.user_id.clone()),
        session_ready,
        pending_count: pending,
        needs_rebind,
        bind_source,
        hint,
    })
}

pub async fn start_qr_login(_st: &AppState) -> Result<WechatQrStart, String> {
    if account::load_account(_st.data_dir()).is_some() {
        return Err("已绑定微信，请先解绑再重新扫码".into());
    }
    let client = dummy_client();
    let (qrcode_id, content) = client.start_qr_login().await?;
    let qrcode_data_url = qr::to_qr_data_url(&content)?;
    Ok(WechatQrStart {
        qrcode_id,
        qrcode_data_url,
    })
}

pub async fn poll_qr_login(st: &AppState, qrcode_id: &str) -> Result<WechatQrPoll, String> {
    let client = dummy_client();
    let data = client.poll_qr_status(qrcode_id).await?;
    if data.status == "confirmed" {
        let bot_token = data.bot_token.ok_or("缺少 bot_token")?;
        let account_id = data.ilink_bot_id.ok_or("缺少 ilink_bot_id")?;
        let user_id = data.ilink_user_id.ok_or("缺少 ilink_user_id")?;
        let account = account::WechatAccount {
            bot_token,
            account_id,
            base_url: data.baseurl.unwrap_or_else(|| account::DEFAULT_BASE_URL.to_string()),
            user_id: user_id.clone(),
            created_at: Local::now().to_rfc3339(),
            source: "qr".into(),
        };
        account::save_account(st.data_dir(), &account)?;
        session::ensure_owner_session(st.data_dir(), &user_id)?;
        let db = st.lock_db()?;
        push::set_push_enabled(&db, true)?;
        return Ok(WechatQrPoll {
            status: "confirmed".into(),
            bound: true,
            retmsg: None,
        });
    }
    if data.status == "expired" {
        return Ok(WechatQrPoll {
            status: "expired".into(),
            bound: false,
            retmsg: data.retmsg,
        });
    }
    Ok(WechatQrPoll {
        status: data.status,
        bound: false,
        retmsg: data.retmsg,
    })
}

pub fn logout(st: &AppState) -> Result<(), String> {
    account::clear_account(st.data_dir())?;
    session::clear_session_data(st.data_dir())?;
    let db = st.lock_db()?;
    push::set_push_enabled(&db, false)?;
    Ok(())
}

pub fn prepare_rebind(st: &AppState) -> Result<(), String> {
    logout(st)
}

pub fn import_from_hanagent(st: &AppState) -> Result<bool, String> {
    let ok = account::try_import_from_hanagent(st.data_dir())?;
    if ok {
        let db = st.lock_db()?;
        push::set_push_enabled(&db, true)?;
    }
    Ok(ok)
}

pub async fn test_send(st: &AppState) -> Result<String, String> {
    match push::send_or_queue(
        st,
        &format!(
            "✅ 小寒日报微信推送测试\n{}",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
    )
    .await?
    {
        push::SendOutcome::Delivered => Ok("测试消息已发送".into()),
        push::SendOutcome::Queued | push::SendOutcome::Skipped if push::session_ready(st.data_dir()) => {
            Ok("发送频率受限或通道繁忙，请稍后再试".into())
        }
        push::SendOutcome::Queued | push::SendOutcome::Skipped => Ok(
            "会话已过期或未激活：消息已加入待发队列，请在微信 ClawBot 发任意一条消息后自动补发".into(),
        ),
    }
}

pub fn set_push_enabled(st: &AppState, enabled: bool) -> Result<(), String> {
    let db = st.lock_db()?;
    push::set_push_enabled(&db, enabled)
}

fn dummy_client() -> ilink::IlinkClient {
    ilink::IlinkClient::anonymous()
}

pub fn on_startup(st: Arc<AppState>) -> WechatRuntime {
    spawn(st)
}
