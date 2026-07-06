//! 时间线 AI 简介后台调度：启动后自动补全今日未缓存片段，不依赖前端打开时间线页

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tauri::AppHandle;

use crate::state::AppState;

use super::describe::run_today_uncached;

const STARTUP_DELAY_SECS: u64 = 12;
const TICK_SECS: u64 = 90;

pub fn spawn(app: AppHandle, st: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(STARTUP_DELAY_SECS)).await;

        loop {
            if st.stop_flag.load(Ordering::Relaxed) {
                break;
            }
            if !st.tracking_enabled.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_secs(TICK_SECS)).await;
                continue;
            }

            match run_today_uncached(&st, &app).await {
                Ok(n) if n > 0 => {
                    eprintln!("xiaohan-daily: timeline background described {n} segments");
                }
                Ok(_) => {}
                Err(e) => eprintln!("xiaohan-daily: timeline background describe failed: {e}"),
            }

            tokio::time::sleep(Duration::from_secs(TICK_SECS)).await;
        }
    });
}
