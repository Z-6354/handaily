//! 跨线程共享的应用状态
//!
//! 并发模型（见计划"并发与状态同步模型"）：
//! - `AppState` 包在 `Arc` 里，setup 时 `manage()` + clone 给后台线程
//! - `rusqlite::Connection` 非 Sync，用 `Mutex<Connection>` 串行
//! - command 用 `async fn`，临界区只 clone 快照不持锁做 I/O
//! - 后台线程退出由 `stop_flag: AtomicBool` 控制，Tauri 不会 join 它

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::thread::JoinHandle;

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::db::stats::TodayAggregator;
use crate::analysis::{AnalysisCoordinator, PeriodScheduler};
use crate::tracker::input_monitor::InputStatsShared;
use crate::tracker::{ForegroundPayload, Segment};
use crate::vault::VaultState;

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
        for slot in [&self.poller, &self.input, &self.file, &self.audio] {
            if let Ok(mut guard) = slot.lock() {
                if let Some(handle) = guard.take() {
                    let _ = handle.join();
                }
            }
        }
    }
}

impl AppState {
    /// 初始化：打开 DB、跑 migrate、重建今日聚合缓存、读 settings
    pub fn new(app: &AppHandle) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let db_path = resolve_db_path(app)?;
        let db = crate::db::open_and_migrate(&db_path)?;

        if let Some(data_dir) = db_path.parent() {
            let _ = crate::prompts::seed_user_prompts(data_dir);
            let _ = crate::ai::seed_user_vendors(data_dir);
            let _ = crate::persona::seed_user_personas(data_dir);
            let _ = crate::timeline::json_log::ensure_logs_dir(data_dir);
            if let Err(e) = crate::timeline::json_log::consolidate_past_days_on_startup(data_dir) {
                eprintln!("xiaohan-daily: timeline-ai startup consolidate failed: {e}");
            }
        }

        // 启动兜底：闭合孤儿段与会话
        {
            let now = chrono::Local::now().to_rfc3339();
            crate::db::recover_orphan_segments(&db, &now)?;
            crate::db::sessions::recover_orphan_sessions(&db)?;
            crate::db::usage::recover_orphan_sessions(&db)?;
            let _ = crate::db::usage::open_app_session(&db);
        }

        // 重建今日聚合缓存
        let today = chrono::Local::now().date_naive();
        let aggregator = crate::db::stats::rebuild_aggregator(&db, today)?;

        let vault = VaultState::new();
        vault.load_config(&db)?;

        // 读采集开关（默认开）
        let enabled = crate::db::get_setting(&db, "tracking_enabled")
            .map(|v| v == "1")
            .unwrap_or(true);

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
        self.tracking_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
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

    pub fn data_dir(&self) -> &std::path::Path {
        self.db_path.parent().unwrap_or(&self.db_path)
    }

    /// 退出前 join 分析/时段 AI 后台线程
    pub fn join_ai_workers(&self) {
        self.analysis.join_worker();
        self.period_scheduler.join_all();
    }

    /// 退出前 flush：把 open_segment + pending 写入 DB，WAL checkpoint
    pub fn flush_on_exit(&self) {
        if let Ok(db) = self.lock_db() {
            let _ = self.flush_open(&db);
            let _ = crate::db::sessions::close_open_session(&db);
            let _ = crate::db::usage::close_companion_session(&db);
            let _ = crate::db::usage::close_app_session(&db);
            let _ = db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        self.vault.lock();
        let (mouse, keys, text, created, modified) = self.input_stats.take_flush_delta();
        if let Ok(db) = self.lock_db() {
            let _ = crate::db::metrics::upsert_delta(&db, mouse, keys, &text, created, modified);
        }
    }

    /// 把 open_segment 和 pending_segment 落盘
    fn flush_open(&self, db: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(seg) = self.pending_segment.lock().unwrap().take() {
            crate::db::insert_segment(db, &seg)?;
        }
        if let Some(seg) = self.open_segment.lock().unwrap().take() {
            crate::db::insert_segment(db, &seg)?;
        }
        Ok(())
    }
}

/// 解析 userData 下的 SQLite 路径：%AppData%/xiaohan-daily/data/xiaohan.sqlite
pub fn resolve_db_path(_app: &AppHandle) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let appdata = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|_| _app.path().app_data_dir())?;
    let dir = appdata.join("xiaohan-daily").join("data");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("xiaohan.sqlite"))
}
