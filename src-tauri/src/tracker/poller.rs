//! 采样循环 + segment 合并/切分
//!
//! 后台线程每 2s 采样前台窗口 → 结合 idle 状态 → 决定延长/闭合/开启 segment。
//! 退出时由 `stop_flag` 触发跳出循环，退出前显式 flush。

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::{Local, TimeZone};

use crate::state::AppState;
use crate::tracker::idle;
use crate::tracker::win32;
use crate::tracker::writer;
use crate::tracker::Segment;
use crate::tracker::Snapshot;

/// 采样间隔（秒）
const POLL_INTERVAL_SECS: u64 = 2;
/// 采集暂停时的采样间隔（秒）——降低空转唤醒
const POLL_IDLE_SECS: u64 = 5;
/// 定期 flush 间隔（秒）——仅 flush pending + WAL，不拆分 open segment
const CHECKPOINT_SECS: u64 = 60;

/// 启动后台采样线程，返回 JoinHandle 供退出时 join
pub fn spawn_poller(state: Arc<AppState>) -> JoinHandle<()> {
    thread::spawn(move || {
        crate::tracker::dampen_thread_priority();
        let mut tick = 0u64;
        loop {
            // 检查停机
            if state.stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            if !state
                .tracking_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                thread::sleep(Duration::from_secs(POLL_IDLE_SECS));
                continue;
            }

            let threshold = read_idle_threshold(&state);

            let idle = idle::is_idle(threshold);
            if let Some(mut snap) = win32::get_foreground_snapshot() {
                snap.is_idle = idle;
                process_snapshot(&state, &snap);
            } else if idle {
                // 无前台窗口且用户空闲：记 idle 段，不产生 unknown
                process_snapshot(&state, &idle_placeholder_snapshot());
            }
            // 无前台且非空闲（如本应用在前台）：跳过本 tick，延续上一段

            // 每 60s：flush pending 短片段 + WAL（open segment 保持内存连续，切换/退出时再落盘）
            tick += POLL_INTERVAL_SECS;
            if tick >= CHECKPOINT_SECS {
                writer::flush_input_metrics(&state);
                writer::checkpoint(&state);
                tick = 0;
            }

            thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
        }
        // 退出前 flush
        writer::flush_on_exit(&state);
    })
}

/// 空闲占位快照（无前台 HWND 时使用）
fn idle_placeholder_snapshot() -> Snapshot {
    Snapshot {
        pid: 0,
        exe_path: String::new(),
        app_name: String::new(),
        window_title: String::new(),
        captured_at: Local::now(),
        is_idle: true,
    }
}

/// 处理一帧快照：决定延长当前 segment、闭合并开启新段、或 idle 切换
fn process_snapshot(state: &AppState, snap: &Snapshot) {
    *state.foreground.lock().unwrap() = Some(snap.to_payload());

    // 跨日检查：若 open segment 的日期与今天不同，先切分
    rollover_if_new_day(state);

    let agg = crate::tracker::derive_agg_key_for_snap(snap);
    let key = crate::state::SnapshotKey {
        aggregation_key: agg.clone(),
        activity_key: crate::tracker::activity_key::activity_key_for_fields(
            &snap.exe_path,
            &snap.app_name,
            &snap.window_title,
            &agg,
        ),
        is_idle: snap.is_idle,
    };

    let mut last = state.last_snapshot.lock().unwrap();
    let now = snap.captured_at.to_rfc3339();

    if let Some(ref prev) = *last {
        if prev == &key {
            extend_open(state, &now);
            // 跨日后 open 已清空但 key 未变时，补开新段
            if state.open_segment.lock().unwrap().is_none() {
                let mut new_seg = Segment::from_snapshot(snap);
                writer::merge_pending_into_new_segment(state, &mut new_seg);
                *state.open_segment.lock().unwrap() = Some(new_seg);
            }
            return;
        }
    }

    // 不同段：闭合当前 open（走延迟 flush），开启新段
    if let Some(ref prev_seg) = *state.open_segment.lock().unwrap() {
        if !prev_seg.is_idle && prev_seg.aggregation_key != key.aggregation_key {
            let end = now.clone();
            state.period_scheduler.on_app_switch(
                &prev_seg.aggregation_key,
                &prev_seg.started_at,
                &end,
                prev_seg
                    .ended_at
                    .as_ref()
                    .and_then(|e| {
                        chrono::DateTime::parse_from_rfc3339(e).ok().and_then(|end_dt| {
                            chrono::DateTime::parse_from_rfc3339(&prev_seg.started_at)
                                .ok()
                                .map(|s| {
                                    (end_dt.timestamp_millis() - s.timestamp_millis()).max(0) as u64
                                })
                        })
                    })
                    .unwrap_or(prev_seg.duration_ms),
            );
        }
    }
    writer::close_and_open(state, &now, &key.aggregation_key, &key.activity_key);
    *last = Some(key);

    // 用快照数据开启新 open segment（短片段合并进 started_at）
    let mut new_seg = Segment::from_snapshot(snap);
    writer::merge_pending_into_new_segment(state, &mut new_seg);
    *state.open_segment.lock().unwrap() = Some(new_seg);
}

/// 延长 open segment 的 ended_at
fn extend_open(state: &AppState, now_iso: &str) {
    if let Some(ref mut seg) = *state.open_segment.lock().unwrap() {
        seg.ended_at = Some(now_iso.to_string());
        // duration 在闭合时计算，这里只更新 ended_at
    }
}

/// 跨日切换：若 open segment 的日期 != 今天，闭合它（ended_at=昨天 23:59:59.999），重建缓存
fn rollover_if_new_day(state: &AppState) {
    let today = Local::now().date_naive();
    let need_rollover = {
        let open = state.open_segment.lock().unwrap();
        if let Some(ref seg) = *open {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&seg.started_at) {
                dt.with_timezone(&Local).date_naive() != today
            } else {
                false
            }
        } else {
            false
        }
    };

    if need_rollover {
        // 闭合 open（算昨天 23:59:59.999 本地时区）
        let yesterday = today.pred_opt().unwrap_or(today);
        let end_of_yesterday = yesterday
            .and_hms_milli_opt(23, 59, 59, 999)
            .and_then(|dt| chrono::Local.from_local_datetime(&dt).single())
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| Local::now().to_rfc3339());
        writer::close_and_open(state, &end_of_yesterday, "", "");
        *state.last_snapshot.lock().unwrap() = None;
        // 重建今天的聚合缓存
        if let Ok(db) = state.lock_db() {
            if let Ok(agg) = crate::db::stats::rebuild_aggregator(&db, today) {
                *state.aggregator.write().unwrap() = agg;
            }
        }
    }
}

/// 读 idle 阈值设置
fn read_idle_threshold(state: &AppState) -> u64 {
    state.idle_threshold_secs()
}

#[cfg(test)]
mod tests {
    use crate::state::SnapshotKey;

    #[test]
    fn segment_merge_same_window() {
        let key = SnapshotKey {
            aggregation_key: "notepad.exe".into(),
            activity_key: "untitled".into(),
            is_idle: false,
        };
        assert_eq!(key, key.clone());
    }

    #[test]
    fn segment_split_on_switch() {
        let a = SnapshotKey {
            aggregation_key: "notepad.exe".into(),
            activity_key: "doc-a".into(),
            is_idle: false,
        };
        let b = SnapshotKey {
            aggregation_key: "chrome.exe".into(),
            activity_key: "page-b".into(),
            is_idle: false,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn segment_split_on_same_app_different_project() {
        let a = SnapshotKey {
            aggregation_key: "cursor.exe".into(),
            activity_key: "handaily".into(),
            is_idle: false,
        };
        let b = SnapshotKey {
            aggregation_key: "cursor.exe".into(),
            activity_key: "other".into(),
            is_idle: false,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn segment_split_on_idle() {
        let active = SnapshotKey {
            aggregation_key: "notepad.exe".into(),
            activity_key: "untitled".into(),
            is_idle: false,
        };
        let idle = SnapshotKey {
            aggregation_key: "notepad.exe".into(),
            activity_key: "untitled".into(),
            is_idle: true,
        };
        assert_ne!(active, idle);
    }
}
