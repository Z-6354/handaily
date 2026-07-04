//! 时段 AI 更新调度：启动 5 分钟、整点、长时间切换应用

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use chrono::{Local, TimeZone, Timelike};

use crate::analysis::period::PeriodAnalysisResult;
use crate::db::periods;
use crate::state::AppState;

const LONG_SESSION_MS: u64 = 15 * 60 * 1000; // 15 分钟
const TICK_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct PeriodJob {
    pub start_iso: String,
    pub end_iso: String,
    pub trigger: &'static str,
}

pub struct PeriodScheduler {
    tx: SyncSender<PeriodJob>,
    worker: Mutex<Option<JoinHandle<()>>>,
    ticker: Mutex<Option<JoinHandle<()>>>,
}

impl PeriodScheduler {
    pub fn spawn(state: Weak<AppState>) -> Self {
        let (tx, rx) = mpsc::sync_channel(16);
        let worker_tx = tx.clone();
        let worker_state = state.clone();
        let worker = thread::Builder::new()
            .name("period-ai-worker".into())
            .spawn(move || worker_loop(worker_state, rx))
            .expect("period worker");

        let ticker_state = state.clone();
        let startup_done = Arc::new(AtomicBool::new(false));
        let last_hour: Arc<Mutex<Option<(String, u32)>>> = Arc::new(Mutex::new(None));
        let started = Instant::now();

        let sd = startup_done.clone();
        let lh = last_hour.clone();
        let ticker = thread::Builder::new()
            .name("period-scheduler".into())
            .spawn(move || {
                loop {
                    let Some(st) = ticker_state.upgrade() else { break };
                    if st.stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    check_triggers(&worker_tx, &sd, &lh, started);
                    thread::sleep(Duration::from_secs(TICK_SECS));
                }
            })
            .expect("period scheduler");

        Self {
            tx,
            worker: Mutex::new(Some(worker)),
            ticker: Mutex::new(Some(ticker)),
        }
    }

    pub fn join_all(&self) {
        for slot in [&self.worker, &self.ticker] {
            if let Ok(mut guard) = slot.lock() {
                if let Some(h) = guard.take() {
                    let _ = h.join();
                }
            }
        }
    }

    /// 应用切换且上一段足够长时，排队分析
    pub fn on_app_switch(
        &self,
        prev_key: &str,
        session_start_iso: &str,
        session_end_iso: &str,
        duration_ms: u64,
    ) {
        if duration_ms < LONG_SESSION_MS || prev_key.is_empty() || prev_key == "__idle__" {
            return;
        }
        let job = PeriodJob {
            start_iso: session_start_iso.to_string(),
            end_iso: session_end_iso.to_string(),
            trigger: "long_session",
        };
        let _ = self.tx.try_send(job);
    }
}

fn check_triggers(
    tx: &SyncSender<PeriodJob>,
    startup_done: &AtomicBool,
    last_hour: &Mutex<Option<(String, u32)>>,
    started: Instant,
) {
    let now = Local::now();

    // 启动后 5 分钟
    if !startup_done.load(Ordering::Relaxed) && started.elapsed() >= Duration::from_secs(300) {
        startup_done.store(true, Ordering::Relaxed);
        let end = now.to_rfc3339();
        let start = (now - chrono::Duration::minutes(5)).to_rfc3339();
        let _ = tx.try_send(PeriodJob {
            start_iso: start,
            end_iso: end,
            trigger: "startup_5m",
        });
    }

    // 整点：分析上一小时
    if now.minute() < 2 {
        let prev_hour = if now.hour() == 0 { 23 } else { now.hour() - 1 };
        let date_naive = if now.hour() == 0 {
            now.date_naive() - chrono::Duration::days(1)
        } else {
            now.date_naive()
        };
        let date_key = date_naive.format("%Y-%m-%d").to_string();
        let mut guard = last_hour.lock().unwrap();
        let already = guard
            .as_ref()
            .map(|(d, h)| d == &date_key && *h == prev_hour)
            .unwrap_or(false);
        if !already {
            *guard = Some((date_key, prev_hour));
            let start = date_naive
                .and_hms_opt(prev_hour, 0, 0)
                .and_then(|dt| chrono::Local.from_local_datetime(&dt).single())
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| now.to_rfc3339());
            let end = date_naive
                .and_hms_opt(prev_hour, 59, 59)
                .and_then(|dt| chrono::Local.from_local_datetime(&dt).single())
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| now.to_rfc3339());
            let _ = tx.try_send(PeriodJob {
                start_iso: start,
                end_iso: end,
                trigger: "hourly",
            });
        }
    }
}

fn worker_loop(state: Weak<AppState>, rx: Receiver<PeriodJob>) {
    loop {
        let Some(st) = state.upgrade() else { break };
        if st.stop_flag.load(Ordering::Relaxed) {
            break;
        }
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(job) => {
                let _ = run_period_job(&st, &job);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn run_period_job(state: &AppState, job: &PeriodJob) -> Result<(), String> {
    let data_dir = state.data_dir();
    let segments = {
        let db = crate::db::lock_conn(&state.db)?;
        periods::query_segments_in_range(&db, &job.start_iso, &job.end_iso)
            .map_err(|e| e.to_string())?
    };
    let (work_types, prep) = {
        let db = crate::db::lock_conn(&state.db)?;
        let work_types = crate::work_type::WorkTypeConfig::load(&db);
        let ai_config = crate::ai::AiConfig::load(&db, data_dir);
        let prep = crate::analysis::period::prepare_period_chat(
            &segments,
            &work_types,
            &ai_config,
            &state.vault,
            &db,
            data_dir,
        )?;
        (work_types, prep)
    };

    let ai_raw = match prep {
        Some(p) => match p.run_sync() {
            Ok(raw) => Some(raw),
            Err(e) => {
                eprintln!(
                    "xiaohan-daily: period AI failed ({}): {e}",
                    job.trigger
                );
                None
            }
        },
        None => None,
    };
    let result = crate::analysis::period::finalize_period_analysis(&segments, &work_types, ai_raw);

    persist_period(state, job, &result)
}

fn persist_period(
    state: &AppState,
    job: &PeriodJob,
    result: &PeriodAnalysisResult,
) -> Result<(), String> {
    let now = Local::now().to_rfc3339();
    let db = state.lock_db()?;
    periods::insert_period_summary(
        &db,
        &job.start_iso,
        &job.end_iso,
        &result.work_type,
        &result.summary,
        job.trigger,
        &now,
    )
    .map_err(|e| e.to_string())?;
    periods::apply_work_type_to_hours(
        &db,
        &job.start_iso,
        &job.end_iso,
        &result.work_type,
        &result.summary,
        &now,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
