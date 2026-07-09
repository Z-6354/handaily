//! 微信推送：消息组装与发送

use std::path::Path;

use chrono::{Datelike, Local, NaiveDate, TimeZone, Timelike};

use crate::db;
use crate::report;
use crate::state::AppState;

use super::account::{load_account, WechatAccount};
use super::ilink::{is_rate_limited_error, is_session_timeout_error, IlinkClient};
use super::session::{
    clear_context_token, enqueue_pending, load_session, take_pending, update_context_token,
};

const KEY_PUSH_ENABLED: &str = "wechat_push_enabled";
const KEY_LAST_DAILY: &str = "wechat_last_daily_report";
const KEY_HOUR_PREFIX: &str = "wechat_hour:";
const KEY_STARTUP_PREFIX: &str = "wechat_startup:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    Delivered,
    Queued,
    /// 定时推送限流/通道未就绪：不入队，下轮重试
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueuePolicy {
    /// 用户主动发送（测试）：可入待发队列
    Allow,
    /// 定时推送：不入队，避免启动后堆积
    Deny,
}

pub fn is_push_enabled(db: &rusqlite::Connection) -> bool {
    match db::get_setting(db, KEY_PUSH_ENABLED).as_deref() {
        Some("1") => true,
        Some("0") => false,
        _ => false,
    }
}

pub fn set_push_enabled(db: &rusqlite::Connection, enabled: bool) -> Result<(), String> {
    if enabled {
        suppress_historical_hour_pushes(db);
    }
    db::set_setting(db, KEY_PUSH_ENABLED, if enabled { "1" } else { "0" })
        .map_err(|e| e.to_string())
}

/// 开启推送时，将过去 24 小时标记为已处理，避免首次启动补发洪峰
fn suppress_historical_hour_pushes(db: &rusqlite::Connection) {
    let now = Local::now();
    let mut cursor = now - chrono::Duration::hours(24);
    let end = now - chrono::Duration::hours(1);
    while cursor < end {
        let date = cursor.date_naive();
        let hour = cursor.hour();
        let _ = mark_hour_pushed(db, date, hour);
        cursor += chrono::Duration::hours(1);
    }
}

pub fn bootstrap_past_hours(db: &rusqlite::Connection) {
    suppress_historical_hour_pushes(db);
}

async fn send_text(st: &AppState, text: &str, queue: QueuePolicy) -> Result<SendOutcome, String> {
    let data_dir = st.data_dir();
    let account = load_account(data_dir).ok_or("微信未绑定")?;
    let session = load_session(data_dir);
    let Some(token) = session
        .as_ref()
        .and_then(|s| s.last_context_token.clone())
        .filter(|t| !t.is_empty())
    else {
        if queue == QueuePolicy::Allow {
            enqueue_pending(data_dir, text)?;
            return Ok(SendOutcome::Queued);
        }
        return Ok(SendOutcome::Skipped);
    };
    let owner = session
        .as_ref()
        .map(|s| s.owner_wx_user_id.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(account.user_id.as_str());
    let client = IlinkClient::from_account(&account);
    match client
        .send_text(&account.user_id, owner, &token, text)
        .await
    {
        Ok(()) => Ok(SendOutcome::Delivered),
        Err(e) if is_session_timeout_error(&e) => {
            crate::log::warn("微信 context_token 已过期，清除并排队待发");
            let _ = clear_context_token(data_dir);
            if queue == QueuePolicy::Allow {
                enqueue_pending(data_dir, text)?;
                Ok(SendOutcome::Queued)
            } else {
                Ok(SendOutcome::Skipped)
            }
        }
        Err(e) if is_rate_limited_error(&e) => {
            crate::log::warn("微信发送频率受限，稍后重试");
            Ok(SendOutcome::Skipped)
        }
        Err(e) => Err(e),
    }
}

pub async fn send_or_queue(st: &AppState, text: &str) -> Result<SendOutcome, String> {
    send_text(st, text, QueuePolicy::Allow).await
}

pub async fn flush_pending(st: &AppState) -> Result<(), String> {
    let data_dir = st.data_dir();
    let pending = take_pending(data_dir);
    if pending.is_empty() {
        return Ok(());
    }
    // 每次最多发 1 条，避免通道激活后洪峰
    let msg = &pending[0];
    match send_text(st, msg, QueuePolicy::Allow).await {
        Ok(SendOutcome::Delivered) => {
            for rest in pending.iter().skip(1) {
                let _ = enqueue_pending(data_dir, rest);
            }
        }
        Ok(SendOutcome::Queued) | Ok(SendOutcome::Skipped) => {
            let _ = enqueue_pending(data_dir, msg);
            for rest in pending.iter().skip(1) {
                let _ = enqueue_pending(data_dir, rest);
            }
        }
        Err(e) => {
            crate::log::warn(format!("微信待发消息失败: {e}"));
            let _ = enqueue_pending(data_dir, msg);
            for rest in pending.iter().skip(1) {
                let _ = enqueue_pending(data_dir, rest);
            }
        }
    }
    Ok(())
}

pub async fn send_startup_message(st: &AppState) -> Result<(), String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let startup_key = format!("{KEY_STARTUP_PREFIX}{today}");
    {
        let db = st.lock_db()?;
        if db::get_setting(&db, &startup_key).as_deref() == Some("1") {
            return Ok(());
        }
    }
    let now = Local::now();
    let text = format!(
        "🐾 小寒日报已启动\n时间：{}\n\n今日继续陪你记录工作～",
        now.format("%Y-%m-%d %H:%M")
    );
    match send_text(st, &text, QueuePolicy::Deny).await? {
        SendOutcome::Delivered => {
            let db = st.lock_db()?;
            db::set_setting(&db, &startup_key, "1").map_err(|e| e.to_string())?;
            Ok(())
        }
        SendOutcome::Queued | SendOutcome::Skipped => Ok(()),
    }
}

pub async fn send_hour_summary(st: &AppState, hour: u32, date: NaiveDate) -> Result<SendOutcome, String> {
    let text = {
        let db = st.lock_db()?;
        build_hour_summary(&db, date, hour)?
    };
    send_text(st, &text, QueuePolicy::Deny).await
}

pub async fn send_daily_report(st: &AppState, date: NaiveDate) -> Result<(), String> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let text = {
        let db = st.lock_db()?;
        build_daily_report(&db, st.data_dir(), &date_str)?
    };
    match send_text(st, &text, QueuePolicy::Deny).await? {
        SendOutcome::Delivered => Ok(()),
        SendOutcome::Queued | SendOutcome::Skipped => Ok(()),
    }
}

pub fn build_hour_summary(
    db: &rusqlite::Connection,
    date: NaiveDate,
    hour: u32,
) -> Result<String, String> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let start = Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), hour, 0, 0)
        .single()
        .ok_or("无效时间")?;
    let end = start + chrono::Duration::hours(1);
    let start_iso = start.to_rfc3339();
    let end_iso = end.to_rfc3339();

    let mut lines = vec![format!(
        "⏰ {} {:02}:00–{:02}:59 小结",
        date_str,
        hour,
        hour
    )];

    if let Ok(hours) = crate::db::periods::load_hour_types_for_date(db, date) {
        if let Some(wt) = hours.get(hour as usize).and_then(|x| x.as_ref()) {
            if !wt.summary.is_empty() {
                lines.push(format!("【{}】{}", wt.work_type, wt.summary));
            } else {
                lines.push(format!("【{}】", wt.work_type));
            }
        }
    }

    let periods = crate::db::periods::list_period_summaries_in_range(
        db,
        &date_str,
        &date_str,
        20,
    )
    .map_err(|e| e.to_string())?;
    for p in periods {
        if p.started_at >= start_iso && p.started_at < end_iso {
            lines.push(format!("· {} {}", p.work_type, p.summary));
        }
    }

    let segs = crate::db::periods::query_segments_in_range(db, &start_iso, &end_iso)
        .map_err(|e| e.to_string())?;
    if segs.is_empty() && lines.len() == 1 {
        lines.push("（这一小时没有记录到前台活动）".into());
    } else if !segs.is_empty() {
        let mut apps: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        for s in segs {
            *apps.entry(s.app_name.clone()).or_insert(0) += s.duration_ms;
        }
        let mut ranked: Vec<_> = apps.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        for (app, ms) in ranked.into_iter().take(5) {
            let mins = ms / 60_000;
            if mins > 0 {
                lines.push(format!("· {app} {mins} 分钟"));
            }
        }
    }

    if date == Local::now().date_naive() {
        if let Ok(day_ms) = day_foreground_ms(db, &date_str) {
            let mins = day_ms / 60_000;
            if mins > 0 {
                lines.push(format!("\n今日累计有效 {mins} 分钟"));
            }
        }
    }

    Ok(truncate_chars(&lines.join("\n"), 3500))
}

fn day_foreground_ms(db: &rusqlite::Connection, date_str: &str) -> Result<u64, rusqlite::Error> {
    let ms: i64 = db.query_row(
        "SELECT COALESCE(SUM(duration_ms), 0) FROM activity_segments \
         WHERE is_idle = 0 AND substr(started_at, 1, 10) = ?1",
        [date_str],
        |r| r.get(0),
    )?;
    Ok(ms.max(0) as u64)
}

pub fn build_daily_report(
    db: &rusqlite::Connection,
    _data_dir: &Path,
    date_str: &str,
) -> Result<String, String> {
    let gathered = report::gather(db, report::TEMPLATE_ACTIVITY_LOG, date_str, date_str)?;
    let draft = report::compose(&gathered, None);
    let header = format!("📋 小寒日报 · {}\n\n", date_str);
    Ok(truncate_chars(
        &format!("{header}{}", draft.content),
        3500,
    ))
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max).collect::<String>())
}

pub fn previous_completed_hour(
    now: chrono::DateTime<chrono::Local>,
) -> (NaiveDate, u32) {
    let t = now - chrono::Duration::hours(1);
    (t.date_naive(), t.hour())
}

/// 仅在每小时前 10 分钟推送上一小时小结（每轮调度 60s，实际约 10 次窗口内触发一次）
pub async fn maybe_push_previous_hour(st: &AppState) -> Result<(), String> {
    let db = st.lock_db()?;
    if !is_push_enabled(&db) {
        return Ok(());
    }
    drop(db);

    let now = Local::now();
    if now.minute() >= 10 {
        return Ok(());
    }

    let (date, hour) = previous_completed_hour(now);
    let should = {
        let db = st.lock_db()?;
        should_push_hour(&db, date, hour)
    };
    if !should {
        return Ok(());
    }
    match send_hour_summary(st, hour, date).await {
        Ok(SendOutcome::Delivered) => {
            let db = st.lock_db()?;
            mark_hour_pushed(&db, date, hour)
        }
        Ok(SendOutcome::Queued) | Ok(SendOutcome::Skipped) => Ok(()),
        Err(e) => Err(e),
    }
}

pub async fn maybe_push_yesterday_daily(st: &AppState) -> Result<(), String> {
    let yesterday = Local::now().date_naive() - chrono::Duration::days(1);
    let should = {
        let db = st.lock_db()?;
        if !is_push_enabled(&db) {
            return Ok(());
        }
        should_push_daily(&db, yesterday)
    };
    if !should {
        return Ok(());
    }
    if send_daily_report(st, yesterday).await.is_ok() {
        let db = st.lock_db()?;
        mark_daily_pushed(&db, yesterday)?;
    }
    Ok(())
}

fn hour_setting_key(date: NaiveDate, hour: u32) -> String {
    format!("{KEY_HOUR_PREFIX}{}T{:02}", date.format("%Y-%m-%d"), hour)
}

pub fn should_push_hour(db: &rusqlite::Connection, date: NaiveDate, hour: u32) -> bool {
    db::get_setting(db, &hour_setting_key(date, hour)).as_deref() != Some("1")
}

pub fn mark_hour_pushed(db: &rusqlite::Connection, date: NaiveDate, hour: u32) -> Result<(), String> {
    db::set_setting(db, &hour_setting_key(date, hour), "1").map_err(|e| e.to_string())
}

pub fn should_push_daily(db: &rusqlite::Connection, report_date: NaiveDate) -> bool {
    let key = report_date.format("%Y-%m-%d").to_string();
    db::get_setting(db, KEY_LAST_DAILY).as_deref() != Some(key.as_str())
}

pub fn mark_daily_pushed(db: &rusqlite::Connection, report_date: NaiveDate) -> Result<(), String> {
    let key = report_date.format("%Y-%m-%d").to_string();
    db::set_setting(db, KEY_LAST_DAILY, &key).map_err(|e| e.to_string())
}

pub fn apply_inbound_context(
    data_dir: &Path,
    _account: &WechatAccount,
    wx_user_id: &str,
    token: &str,
) -> Result<(), String> {
    update_context_token(data_dir, token)?;
    let mut session = super::session::load_session(data_dir).unwrap_or(super::session::WechatSession {
        owner_wx_user_id: wx_user_id.to_string(),
        last_context_token: None,
        updated_at: chrono::Local::now().to_rfc3339(),
    });
    session.owner_wx_user_id = wx_user_id.to_string();
    session.last_context_token = Some(token.to_string());
    session.updated_at = chrono::Local::now().to_rfc3339();
    super::session::save_session(data_dir, &session)
}

pub fn session_ready(data_dir: &Path) -> bool {
    load_session(data_dir)
        .and_then(|s| s.last_context_token)
        .is_some_and(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use chrono::NaiveDate;
    use std::path::PathBuf;

    fn test_db() -> rusqlite::Connection {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "xiaohan-wechat-db-{}-{}",
            std::process::id(),
            n
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("t.sqlite");
        db::open_and_migrate(&path).unwrap()
    }

    fn temp_data_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "xiaohan-wechat-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn push_enabled_only_when_explicit() {
        let db = test_db();
        assert!(!is_push_enabled(&db));
        set_push_enabled(&db, true).unwrap();
        assert!(is_push_enabled(&db));
        set_push_enabled(&db, false).unwrap();
        assert!(!is_push_enabled(&db));
    }

    #[test]
    fn hour_and_daily_dedup_keys() {
        let db = test_db();
        let date = NaiveDate::from_ymd_opt(2026, 7, 8).unwrap();
        assert!(should_push_hour(&db, date, 14));
        mark_hour_pushed(&db, date, 14).unwrap();
        assert!(!should_push_hour(&db, date, 14));
        assert!(should_push_hour(&db, date, 15));

        assert!(should_push_daily(&db, date));
        mark_daily_pushed(&db, date).unwrap();
        assert!(!should_push_daily(&db, date));
    }

    #[test]
    fn pending_queue_and_context() {
        let dir = temp_data_dir();
        let account = WechatAccount {
            bot_token: "t".into(),
            account_id: "bot1".into(),
            base_url: super::super::account::DEFAULT_BASE_URL.into(),
            user_id: "owner1".into(),
            created_at: "now".into(),
            source: "qr".into(),
        };
        apply_inbound_context(&dir, &account, "wx_user_1", "ctx_abc").unwrap();
        assert!(session_ready(&dir));
        let session = load_session(&dir).unwrap();
        assert_eq!(session.owner_wx_user_id, "wx_user_1");
        assert_eq!(session.last_context_token.as_deref(), Some("ctx_abc"));

        enqueue_pending(&dir, "hello").unwrap();
        let pending = super::super::session::load_pending(&dir);
        assert_eq!(pending.messages.len(), 1);
        let taken = super::super::session::take_pending(&dir);
        assert_eq!(taken.len(), 1);
        assert!(super::super::session::load_pending(&dir).messages.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn per_hour_dedup_keys_are_independent() {
        let db = test_db();
        let date = NaiveDate::from_ymd_opt(2026, 7, 8).unwrap();
        assert!(should_push_hour(&db, date, 10));
        mark_hour_pushed(&db, date, 10).unwrap();
        assert!(!should_push_hour(&db, date, 10));
        assert!(should_push_hour(&db, date, 11));
    }

    #[test]
    fn rate_limit_error_detection() {
        assert!(super::super::ilink::is_rate_limited_error("发送频率受限，请稍后再试"));
    }

    #[test]
    fn build_hour_summary_empty_db() {
        let db = test_db();
        let date = NaiveDate::from_ymd_opt(2026, 7, 8).unwrap();
        let text = build_hour_summary(&db, date, 10).unwrap();
        assert!(text.contains("10:00"));
        assert!(text.contains("没有记录"));
    }
}
