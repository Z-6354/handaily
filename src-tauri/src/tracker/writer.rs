//! segment 写入逻辑：延迟 flush + 短片段合并 + checkpoint + 退出 flush
//!
//! 延迟 flush 设计（见计划修正点⑤）：
//! - 闭合时不立即写 DB，先放 pending_segment
//! - 下一次闭合时判断：pending 是短片段（<2s）且与当前 open 同 agg_key → 合并进 open
//! - 否则 pending 落盘
//! - 这样 DB 永不出现需要 DELETE 的短行

use std::sync::Arc;

use crate::state::AppState;
use crate::tracker::Segment;

/// 短片段阈值（ms）——小于此值且与即将开启的新段同 agg_key 则合并
pub(crate) const SHORT_SEGMENT_MS: u64 = 2000;

/// 判断 pending 短片段是否应合并进即将开启的新 segment
pub fn should_merge_pending_into_new(pending: &Segment, new_seg: &Segment) -> bool {
    should_merge_pending_with_keys(
        pending,
        &new_seg.aggregation_key,
        &crate::tracker::activity_key::activity_key_for_segment(new_seg),
    )
}

fn should_merge_pending_with_keys(
    pending: &Segment,
    new_agg_key: &str,
    new_activity_key: &str,
) -> bool {
    pending.duration_ms < SHORT_SEGMENT_MS
        && pending.aggregation_key == new_agg_key
        && crate::tracker::activity_key::activity_key_for_segment(pending) == new_activity_key
}

/// 闭合当前 open segment：评估 pending，把 closing 落盘或暂存
pub fn close_and_open(
    state: &AppState,
    now_iso: &str,
    new_agg_key: &str,
    new_activity_key: &str,
) {
    let closing = take_closing_open(state, now_iso);

    let mut pending_guard = state.pending_segment.lock().unwrap();
    let old_pending = pending_guard.take();

    if let Some(pend) = old_pending {
        if should_merge_pending_with_keys(&pend, new_agg_key, new_activity_key) {
            *pending_guard = Some(pend);
        } else {
            flush_segment(state, &pend);
        }
    }

    if let Some(clos) = closing {
        if pending_guard.is_some() {
            // 已有待合并的短片段，当前段直接落盘
            flush_segment(state, &clos);
        } else {
            *pending_guard = Some(clos);
        }
    }
}

/// 开启新 segment 时，把 pending 短片段合并进去（延长 started_at）
pub fn merge_pending_into_new_segment(state: &AppState, new_seg: &mut Segment) {
    let mut pending_guard = state.pending_segment.lock().unwrap();
    if let Some(pend) = pending_guard.take() {
        if should_merge_pending_into_new(&pend, new_seg) {
            new_seg.started_at = pend.started_at.clone();
        } else {
            flush_segment(state, &pend);
        }
    }
}

fn take_closing_open(state: &AppState, now_iso: &str) -> Option<Segment> {
    let mut open_guard = state.open_segment.lock().unwrap();
    let closing = open_guard.take();
    drop(open_guard);

    closing.map(|mut seg| {
        seg.ended_at = Some(now_iso.to_string());
        seg.duration_ms = duration_ms(&seg.started_at, &seg.ended_at);
        seg
    })
}

/// 定期 flush：仅落盘 pending 短片段 + WAL passive checkpoint（不拆分 open segment）
pub fn checkpoint(state: &AppState) {
    let mut pending_guard = state.pending_segment.lock().unwrap();
    if let Some(pend) = pending_guard.take() {
        flush_segment(state, &pend);
    }
    drop(pending_guard);

    if let Ok(db) = state.lock_db() {
        let _ = db.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
    }
}

/// 退出前把 pending + open 落盘（计算 duration 并走 flush_segment）
pub fn flush_all_segments(state: &AppState) {
    checkpoint(state);

    let mut open_guard = state.open_segment.lock().unwrap();
    if let Some(mut seg) = open_guard.take() {
        let now = chrono::Local::now().to_rfc3339();
        if seg.ended_at.is_none() {
            seg.ended_at = Some(now);
        }
        seg.duration_ms = duration_ms(&seg.started_at, &seg.ended_at);
        drop(open_guard);
        flush_segment(state, &seg);
    }
}

/// 退出 flush：segments 落盘 + WAL checkpoint
pub fn flush_on_exit(state: &Arc<AppState>) {
    flush_all_segments(state);
    state.finalize_shutdown();
}

/// 键鼠/文件增量写入 daily_metrics
pub fn flush_input_metrics(state: &AppState) {
    let (mouse, keys, text, created, modified) = state.input_stats.take_flush_delta();
    if let Ok(db) = state.lock_db() {
        let _ = crate::db::metrics::upsert_delta(&db, mouse, keys, &text, created, modified);
    }
}

/// 暂停采集：闭合进行中的 foreground 段并落盘，避免恢复后把暂停时长算进 segment
pub fn pause_tracking(state: &AppState) {
    flush_all_segments(state);
    *state.last_snapshot.lock().unwrap() = None;
    flush_input_metrics(state);
}

/// 增量更新聚合缓存
fn apply_to_aggregator(state: &AppState, seg: &Segment) {
    if let Ok(mut agg) = state.aggregator.write() {
        agg.apply(seg);
    }
}

/// 从 started_at 和 ended_at 计算 duration_ms
pub fn duration_ms(started: &str, ended: &Option<String>) -> u64 {
    compute_duration_ms(started, ended)
}

/// 后台音频 segment 落盘（也进入分析与聚合）
pub fn flush_audio_segment(state: &AppState, seg: &Segment) {
    flush_segment(state, seg);
}

/// 写一行到 DB；成功后再更新聚合缓存并触发混合分析
fn flush_segment(state: &AppState, seg: &Segment) {
    let inserted = match state.lock_db() {
        Ok(db) => crate::db::insert_segment(&db, seg).is_ok(),
        Err(_) => false,
    };
    if !inserted {
        crate::log::warn(format!(
            "insert_segment failed ({}, {} ms)",
            seg.started_at, seg.duration_ms
        ));
        return;
    }
    apply_to_aggregator(state, seg);
    if state
        .tracking_enabled
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        state.analysis.enqueue_segment(seg.clone());
    }
}

/// 从 started_at 和 ended_at 计算 duration_ms
fn compute_duration_ms(started: &str, ended: &Option<String>) -> u64 {
    let end = match ended {
        Some(e) => e,
        None => return 0,
    };
    let start = match chrono::DateTime::parse_from_rfc3339(started) {
        Ok(d) => d.with_timezone(&chrono::Local),
        Err(_) => return 0,
    };
    let end = match chrono::DateTime::parse_from_rfc3339(end) {
        Ok(d) => d.with_timezone(&chrono::Local),
        Err(_) => return 0,
    };
    (end - start).num_milliseconds().max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::Segment;

    fn seg(key: &str, duration_ms: u64) -> Segment {
        Segment {
            started_at: "2026-07-02T10:00:00+08:00".into(),
            ended_at: Some("2026-07-02T10:00:01+08:00".into()),
            duration_ms,
            app_name: "app".into(),
            exe_path: key.into(),
            window_title: String::new(),
            is_idle: false,
            aggregation_key: key.into(),
            icon: None,
            source_type: "foreground".into(),
            audio_activity: String::new(),
            activity_label: None,
        }
    }

    #[test]
    fn short_segment_merge_same_app() {
        let pending = seg("notepad.exe", 1500);
        let new_seg = seg("notepad.exe", 0);
        assert!(should_merge_pending_into_new(&pending, &new_seg));
    }

    #[test]
    fn short_segment_no_merge_different_app() {
        let pending = seg("notepad.exe", 1500);
        let new_seg = seg("chrome.exe", 0);
        assert!(!should_merge_pending_into_new(&pending, &new_seg));
    }

    #[test]
    fn short_segment_no_merge_different_activity() {
        let mut pending = seg("cursor.exe", 1500);
        pending.window_title = "a.rs - PROJECT_A - Cursor".into();
        let mut new_seg = seg("cursor.exe", 0);
        new_seg.window_title = "b.rs - PROJECT_B - Cursor".into();
        assert!(!should_merge_pending_into_new(&pending, &new_seg));
    }

    #[test]
    fn long_segment_no_merge() {
        let pending = seg("notepad.exe", 5000);
        let new_seg = seg("notepad.exe", 0);
        assert!(!should_merge_pending_into_new(&pending, &new_seg));
    }

    #[test]
    fn compute_duration_ms_basic() {
        let started = "2026-07-02T10:00:00+08:00";
        let ended = Some("2026-07-02T10:00:05+08:00".into());
        assert_eq!(compute_duration_ms(started, &ended), 5000);
    }
}
