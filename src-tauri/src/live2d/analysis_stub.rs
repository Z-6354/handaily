//! [live2d-only] 分析/时段调度空实现

use std::sync::mpsc::SyncSender;
use std::sync::{Mutex, Weak};
use std::thread::JoinHandle;

use crate::state::AppState;
use crate::tracker::Segment;

#[derive(Default)]
pub struct SystemGuard;

impl SystemGuard {
    pub fn new() -> Self {
        Self
    }
}

pub struct AnalysisCoordinator {
    _tx: SyncSender<()>,
    _worker: Mutex<Option<JoinHandle<()>>>,
}

impl AnalysisCoordinator {
    pub fn spawn(_state: Weak<AppState>) -> Self {
        let (_tx, _rx) = std::sync::mpsc::sync_channel(1);
        Self {
            _tx,
            _worker: Mutex::new(None),
        }
    }

    pub fn enqueue_segment(&self, _segment: Segment) {}

    pub fn join_worker(&self) {}
}

pub struct PeriodScheduler {
    _tx: SyncSender<()>,
    _worker: Mutex<Option<JoinHandle<()>>>,
    _ticker: Mutex<Option<JoinHandle<()>>>,
}

impl PeriodScheduler {
    pub fn spawn(_state: Weak<AppState>) -> Self {
        let (_tx, _rx) = std::sync::mpsc::sync_channel(1);
        Self {
            _tx,
            _worker: Mutex::new(None),
            _ticker: Mutex::new(None),
        }
    }

    pub fn on_app_switch(
        &self,
        _prev_agg_key: &str,
        _started_at: &str,
        _ended_at: &str,
        _duration_ms: u64,
    ) {
    }

    pub fn join_all(&self) {}
}
