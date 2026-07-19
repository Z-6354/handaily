//! 跨线程共享的应用状态
//!
//! 并发模型（见计划"并发与状态同步模型"）：
//! - `AppState` 包在 `Arc` 里，setup 时 `manage()` + clone 给后台线程
//! - `rusqlite::Connection` 非 Sync，用 `Mutex<Connection>` 串行
//! - command 用 `async fn`，临界区只 clone 快照不持锁做 I/O
//! - 后台线程退出由 `stop_flag: AtomicBool` 控制，Tauri 不会 join 它

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::thread::JoinHandle;

use rusqlite::Connection;
use tauri::AppHandle;

use crate::db::stats::TodayAggregator;
use crate::live2d::{AnalysisCoordinator, PeriodScheduler, VaultState};
use crate::tracker::input_monitor::InputStatsShared;
use crate::tracker::{ForegroundPayload, Segment};

/// 核心共享状态——后台线程写、command 读
pub struct AppState {
    /// SQLite 连接（非 Sync，必须 Mutex 串行）
    pub db: Mutex<Connection>,
    /// 今日聚合缓存——segment 闭合时增量更新，command 直接读
    pub aggregator: RwLock<TodayAggregator>,
    /// 当前未闭合 segment（内存暂存，延迟 flush 设计）
    pub open_segment: Mutex<Option<Segment>>,
    /// 暂存待合并判断的短片段（延迟 flush 设计，见 writer.rs）
    pub pending_segment: Mutex<Option<Segment>>,
    /// 上一帧快照（用于判断 segment 是否需要切分）
    pub last_snapshot: Mutex<Option<SnapshotKey>>,
    /// 最近一次前台快照（供 IPC 查询）
    pub foreground: Mutex<Option<ForegroundPayload>>,
    /// SQLite 文件路径（供设置页展示）
    pub db_path: std::path::PathBuf,
    /// 采集开关
    pub tracking_enabled: AtomicBool,
    /// 空闲阈值（秒），避免 poller 每 tick 读 DB
    pub idle_threshold_secs: AtomicU64,
    /// 停机标志——后台线程循环检查，true 时退出
    pub stop_flag: AtomicBool,
    /// Tauri 句柄（用于从后台线程发事件到前端）
    pub app: AppHandle,
    /// 密码本解锁状态
    pub vault: VaultState,
    /// 混合分析协调器
    pub analysis: AnalysisCoordinator,
    /// 时段 AI 调度
    pub period_scheduler: PeriodScheduler,
    /// 键鼠与文件指标（内存计数）
    pub input_stats: Arc<InputStatsShared>,
    /// 防止 timeline_describe 并发重复 AI 调用
    pub timeline_describe_lock: tokio::sync::Mutex<()>,
}

/// 用于 segment 切分判断的快照键
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotKey {
    pub aggregation_key: String,
    /// 同一应用内的活动内容（项目名、网页标题等）
    pub activity_key: String,
    pub is_idle: bool,
}

/// 持有后台线程 JoinHandle
pub struct JoinState {
    poller: Mutex<Option<JoinHandle<()>>>,
    input: Mutex<Option<JoinHandle<()>>>,
    file: Mutex<Option<JoinHandle<()>>>,
    audio: Mutex<Option<JoinHandle<()>>>,
}

impl JoinState {
    pub fn new(
        poller: JoinHandle<()>,
        input: JoinHandle<()>,
        file: JoinHandle<()>,
        audio: JoinHandle<()>,
    ) -> Self {
        Self {
            poller: Mutex::new(Some(poller)),
            input: Mutex::new(Some(input)),
            file: Mutex::new(Some(file)),
            audio: Mutex::new(Some(audio)),
        }
    }
    pub fn join_all(&self) {
        self.join_all_timeout(std::time::Duration::from_secs(2));
    }

    /// 退出时 join 后台线程，超时后放弃等待以免托盘图标长时间残留。
    pub fn join_all_timeout(&self, timeout: std::time::Duration) {
        let deadline = std::time::Instant::now() + timeout;
        for slot in [&self.poller, &self.input, &self.file, &self.audio] {
            if std::time::Instant::now() >= deadline {
                return;
            }
            if let Ok(mut guard) = slot.lock() {
                if let Some(handle) = guard.take() {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    let _ = wait_join_timeout(handle, remaining);
                }
            }
        }
    }
}

fn wait_join_timeout(handle: JoinHandle<()>, timeout: std::time::Duration) -> bool {
    if timeout.is_zero() {
        return false;
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = tx.send(handle.join());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(())) => true,
        _ => false,
    }
}

impl AppState {
    /// 初始化：打开 DB、跑 migrate、重建今日聚合缓存、读 settings
    pub fn new(app: &AppHandle) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let db_path = resolve_db_path(app)?;
        let data_dir = db_path.parent().map(|p| p.to_path_buf());
        let db = crate::db::open_and_migrate(&db_path)?;

        if let Some(ref data_dir) = data_dir {
            let _ = crate::prompts::seed_user_prompts(data_dir);
            // [live2d-only] 跳过 AI 供应商与时间线日志目录
            let _ = crate::persona::seed_user_personas(data_dir);
            let _ = crate::character::seed_user_characters(data_dir);
        }

        // 启动兜底：闭合孤儿段与会话
        {
            let now = chrono::Local::now().to_rfc3339();
            crate::db::recover_orphan_segments(&db, &now)?;
            crate::db::sessions::recover_orphan_sessions(&db)?;
            crate::db::usage::recover_orphan_sessions(&db)?;
            let _ = crate::db::usage::open_app_session(&db);
            let _ = crate::pet::models::migrate_legacy_builtin_models(&db);
            if let Some(ref data_dir) = data_dir {
                let _ = crate::persona::migrate_legacy_personas(data_dir, &db);
            }
        }

        // [live2d-only] 默认关闭行为采集（仍保留 foreground/idle 轮询）
        let enabled = crate::db::get_setting(&db, "tracking_enabled")
            .map(|v| v == "1")
            .unwrap_or(false);

        // 重建今日聚合缓存（采集关闭时跳过全表扫描）
        let today = chrono::Local::now().date_naive();
        let aggregator = if enabled {
            crate::db::stats::rebuild_aggregator(&db, today)?
        } else {
            crate::db::stats::TodayAggregator {
                date: Some(today),
                ..Default::default()
            }
        };

        let vault = VaultState::new();
        vault.load_config(&db)?;

        let idle_threshold = crate::db::get_setting(&db, "idle_threshold_secs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(crate::tracker::idle::DEFAULT_IDLE_THRESHOLD_SECS);

        if enabled {
            let _ = crate::db::sessions::open_session(&db);
        }

        let input_stats = InputStatsShared::new();

        let state = Arc::new_cyclic(|weak: &Weak<Self>| Self {
            db: Mutex::new(db),
            aggregator: RwLock::new(aggregator),
            open_segment: Mutex::new(None),
            pending_segment: Mutex::new(None),
            last_snapshot: Mutex::new(None),
            foreground: Mutex::new(None),
            db_path: db_path.clone(),
            tracking_enabled: AtomicBool::new(enabled),
            idle_threshold_secs: AtomicU64::new(idle_threshold),
            stop_flag: AtomicBool::new(false),
            app: app.clone(),
            vault,
            analysis: AnalysisCoordinator::spawn(weak.clone()),
            period_scheduler: PeriodScheduler::spawn(weak.clone()),
            input_stats: input_stats.clone(),
            timeline_describe_lock: tokio::sync::Mutex::new(()),
        });

        Ok(state)
    }

    /// 获取 DB 锁（poison 时尝试恢复）
    pub fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        crate::db::lock_conn(&self.db)
    }

    /// 同步采集开关与会话表
    pub fn set_tracking_enabled(&self, enabled: bool) -> Result<(), String> {
        let was_enabled = self
            .tracking_enabled
            .load(std::sync::atomic::Ordering::Relaxed);
        self.tracking_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
        if was_enabled && !enabled {
            crate::tracker::writer::pause_tracking(self);
        }
        let db = self.lock_db()?;
        crate::db::set_setting(&db, "tracking_enabled", if enabled { "1" } else { "0" })
            .map_err(|e| e.to_string())?;
        if enabled {
            crate::db::sessions::open_session(&db).map_err(|e| e.to_string())?;
        } else {
            crate::db::sessions::close_open_session(&db).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn idle_threshold_secs(&self) -> u64 {
        self.idle_threshold_secs.load(Ordering::Relaxed)
    }

    pub fn set_idle_threshold_secs(&self, secs: u64) {
        self.idle_threshold_secs
            .store(secs.max(1), Ordering::Relaxed);
    }

    pub fn data_dir(&self) -> &std::path::Path {
        self.db_path.parent().unwrap_or(&self.db_path)
    }

    /// 退出前 join 分析/时段 AI 后台线程
    pub fn join_ai_workers(&self) {
        self.analysis.join_worker();
        self.period_scheduler.join_all();
    }

    /// 退出收尾：会话关闭 + WAL checkpoint（segment 由 writer::flush_all_segments 落盘）
    pub fn finalize_shutdown(&self) {
        if let Ok(db) = self.lock_db() {
            let _ = crate::db::sessions::close_open_session(&db);
            let _ = crate::db::usage::close_companion_session(&db);
            let _ = crate::db::usage::close_app_session(&db);
            let _ = db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        self.vault.lock();
        // [live2d-only] 不落库键鼠指标
    }
}

/// 解析 SQLite 路径：与 `data_layout::handaily_data_dir` 一致（便携优先）。
pub fn resolve_db_path(_app: &AppHandle) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = crate::data_layout::handaily_data_dir().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, e)
    })?;
    std::fs::create_dir_all(&dir)?;
    Ok(crate::data_layout::db_path(&dir))
}
