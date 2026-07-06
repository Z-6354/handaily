//! 混合分析协调器：文本优先，低置信度时尝试截图

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::analysis::guard::SystemGuard;
use crate::analysis::{AnalysisJob, AnalysisSettings};
use crate::analysis::{text, vision};
use crate::db::insights::{insert_insight, InsightRow, TodayAnalysisStats};
use crate::screenshot;
use crate::state::AppState;
use crate::tracker::Segment;

const MAX_QUEUE: usize = 32;

pub struct AnalysisCoordinator {
    tx: SyncSender<AnalysisJob>,
    guard: Arc<SystemGuard>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl AnalysisCoordinator {
    pub fn spawn(state: Weak<AppState>) -> Self {
        let (tx, rx) = mpsc::sync_channel(MAX_QUEUE);
        let guard = Arc::new(SystemGuard::new());
        let worker_guard = guard.clone();

        let worker = thread::Builder::new()
            .name("analysis-worker".into())
            .spawn(move || worker_loop(state, rx, worker_guard))
            .expect("analysis worker thread");

        Self {
            tx,
            guard,
            worker: Mutex::new(Some(worker)),
        }
    }

    pub fn enqueue_segment(&self, segment: Segment) {
        if segment.is_idle {
            return;
        }
        let job = AnalysisJob { segment };
        match self.tx.try_send(job) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                eprintln!("xiaohan-daily: analysis queue full, dropping segment");
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    pub fn guard(&self) -> &SystemGuard {
        &self.guard
    }

    pub fn join_worker(&self) {
        if let Ok(mut handle) = self.worker.lock() {
            if let Some(h) = handle.take() {
                let _ = h.join();
            }
        }
    }
}

fn worker_loop(state: Weak<AppState>, rx: Receiver<AnalysisJob>, guard: Arc<SystemGuard>) {
    crate::tracker::dampen_thread_priority();
    loop {
        let Some(st) = state.upgrade() else { break };
        if st.stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(job) => process_job(&st, &guard, job),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn process_job(state: &AppState, guard: &SystemGuard, job: AnalysisJob) {
    if !state
        .tracking_enabled
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return;
    }
    let settings = {
        let db = match state.lock_db() {
            Ok(d) => d,
            Err(_) => return,
        };
        AnalysisSettings::load(&db)
    };

    if !settings.hybrid_enabled {
        return;
    }

    let seg = &job.segment;
    if seg.duration_ms < settings.min_segment_ms {
        return;
    }

    let text = text::analyze(seg);
    let now = chrono::Local::now().to_rfc3339();

    let _ = persist_insight(state, seg, "text", &text.category, &text.summary, text.confidence, &now);

    if !text.needs_screenshot {
        return;
    }

    if let Err(reason) = guard.can_screenshot(&settings, &seg.exe_path, &seg.aggregation_key) {
        let skip_summary = format!("文本分析不足，已跳过截图：{reason}");
        let _ = persist_insight(state, seg, "text", &text.category, &skip_summary, text.confidence, &now);
        return;
    }

    let jpeg = match screenshot::capture_foreground_jpeg(1280, 70) {
        Ok(j) => j,
        Err(e) => {
            let _ = persist_insight(
                state,
                seg,
                "text",
                &text.category,
                &format!("截图失败：{e}"),
                text.confidence,
                &now,
            );
            return;
        }
    };

    guard.mark_screenshot_taken();

    let data_dir = state.data_dir();
    let vision_result = {
        let prep = {
            let db = match state.lock_db() {
                Ok(d) => d,
                Err(_) => return,
            };
            vision::prepare_screenshot(seg, &settings, &state.vault, &db, data_dir)
        };
        match prep {
            Ok(Some(prepared)) => Ok(vision::execute_or_fallback(&prepared, &jpeg, seg)),
            Ok(None) => Ok(vision::fallback_insight(seg)),
            Err(e) => Err(e),
        }
    };

    match vision_result {
        Ok(v) => {
            let _ = persist_insight(state, seg, "screenshot", &v.category, &v.summary, v.confidence, &now);
        }
        Err(e) => {
            let _ = persist_insight(
                state,
                seg,
                "text",
                &text.category,
                &format!("截图分析失败：{e}"),
                text.confidence,
                &now,
            );
        }
    }
    // jpeg 在此 drop，不落盘
}

fn persist_insight(
    state: &AppState,
    seg: &Segment,
    source: &str,
    category: &str,
    summary: &str,
    confidence: f32,
    created_at: &str,
) -> Result<(), String> {
    let db = state.lock_db()?;
    insert_insight(
        &db,
        &InsightRow {
            started_at: seg.started_at.clone(),
            ended_at: seg.ended_at.clone(),
            app_name: seg.app_name.clone(),
            window_title: seg.window_title.clone(),
            aggregation_key: seg.aggregation_key.clone(),
            source: source.into(),
            category: category.into(),
            summary: summary.into(),
            confidence,
            created_at: created_at.into(),
        },
    )
    .map_err(|e| e.to_string())
}

pub fn query_today_stats(state: &AppState) -> Result<TodayAnalysisStats, String> {
    let db = state.lock_db()?;
    crate::db::insights::today_stats(&db).map_err(|e| e.to_string())
}
