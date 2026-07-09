//! 微信后台：getupdates 轮询 + 定时推送

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::{Local, Timelike};

use crate::state::AppState;

use super::account::load_account;
use super::ilink::IlinkClient;
use super::push::{
    apply_inbound_context, bootstrap_past_hours, flush_pending, is_push_enabled,
    maybe_push_previous_hour, maybe_push_yesterday_daily, send_startup_message,
};
use super::session::{clear_pending, load_sync_buf, save_sync_buf};

static STARTUP_PUSH_SENT: AtomicBool = AtomicBool::new(false);
static HOUR_BOOTSTRAP_DONE: AtomicBool = AtomicBool::new(false);

pub struct WechatRuntime {
    #[allow(dead_code)]
    rt: Arc<tokio::runtime::Runtime>,
    poll: Mutex<Option<JoinHandle<()>>>,
    scheduler: Mutex<Option<JoinHandle<()>>>,
}

impl WechatRuntime {
    pub fn join_all(&self) {
        for slot in [&self.poll, &self.scheduler] {
            if let Ok(mut guard) = slot.lock() {
                if let Some(h) = guard.take() {
                    let _ = h.join();
                }
            }
        }
    }
}

pub fn spawn(st: Arc<AppState>) -> WechatRuntime {
    STARTUP_PUSH_SENT.store(false, Ordering::Relaxed);
    HOUR_BOOTSTRAP_DONE.store(false, Ordering::Relaxed);

    let data_dir = st.data_dir().to_path_buf();
    if load_account(&data_dir).is_some() {
        if let Ok(db) = st.lock_db() {
            if is_push_enabled(&db) {
                bootstrap_past_hours(&db);
                let pending = super::session::load_pending(&data_dir);
                if pending.messages.len() > 1 {
                    let n = pending.messages.len();
                    let _ = clear_pending(&data_dir);
                    crate::log::info(format!("已清除 {n} 条积压待发微信消息"));
                }
            }
        }
    }

    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("wechat-rt")
            .build()
            .expect("wechat tokio runtime"),
    );

    let poll_st = st.clone();
    let poll_rt = rt.clone();
    let poll = thread::Builder::new()
        .name("wechat-poll".into())
        .spawn(move || poll_loop(poll_st, poll_rt))
        .ok();

    let sched_st = st.clone();
    let sched_rt = rt.clone();
    let scheduler = thread::Builder::new()
        .name("wechat-scheduler".into())
        .spawn(move || scheduler_loop(sched_st, sched_rt))
        .ok();

    WechatRuntime {
        rt,
        poll: Mutex::new(poll),
        scheduler: Mutex::new(scheduler),
    }
}

fn poll_loop(st: Arc<AppState>, rt: Arc<tokio::runtime::Runtime>) {
    crate::tracker::dampen_thread_priority();
    loop {
        if st.stop_flag.load(Ordering::Relaxed) {
            break;
        }
        let data_dir = st.data_dir();
        let Some(account) = load_account(data_dir) else {
            thread::sleep(Duration::from_secs(15));
            continue;
        };
        let push_on = st
            .lock_db()
            .map(|db| is_push_enabled(&db))
            .unwrap_or(false);

        let client = IlinkClient::from_account(&account);
        let sync_buf = load_sync_buf(data_dir);
        match rt.block_on(client.get_updates(&sync_buf)) {
            Ok(updates) => {
                if let Some(buf) = updates.get_updates_buf.as_deref().filter(|s| !s.is_empty()) {
                    let _ = save_sync_buf(data_dir, buf);
                }
                for msg in updates.msgs.unwrap_or_default() {
                    if msg.message_type != Some(1) {
                        continue;
                    }
                    if msg.message_state != Some(2) && msg.message_state.is_some() {
                        continue;
                    }
                    let wx_user = msg
                        .from_user_id
                        .as_deref()
                        .filter(|id| !id.is_empty() && *id != account.account_id);
                    if let (Some(wx_user), Some(token)) = (
                        wx_user,
                        msg.context_token.as_deref().filter(|t| !t.is_empty()),
                    ) {
                        let _ = apply_inbound_context(data_dir, &account, wx_user, token);
                        if push_on {
                            let st2 = st.clone();
                            let _ = rt.block_on(flush_pending(&st2));
                        }
                    }
                }
            }
            Err(e) => {
                crate::log::warn(format!("微信 getupdates 失败: {e}"));
                thread::sleep(Duration::from_secs(5));
            }
        }
        // getupdates 本身长轮询 ~35s，成功后短歇即可
        thread::sleep(Duration::from_secs(1));
    }
}

fn scheduler_loop(st: Arc<AppState>, rt: Arc<tokio::runtime::Runtime>) {
    crate::tracker::dampen_thread_priority();

    loop {
        if st.stop_flag.load(Ordering::Relaxed) {
            break;
        }
        if load_account(st.data_dir()).is_none() {
            thread::sleep(Duration::from_secs(15));
            continue;
        }
        let push_on = st
            .lock_db()
            .map(|db| is_push_enabled(&db))
            .unwrap_or(false);
        if !push_on {
            thread::sleep(Duration::from_secs(30));
            continue;
        }

        if !HOUR_BOOTSTRAP_DONE.swap(true, Ordering::Relaxed) {
            if let Ok(db) = st.lock_db() {
                bootstrap_past_hours(&db);
            }
        }

        if !STARTUP_PUSH_SENT.swap(true, Ordering::Relaxed) {
            let st2 = st.clone();
            if let Err(e) = rt.block_on(run_startup_push(&st2)) {
                crate::log::warn(format!("微信启动推送失败: {e}"));
            }
        }

        let st2 = st.clone();
        if let Err(e) = rt.block_on(maybe_push_previous_hour(&st2)) {
            crate::log::warn(format!("微信小时推送失败: {e}"));
        }

        let now = Local::now();
        if now.hour() == 0 && now.minute() < 3 {
            let st2 = st.clone();
            if let Err(e) = rt.block_on(maybe_push_yesterday_daily(&st2)) {
                crate::log::warn(format!("微信日报推送失败: {e}"));
            }
        }

        thread::sleep(Duration::from_secs(60));
    }
}

async fn run_startup_push(st: &AppState) -> Result<(), String> {
    send_startup_message(st).await?;
    maybe_push_yesterday_daily(st).await
}
