//! 桌宠：透明置顶窗口、位置持久化、气泡调度

pub mod lines_import;
pub mod models;
pub mod wiki_scrape;

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::webview::{Color, PageLoadEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use crate::state::AppState;

pub const PET_LABEL: &str = "pet";
pub const PET_MENU_LABEL: &str = "pet-menu";

const DEFAULT_WIDTH: f64 = 240.0;
const DEFAULT_HEIGHT: f64 = 320.0;
const MENU_WIDTH: f64 = 220.0;
const MENU_HEIGHT: f64 = 360.0;

#[cfg(debug_assertions)]
const DEV_PET_PAGE: &str = "http://127.0.0.1:1420/pet.html";

const SCREEN_MARGIN: i32 = 8;

#[derive(Default)]
pub struct PetRuntimeState {
    pub scheduler_running: AtomicBool,
    pub last_context_key: Mutex<String>,
    pub last_remark_at: Mutex<Option<Instant>>,
    pub last_remark_text: Mutex<String>,
    pub fullscreen_suppressed: AtomicBool,
    pub window_create_lock: Mutex<()>,
    pub position_guard_attached: AtomicBool,
    pub position_clamp_suppress: AtomicBool,
    /// pet.html 是否已完成至少一次 Finished 加载
    pub page_load_finished: AtomicBool,
    /// 页面加载完成次数（>1 且窗口可见时才在 on_page_load 补发 reload，避免首启双 reload）
    pub page_load_count: AtomicU32,
    /// 前端 Spine 已成功初始化；隐藏后再显示时走 resume 而非全量 reload
    pub spine_ready: AtomicBool,
    /// 与 spine_ready 对应、已通过前端校验的 model_id
    pub spine_ready_model_id: Mutex<String>,
    /// 菜单切换模型：等待桌宠前端 mark_spine_ready 的一次性确认（std mpsc，避免 block_on 饿死 runtime）
    pub switch_confirm_tx: Mutex<Option<mpsc::SyncSender<String>>>,
    /// 菜单切换模型：目标 model_id（规范 id）
    pub switch_target_model_id: Mutex<Option<String>>,
    /// 菜单切换模型：当前等待的 request id
    pub switch_request_id: Mutex<Option<u64>>,
    pub switch_seq: AtomicU64,
    /// 正在后台爬取 Wiki 台词的 model_id，避免重复请求
    pub wiki_lines_import_inflight: Mutex<HashSet<String>>,
    /// 启动批量 Wiki 台词导入是否进行中
    pub wiki_bulk_import_running: AtomicBool,
    /// 最近一次批量导入进度（供前端晚挂载时恢复）
    pub wiki_bulk_last_progress: Mutex<Option<lines_import::PetWikiBulkImportProgress>>,
    /// 用户请求停止批量 Wiki 导入
    pub wiki_bulk_stop_requested: AtomicBool,
    /// 批量 Wiki 导入是否暂停
    pub wiki_bulk_paused: AtomicBool,
    /// pet-menu.html 是否已完成至少一次 Finished 加载
    pub menu_page_load_finished: AtomicBool,
    /// 菜单页未就绪时暂存的显示坐标（物理像素）
    pub menu_pending_show: Mutex<Option<(i32, i32)>>,
    /// 是否已有 pending menu show 轮询任务在跑
    pub menu_pending_spawn_running: AtomicBool,
    /// 用户已关闭菜单；阻止迟到的 show 把菜单又弹出来
    pub menu_suppress_show: AtomicBool,
    /// 用户通过菜单/托盘主动隐藏桌宠（非全屏抑制）；阻止台词等逻辑自动 show
    pub user_hidden_pet: AtomicBool,
}

#[derive(Clone, Serialize)]
pub struct PetScreenBounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Serialize)]
pub struct PetPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Serialize)]
pub struct PetRemarkPayload {
    pub text: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct PetStatusPayload {
    pub enabled: bool,
    pub visible: bool,
    /// 设置页「启用桌宠」：DB 已启用且非用户主动隐藏（全屏临时隐藏仍为 true）。
    pub active: bool,
    pub power_mode: String,
    pub scale: f64,
    pub remark_interval_sec: i64,
    pub bubble_enabled: bool,
    pub model_id: String,
    pub model_name: String,
    pub animations: Vec<String>,
    pub idle_animation: Option<String>,
    pub click_animation: Option<String>,
    pub boot_animation: Option<String>,
    pub return_idle_animation: Option<String>,
    pub drag_animation: Option<String>,
    pub random_animations: Vec<String>,
    pub random_min_sec: i64,
    pub random_max_sec: i64,
    pub lines: Vec<models::PetRemarkLine>,
}

#[derive(Clone, Serialize)]
pub struct PetStatusChangedPayload {
    /// 设置页「启用桌宠」开关：DB 已启用且非用户主动隐藏。
    pub active: bool,
    pub bubble_enabled: bool,
}

#[derive(Clone, Serialize)]
pub struct PetMenuDismissPoll {
    pub left_down: bool,
    pub menu_contains_cursor: bool,
}

#[derive(Clone, Serialize)]
pub struct PetConfigPayload {
    pub model_id: String,
    pub model_name: String,
    pub asset_base: String,
    pub config_file: Option<String>,
    pub skel_file: String,
    pub atlas_file: String,
    pub png_file: String,
    pub use_file_src: bool,
    pub power_mode: String,
    pub scale: f64,
    pub animations: Vec<String>,
    pub idle_animation: Option<String>,
    pub click_animation: Option<String>,
    pub boot_animation: Option<String>,
    pub return_idle_animation: Option<String>,
    pub drag_animation: Option<String>,
    pub random_animations: Vec<String>,
    pub random_min_sec: i64,
    pub random_max_sec: i64,
    pub lines: Vec<models::PetRemarkLine>,
    pub window_width: f64,
    pub window_height: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub bubble_enabled: bool,
}

#[derive(Clone, Serialize)]
pub struct PetSwitchPayload {
    pub switch_id: u64,
    pub config: PetConfigPayload,
}

pub fn is_enabled(db: &rusqlite::Connection) -> bool {
    // 默认开启；仅当用户显式关闭（pet_enabled=0）时不显示
    crate::db::get_setting(db, "pet_enabled").as_deref() != Some("0")
}

pub fn get_scale(db: &rusqlite::Connection) -> f64 {
    let v: f64 = crate::db::get_setting(db, "pet_scale")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.8);
    v.clamp(0.4, 1.5)
}

pub fn get_remark_interval_sec(db: &rusqlite::Connection) -> i64 {
    crate::db::get_setting(db, "pet_remark_interval_sec")
        .and_then(|s| s.parse().ok())
        .unwrap_or(300)
        .clamp(0, 3600)
}

pub fn is_bubble_enabled(db: &rusqlite::Connection) -> bool {
    crate::db::get_setting(db, "pet_bubble_enabled").as_deref() != Some("0")
}

pub fn set_bubble_enabled(db: &rusqlite::Connection, enabled: bool) -> Result<(), String> {
    crate::db::set_setting(
        db,
        "pet_bubble_enabled",
        if enabled { "1" } else { "0" },
    )
    .map_err(|e| e.to_string())
}

fn model_power_mode(_db: &rusqlite::Connection, _meta: &models::PetAnimationMeta) -> String {
    "balanced".to_string()
}

fn model_scale(db: &rusqlite::Connection, _meta: &models::PetAnimationMeta) -> f64 {
    get_scale(db)
}

pub fn set_scale(db: &rusqlite::Connection, scale: f64) -> Result<(), String> {
    let s = scale.clamp(0.4, 1.5);
    crate::db::set_setting(db, "pet_scale", &s.to_string()).map_err(|e| e.to_string())
}

fn model_remark_interval_sec(db: &rusqlite::Connection, meta: &models::PetAnimationMeta) -> i64 {
    meta.remark_interval_sec
        .map(|s| s.clamp(0, 3600))
        .unwrap_or_else(|| get_remark_interval_sec(db))
}

pub fn get_window_size(db: &rusqlite::Connection) -> (f64, f64) {
    let w: f64 = crate::db::get_setting(db, "pet_width")
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_WIDTH);
    let h: f64 = crate::db::get_setting(db, "pet_height")
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_HEIGHT);
    (w.clamp(160.0, 480.0), h.clamp(200.0, 600.0))
}

pub fn get_model_offset(db: &rusqlite::Connection) -> (f64, f64) {
    let x: f64 = crate::db::get_setting(db, "pet_offset_x")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let y: f64 = crate::db::get_setting(db, "pet_offset_y")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    (x, y)
}

pub fn save_window_size(db: &rusqlite::Connection, width: f64, height: f64) -> Result<(), String> {
    let (w, h) = (width.clamp(160.0, 480.0), height.clamp(200.0, 600.0));
    crate::db::set_setting(db, "pet_width", &w.to_string()).map_err(|e| e.to_string())?;
    crate::db::set_setting(db, "pet_height", &h.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save_layout(
    db: &rusqlite::Connection,
    width: f64,
    height: f64,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
    position_win_w: Option<i32>,
    position_win_h: Option<i32>,
) -> Result<(), String> {
    save_window_size(db, width, height)?;
    let s = scale.clamp(0.4, 1.5);
    crate::db::set_setting(db, "pet_scale", &s.to_string()).map_err(|e| e.to_string())?;
    crate::db::set_setting(db, "pet_offset_x", &offset_x.round().to_string())
        .map_err(|e| e.to_string())?;
    crate::db::set_setting(db, "pet_offset_y", &offset_y.round().to_string())
        .map_err(|e| e.to_string())?;
    let pos = load_position(db);
    save_position(db, pos.0, pos.1, position_win_w, position_win_h)?;
    Ok(())
}

fn pet_webview_url() -> WebviewUrl {
    // 必须 App 协议：External(localhost) 为 remote origin，pet-capability 仅授权 local，
    // invoke 被拒 → 透明空窗（见 docs/questions/62、67）
    WebviewUrl::App("pet.html".into())
}

fn pet_menu_webview_url() -> WebviewUrl {
    WebviewUrl::App("pet-menu.html".into())
}

fn clamp_menu_position(x: i32, y: i32) -> (i32, i32) {
    clamp_pet_position(x, y, MENU_WIDTH as i32, MENU_HEIGHT as i32)
}

fn prepare_menu_webview(win: &WebviewWindow) -> Result<(), String> {
    let _ = win.set_background_color(Some(Color(0, 0, 0, 0)));
    let _ = win.set_always_on_top(true);
    Ok(())
}

fn is_pet_menu_visible(app: &AppHandle) -> bool {
    app.get_webview_window(PET_MENU_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// 将菜单窗置于桌宠窗之上（Windows 下两者均为 TOPMOST 时后设的在上层）
fn raise_menu_above_pet(app: &AppHandle, menu: &WebviewWindow) {
    let pet = app.get_webview_window(PET_LABEL);
    if let Some(pet) = &pet {
        if pet.is_visible().unwrap_or(false) {
            let _ = pet.set_always_on_top(true);
        }
    }
    let _ = menu.set_always_on_top(true);
    raise_menu_hwnd_above_pet(menu, pet.as_ref());
}

pub fn sync_menu_z_order_if_visible(app: &AppHandle) {
    if !is_pet_menu_visible(app) {
        return;
    }
    if let Some(menu) = app.get_webview_window(PET_MENU_LABEL) {
        raise_menu_above_pet(app, &menu);
    }
}

#[cfg(windows)]
fn raise_menu_hwnd_above_pet(menu: &WebviewWindow, pet: Option<&WebviewWindow>) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    let Ok(menu_hwnd) = menu.hwnd() else {
        return;
    };
    unsafe {
        if let Some(pet) = pet.filter(|p| p.is_visible().unwrap_or(false)) {
            if let Ok(pet_hwnd) = pet.hwnd() {
                let _ = SetWindowPos(
                    pet_hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
        let _ = SetWindowPos(
            menu_hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(not(windows))]
fn raise_menu_hwnd_above_pet(_menu: &WebviewWindow, _pet: Option<&WebviewWindow>) {}

fn point_in_rect(cx: i32, cy: i32, rx: i32, ry: i32, rw: i32, rh: i32) -> bool {
    cx >= rx && cx < rx + rw && cy >= ry && cy < ry + rh
}

#[cfg(windows)]
fn read_window_frame_rect(win: &WebviewWindow) -> Result<(i32, i32, i32, i32), String> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let frame = pet_frame_hwnd(win)?;
    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(frame, &mut rect).map_err(|e| e.to_string())?;
    }
    Ok((
        rect.left,
        rect.top,
        (rect.right - rect.left).max(1),
        (rect.bottom - rect.top).max(1),
    ))
}

#[cfg(not(windows))]
fn read_window_frame_rect(win: &WebviewWindow) -> Result<(i32, i32, i32, i32), String> {
    use tauri::{PhysicalPosition, PhysicalSize};
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let size = win.outer_size().map_err(|e| e.to_string())?;
    Ok((
        pos.x,
        pos.y,
        size.width as i32,
        size.height as i32,
    ))
}

fn is_cursor_over_webview(win: &WebviewWindow, cursor_x: i32, cursor_y: i32) -> bool {
    if !win.is_visible().unwrap_or(false) {
        return false;
    }
    let Ok((x, y, w, h)) = read_window_frame_rect(win) else {
        return false;
    };
    point_in_rect(cursor_x, cursor_y, x, y, w, h)
}

fn is_cursor_over_pet_window(app: &AppHandle) -> bool {
    let Ok(cursor) = app.cursor_position() else {
        return false;
    };
    let Some(pet) = app.get_webview_window(PET_LABEL) else {
        return false;
    };
    is_cursor_over_webview(
        &pet,
        cursor.x.round() as i32,
        cursor.y.round() as i32,
    )
}

fn spawn_pending_menu_show(app: &AppHandle) {
    let Some(rt) = app.try_state::<PetRuntimeState>() else {
        return;
    };
    if rt
        .menu_pending_spawn_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        struct ResetGuard {
            app: AppHandle,
        }
        impl Drop for ResetGuard {
            fn drop(&mut self) {
                if let Some(rt) = self.app.try_state::<PetRuntimeState>() {
                    rt.menu_pending_spawn_running
                        .store(false, Ordering::Release);
                }
            }
        }
        let _guard = ResetGuard { app: app.clone() };

        for _ in 0..60 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let Some(rt) = app.try_state::<PetRuntimeState>() else {
                continue;
            };
            if !rt.menu_page_load_finished.load(Ordering::Acquire) {
                continue;
            }
            let pending = rt
                .menu_pending_show
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if let Some((x, y)) = pending {
                let _ = show_pet_menu_immediate(&app, x, y);
            }
            return;
        }
        crate::log::warn("桌宠菜单页加载超时，仍尝试显示");
        if let Some(rt) = app.try_state::<PetRuntimeState>() {
            rt.menu_page_load_finished
                .store(true, Ordering::Release);
            let pending = rt
                .menu_pending_show
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if let Some((x, y)) = pending {
                let _ = show_pet_menu_immediate(&app, x, y);
            }
        }
    });
}

pub fn ensure_pet_menu_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(PET_MENU_LABEL).is_some() {
        return Ok(());
    }
    let app_for_load = app.clone();
    WebviewWindowBuilder::new(app, PET_MENU_LABEL, pet_menu_webview_url())
        .title("小寒桌宠菜单")
        .inner_size(MENU_WIDTH, MENU_HEIGHT)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .on_page_load(move |_window, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            if let Some(rt) = app_for_load.try_state::<PetRuntimeState>() {
                rt.menu_page_load_finished
                    .store(true, Ordering::Release);
                let pending = rt
                    .menu_pending_show
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                if let Some((x, y)) = pending {
                    let _ = show_pet_menu_immediate(&app_for_load, x, y);
                }
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    if let Some(win) = app.get_webview_window(PET_MENU_LABEL) {
        let _ = prepare_menu_webview(&win);
    }
    Ok(())
}

fn emit_pet_menu_state(app: &AppHandle, open: bool) {
    let _ = app.emit_to(PET_LABEL, "pet-menu-state", open);
}

fn reset_menu_runtime(rt: &PetRuntimeState) {
    rt.menu_page_load_finished.store(false, Ordering::Release);
    rt.menu_pending_spawn_running
        .store(false, Ordering::Release);
    suppress_menu_show(rt);
}

fn suppress_menu_show(rt: &PetRuntimeState) {
    rt.menu_suppress_show.store(true, Ordering::Release);
    *rt.menu_pending_show
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
}

fn user_hidden_pet(app: &AppHandle) -> bool {
    app.try_state::<PetRuntimeState>()
        .map(|rt| rt.user_hidden_pet.load(Ordering::Acquire))
        .unwrap_or(false)
}

fn pet_active_flag(app: &AppHandle, db: &rusqlite::Connection) -> bool {
    is_enabled(db) && !user_hidden_pet(app)
}

fn pet_status_changed_payload(app: &AppHandle) -> PetStatusChangedPayload {
    app.try_state::<Arc<AppState>>()
        .and_then(|st| {
            st.db.lock().ok().map(|db| PetStatusChangedPayload {
                active: pet_active_flag(app, &db),
                bubble_enabled: is_bubble_enabled(&db),
            })
        })
        .unwrap_or(PetStatusChangedPayload {
            active: false,
            bubble_enabled: true,
        })
}

fn sync_pet_visibility_ui(app: &AppHandle) {
    crate::tray::sync_pet_toggle_label(app);
    emit_pet_status_changed(app);
}

/// 推送轻量状态供设置页增量同步（payload 极小，不门控主窗口可见性）。
pub fn emit_pet_status_changed(app: &AppHandle) {
    if crate::APP_EXITING.load(Ordering::Relaxed) {
        return;
    }
    let _ = app.emit_to(
        "main",
        "pet-status-changed",
        pet_status_changed_payload(app),
    );
}

pub fn poll_menu_dismiss(app: &AppHandle) -> PetMenuDismissPoll {
    PetMenuDismissPoll {
        left_down: is_left_mouse_down(),
        menu_contains_cursor: is_cursor_over_menu_window(app),
    }
}

pub fn hide_pet_menu(app: &AppHandle) -> Result<(), String> {
    if crate::APP_EXITING.load(Ordering::Relaxed) {
        return Ok(());
    }
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        suppress_menu_show(&rt);
    }
    if let Some(win) = app.get_webview_window(PET_MENU_LABEL) {
        let _ = win.hide();
    }
    emit_pet_menu_state(app, false);
    sync_pet_interaction_state(app);
    Ok(())
}

fn detach_menu_window_effects(win: &WebviewWindow) {
    let _ = win.set_always_on_top(false);
    let _ = win.hide();
}

fn destroy_pet_menu_window(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        reset_menu_runtime(&rt);
    }
    if is_pet_menu_visible(app) || pet_menu_open_or_pending(app) {
        emit_pet_menu_state(app, false);
    }
    if let Some(win) = app.get_webview_window(PET_MENU_LABEL) {
        detach_menu_window_effects(&win);
        let _ = win.close();
    }
}

#[derive(Clone, Serialize)]
pub struct PetMenuShownPayload {
    pub suppress_blur_ms: u64,
}

fn show_pet_menu_immediate(app: &AppHandle, screen_x: i32, screen_y: i32) -> Result<(), String> {
    if app
        .try_state::<PetRuntimeState>()
        .map(|rt| rt.menu_suppress_show.load(Ordering::Acquire))
        .unwrap_or(false)
    {
        return Ok(());
    }
    let win = app
        .get_webview_window(PET_MENU_LABEL)
        .ok_or_else(|| "菜单窗口不存在".to_string())?;
    let _ = prepare_menu_webview(&win);
    if let Some(pet) = app.get_webview_window(PET_LABEL) {
        let _ = pet.set_ignore_cursor_events(false);
    }
    let (x, y) = clamp_menu_position(screen_x, screen_y);
    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    win.show().map_err(|e| e.to_string())?;
    raise_menu_above_pet(app, &win);
    let _ = app.emit_to(
        PET_MENU_LABEL,
        "pet-menu-shown",
        PetMenuShownPayload {
            suppress_blur_ms: 1200,
        },
    );
    emit_pet_menu_state(app, true);
    Ok(())
}

pub fn show_pet_menu(app: &AppHandle, screen_x: i32, screen_y: i32) -> Result<(), String> {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.menu_suppress_show.store(false, Ordering::Release);
    }
    ensure_pet_menu_window(app)?;
    let ready = app
        .try_state::<PetRuntimeState>()
        .map(|rt| rt.menu_page_load_finished.load(Ordering::Acquire))
        .unwrap_or(false);
    if !ready {
        if let Some(rt) = app.try_state::<PetRuntimeState>() {
            *rt.menu_pending_show
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some((screen_x, screen_y));
        }
        spawn_pending_menu_show(app);
        return Ok(());
    }
    show_pet_menu_immediate(app, screen_x, screen_y)
}

fn pet_menu_open_or_pending(app: &AppHandle) -> bool {
    if is_pet_menu_visible(app) {
        return true;
    }
    app.try_state::<PetRuntimeState>()
        .map(|rt| {
            rt.menu_pending_show
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_some()
        })
        .unwrap_or(false)
}

pub fn toggle_pet_menu(app: &AppHandle, screen_x: i32, screen_y: i32) -> Result<(), String> {
    if pet_menu_open_or_pending(app) {
        hide_pet_menu(app)
    } else {
        show_pet_menu(app, screen_x, screen_y)
    }
}

pub fn toggle_pet_menu_at_cursor(app: &AppHandle) -> Result<bool, String> {
    if !is_cursor_over_pet_window(app) {
        return Ok(pet_menu_open_or_pending(app));
    }
    if pet_menu_open_or_pending(app) {
        hide_pet_menu(app)?;
        return Ok(false);
    }
    let pos = app.cursor_position().map_err(|e| e.to_string())?;
    show_pet_menu(app, pos.x.round() as i32, pos.y.round() as i32)?;
    Ok(pet_menu_open_or_pending(app))
}

pub fn show_pet_menu_at_cursor(app: &AppHandle) -> Result<(), String> {
    toggle_pet_menu_at_cursor(app).map(|_| ())
}

#[cfg(windows)]
pub fn is_right_mouse_down() -> bool {
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        GetAsyncKeyState(0x02) as u16 & 0x8000 != 0
    }
}

#[cfg(not(windows))]
pub fn is_right_mouse_down() -> bool {
    false
}

#[cfg(windows)]
pub fn is_left_mouse_down() -> bool {
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        GetAsyncKeyState(0x01) as u16 & 0x8000 != 0
    }
}

#[cfg(not(windows))]
pub fn is_left_mouse_down() -> bool {
    false
}

pub fn is_cursor_over_menu_window(app: &AppHandle) -> bool {
    let Ok(cursor) = app.cursor_position() else {
        return false;
    };
    let Some(menu) = app.get_webview_window(PET_MENU_LABEL) else {
        return false;
    };
    is_cursor_over_webview(
        &menu,
        cursor.x.round() as i32,
        cursor.y.round() as i32,
    )
}

pub fn enter_pet_edit_bounds(app: &AppHandle) -> Result<(), String> {
    if crate::APP_EXITING.load(Ordering::Relaxed) {
        return Ok(());
    }
    // 先发事件让桌宠窗进入编辑模式（禁用穿透），再关菜单，避免 pet-menu-state 抢先 startClickThrough
    let _ = app.emit_to(PET_LABEL, "pet-enter-edit-bounds", ());
    hide_pet_menu(app)?;
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        let _ = win.set_ignore_cursor_events(false);
        let _ = win.show();
        let _ = win.set_always_on_top(true);
        let _ = win.set_focus();
    }
    Ok(())
}

#[derive(Clone, Serialize)]
pub struct PetWindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[cfg(windows)]
fn pet_frame_hwnd(win: &WebviewWindow) -> Result<windows::Win32::Foundation::HWND, String> {
    use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GA_ROOT};
    let hwnd = win.hwnd().map_err(|e| e.to_string())?;
    let frame = unsafe {
        let root = GetAncestor(hwnd, GA_ROOT);
        if root.0.is_null() {
            hwnd
        } else {
            root
        }
    };
    Ok(frame)
}

fn read_pet_window_bounds_from_win(win: &WebviewWindow) -> Result<PetWindowBounds, String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let frame = pet_frame_hwnd(win)?;
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(frame, &mut rect).map_err(|e| e.to_string())?;
        }
        let width = (rect.right - rect.left).max(1) as u32;
        let height = (rect.bottom - rect.top).max(1) as u32;
        Ok(PetWindowBounds {
            x: rect.left,
            y: rect.top,
            width,
            height,
        })
    }
    #[cfg(not(windows))]
    {
        use tauri::{PhysicalPosition, PhysicalSize};
        let size = win.outer_size().map_err(|e| e.to_string())?;
        let pos = win.outer_position().map_err(|e| e.to_string())?;
        Ok(PetWindowBounds {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        })
    }
}

pub fn get_pet_window_bounds(app: &AppHandle) -> Result<PetWindowBounds, String> {
    let win = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    read_pet_window_bounds_from_win(&win)
}

pub fn set_pet_window_bounds(
    app: &AppHandle,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    move_x: bool,
    move_y: bool,
) -> Result<(), String> {
    let win = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.position_clamp_suppress
            .store(true, Ordering::Release);
    }
    let result = set_pet_window_bounds_on_win(&win, x, y, width, height, move_x, move_y);
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.position_clamp_suppress
            .store(false, Ordering::Release);
    }
    result
}

fn set_pet_window_bounds_on_win(
    win: &WebviewWindow,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    move_x: bool,
    move_y: bool,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};
        let frame = pet_frame_hwnd(win)?;
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(frame, &mut rect).map_err(|e| e.to_string())?;
        }
        let final_x = if move_x { x } else { rect.left };
        let final_y = if move_y { y } else { rect.top };
        unsafe {
            SetWindowPos(
                frame,
                None,
                final_x,
                final_y,
                width as i32,
                height as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .map_err(|e| e.to_string())?;
        }
    }
    #[cfg(not(windows))]
    {
        use tauri::{PhysicalPosition, PhysicalSize, Size};
        if move_x || move_y {
            let pos = win.outer_position().map_err(|e| e.to_string())?;
            win.set_position(PhysicalPosition::new(
                if move_x { x } else { pos.x },
                if move_y { y } else { pos.y },
            ))
            .map_err(|e| e.to_string())?;
        }
        win.set_size(Size::Physical(PhysicalSize::new(width, height)))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(debug_assertions)]
async fn wait_frontend_ready(max_secs: u64) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(400))
        .build()
        .ok();
    let deadline = Instant::now() + Duration::from_secs(max_secs);
    while Instant::now() < deadline {
        let ok = tokio::net::TcpStream::connect("127.0.0.1:1420")
            .await
            .is_ok()
            || if let Some(c) = &client {
                c.get(DEV_PET_PAGE)
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false)
            } else {
                false
            };
        if ok {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    crate::log::info("桌宠: 等待 Vite dev server 超时，仍将尝试显示");
}

fn prepare_pet_webview(win: &tauri::WebviewWindow) -> Result<(), String> {
    let _ = win.set_background_color(Some(Color(0, 0, 0, 0)));
    let _ = win.set_always_on_top(true);
    // 默认穿透，待前端按角色区域再关闭穿透（避免 WebView 就绪前透明窗挡桌面）
    let _ = win.set_ignore_cursor_events(true);
    Ok(())
}

pub fn pet_visible(app: &AppHandle) -> bool {
    app.get_webview_window(PET_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// 主窗口是否仍在前台展示（可见即视为打开，失焦不算关闭）。
pub fn is_main_window_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// 广播主窗可见性（主窗 WebView + 全应用），桌宠端依赖此事件更新 mainWindowVisible。
pub fn emit_main_window_visible(app: &AppHandle, visible: bool) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("main-window-visible", visible);
    }
    let _ = app.emit("main-window-visible", visible);
}

/// 显示主窗口；桌宠 `always_on_top` 会挡住主窗口，需先让桌宠降层。
pub fn show_main_window(app: &AppHandle, page: Option<&str>) -> Result<(), String> {
    let win = crate::ensure_main_window(app)?;
    if let Some(pet) = app.get_webview_window(PET_LABEL) {
        let _ = pet.set_always_on_top(false);
        let _ = pet.set_ignore_cursor_events(false);
    }
    let _ = win.unminimize();
    win.show().map_err(|e| e.to_string())?;
    // 主窗 show 成功后再通知桌宠，避免 show 失败时桌宠长期锁在 overlay 态
    let _ = app.emit_to(PET_LABEL, "pet-main-opening", 1200u64);
    if let Some(pet) = app.get_webview_window(PET_LABEL) {
        let _ = pet.set_always_on_top(false);
        let _ = pet.set_ignore_cursor_events(false);
    }
    let _ = win.set_focus();
    emit_main_window_visible(app, true);
    let _ = hide_pet_menu(app);
    sync_pet_interaction_state(app);
    if let Some(page) = page.filter(|p| !p.is_empty()) {
        let _ = app.emit_to("main", "main-navigate", page.to_string());
    }
    Ok(())
}

/// 主窗口失焦/隐藏后恢复桌宠置顶（若仍可见）；菜单打开时保持菜单在桌宠之上。
pub fn restore_pet_topmost_if_visible(app: &AppHandle) {
    if is_main_window_visible(app) {
        return;
    }
    if app
        .try_state::<PetRuntimeState>()
        .map(|rt| rt.user_hidden_pet.load(Ordering::Acquire))
        .unwrap_or(false)
    {
        return;
    }
    if is_pet_menu_visible(app) {
        if let Some(menu) = app.get_webview_window(PET_MENU_LABEL) {
            raise_menu_above_pet(app, &menu);
        }
        return;
    }
    let _ = app.emit_to(PET_LABEL, "pet-main-closed", ());
    sync_pet_interaction_state(app);
}

/// 主窗开关 / 菜单关闭后，强制同步桌宠穿透、置顶与前端菜单状态位。
pub fn sync_pet_interaction_state(app: &AppHandle) {
    if crate::APP_EXITING.load(Ordering::Relaxed) {
        return;
    }
    if is_main_window_visible(app) {
        if let Some(pet) = app.get_webview_window(PET_LABEL) {
            let _ = pet.set_always_on_top(false);
            let _ = pet.set_ignore_cursor_events(false);
        }
        if is_pet_menu_visible(app) || pet_menu_open_or_pending(app) {
            let _ = app.emit_to(PET_LABEL, "pet-menu-state", false);
        }
        let _ = app.emit_to(PET_LABEL, "pet-sync-click-through", ());
        return;
    }
    if is_pet_menu_visible(app) || pet_menu_open_or_pending(app) {
        if let Some(pet) = app.get_webview_window(PET_LABEL) {
            let _ = pet.set_ignore_cursor_events(false);
        }
        let _ = app.emit_to(PET_LABEL, "pet-sync-click-through", ());
        return;
    }
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        let pending = rt
            .menu_pending_show
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if pending.is_some() {
            suppress_menu_show(&rt);
            let _ = app.emit_to(PET_LABEL, "pet-menu-state", false);
        }
    }
    if let Some(pet) = app.get_webview_window(PET_LABEL) {
        if pet.is_visible().unwrap_or(false) {
            let _ = pet.set_ignore_cursor_events(false);
            if !app
                .try_state::<PetRuntimeState>()
                .map(|rt| rt.user_hidden_pet.load(Ordering::Acquire))
                .unwrap_or(false)
            {
                let _ = pet.set_always_on_top(true);
            }
        }
    }
    let menu_open = is_pet_menu_visible(app) || pet_menu_open_or_pending(app);
    let _ = app.emit_to(PET_LABEL, "pet-menu-state", menu_open);
    let _ = app.emit_to(PET_LABEL, "pet-sync-click-through", ());
}

/// 自动化测试：桌宠交互层状态（Rust 侧）
#[derive(Clone, Serialize)]
pub struct PetInteractionState {
    pub main_window_visible: bool,
    pub pet_visible: bool,
    pub menu_visible: bool,
    pub menu_open_or_pending: bool,
    pub bubble_enabled: bool,
    pub cursor_over_pet: bool,
    pub cursor_over_menu: bool,
}

pub fn interaction_state(app: &AppHandle, st: &AppState) -> Result<PetInteractionState, String> {
    let bubble_enabled = {
        let db = crate::db::lock_conn(&st.db)?;
        is_bubble_enabled(&db)
    };
    Ok(PetInteractionState {
        main_window_visible: is_main_window_visible(app),
        pet_visible: pet_visible(app),
        menu_visible: is_pet_menu_visible(app),
        menu_open_or_pending: pet_menu_open_or_pending(app),
        bubble_enabled,
        cursor_over_pet: is_cursor_over_pet_window(app),
        cursor_over_menu: is_cursor_over_menu_window(app),
    })
}

pub fn emit_pet_test_action(app: &AppHandle, action: &str) -> Result<(), String> {
    let _ = app.emit_to(PET_LABEL, "pet-test-action", action.to_string());
    Ok(())
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    let Some(win) = app.get_webview_window("main") else {
        return Ok(());
    };
    let _ = win.hide();
    emit_main_window_visible(app, false);
    restore_pet_topmost_if_visible(app);
    Ok(())
}

pub fn model_status(
    app: &AppHandle,
    st: &AppState,
    model_id: &str,
) -> Result<PetStatusPayload, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let anim_meta = models::read_animation_meta(st.data_dir(), &db, model_id);
    let model_name = models::resolve_assets(st.data_dir(), model_id)
        .map(|a| a.model_name)
        .unwrap_or_else(|_| model_id.to_string());
    Ok(PetStatusPayload {
        enabled: is_enabled(&db),
        visible: false,
        active: pet_active_flag(app, &db),
        power_mode: model_power_mode(&db, &anim_meta),
        scale: model_scale(&db, &anim_meta),
        remark_interval_sec: model_remark_interval_sec(&db, &anim_meta),
        bubble_enabled: is_bubble_enabled(&db),
        model_id: model_id.to_string(),
        model_name,
        animations: anim_meta.animations,
        idle_animation: anim_meta.idle_animation,
        click_animation: anim_meta.click_animation,
        boot_animation: anim_meta.boot_animation,
        return_idle_animation: anim_meta.return_idle_animation,
        drag_animation: anim_meta.drag_animation,
        random_animations: anim_meta.random_animations,
        random_min_sec: anim_meta.random_min_sec,
        random_max_sec: anim_meta.random_max_sec,
        lines: anim_meta.lines,
    })
}

pub fn status(app: &AppHandle, st: &AppState) -> Result<PetStatusPayload, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let model_id = models::active_model_id(&db);
    let model_name = models::resolve_assets(st.data_dir(), &model_id)
        .map(|a| a.model_name)
        .unwrap_or_else(|_| model_id.clone());
    let anim_meta = models::read_animation_meta(st.data_dir(), &db, &model_id);
    Ok(PetStatusPayload {
        enabled: is_enabled(&db),
        visible: pet_visible(app),
        active: pet_active_flag(app, &db),
        power_mode: model_power_mode(&db, &anim_meta),
        scale: model_scale(&db, &anim_meta),
        remark_interval_sec: model_remark_interval_sec(&db, &anim_meta),
        bubble_enabled: is_bubble_enabled(&db),
        model_id,
        model_name,
        animations: anim_meta.animations,
        idle_animation: anim_meta.idle_animation,
        click_animation: anim_meta.click_animation,
        boot_animation: anim_meta.boot_animation,
        return_idle_animation: anim_meta.return_idle_animation,
        drag_animation: anim_meta.drag_animation,
        random_animations: anim_meta.random_animations,
        random_min_sec: anim_meta.random_min_sec,
        random_max_sec: anim_meta.random_max_sec,
        lines: anim_meta.lines,
    })
}

/// Debug / 自动化测试：桌宠运行时快照（含 spine 与菜单切换等待状态）
#[derive(Clone, Serialize)]
pub struct PetTestSnapshot {
    pub status: PetStatusPayload,
    pub spine_ready: bool,
    pub spine_ready_model_id: String,
    pub switch_wait_target: Option<String>,
    pub switch_request_id: Option<u64>,
}

pub fn test_snapshot(app: &AppHandle, st: &AppState) -> Result<PetTestSnapshot, String> {
    let status = status(app, st)?;
    test_snapshot_from_status(app, status)
}

/// 轻量快照：不访问 DB / 台词，供测试 API 切换后快速返回。
pub fn test_snapshot_light(app: &AppHandle, model_id: &str) -> Result<PetTestSnapshot, String> {
    let status = PetStatusPayload {
        enabled: true,
        visible: pet_visible(app),
        active: true,
        power_mode: "balanced".into(),
        scale: 0.55,
        remark_interval_sec: 300,
        bubble_enabled: false,
        model_id: model_id.to_string(),
        model_name: model_id.to_string(),
        animations: vec![],
        idle_animation: Some("normal".into()),
        click_animation: Some("touch".into()),
        boot_animation: Some("normal".into()),
        return_idle_animation: Some("normal".into()),
        drag_animation: Some("tuozhuai".into()),
        random_animations: vec![],
        random_min_sec: 30,
        random_max_sec: 120,
        lines: vec![],
    };
    test_snapshot_from_status(app, status)
}

fn test_snapshot_from_status(app: &AppHandle, status: PetStatusPayload) -> Result<PetTestSnapshot, String> {
    let (spine_ready, spine_ready_model_id, switch_wait_target, switch_request_id) = app
        .try_state::<PetRuntimeState>()
        .map(|rt| {
            (
                rt.spine_ready.load(Ordering::Acquire),
                rt.spine_ready_model_id
                    .lock()
                    .ok()
                    .map(|g| g.clone())
                    .unwrap_or_default(),
                rt.switch_target_model_id
                    .lock()
                    .ok()
                    .and_then(|g| g.clone()),
                rt.switch_request_id.lock().ok().and_then(|g| *g),
            )
        })
        .unwrap_or((false, String::new(), None, None));
    Ok(PetTestSnapshot {
        status,
        spine_ready,
        spine_ready_model_id,
        switch_wait_target,
        switch_request_id,
    })
}

pub fn get_config(st: &AppState) -> Result<PetConfigPayload, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let model_id = models::active_model_id(&db);
    get_config_for_model(st.data_dir(), &db, &model_id)
}

/// 解析任意模型的资源信息（供空闲预加载，不读台词元数据）
pub fn get_config_for_model(
    data_dir: &std::path::Path,
    db: &rusqlite::Connection,
    model_id: &str,
) -> Result<PetConfigPayload, String> {
    let assets = models::resolve_assets(data_dir, model_id)?;
    let anim_meta = models::read_animation_meta(data_dir, db, &assets.model_id);
    let (window_width, window_height) = get_window_size(db);
    let (offset_x, offset_y) = get_model_offset(db);
    Ok(PetConfigPayload {
        model_id: assets.model_id,
        model_name: assets.model_name,
        asset_base: assets.asset_base,
        config_file: assets.config_file,
        skel_file: assets.skel_file,
        atlas_file: assets.atlas_file,
        png_file: assets.png_file,
        use_file_src: assets.use_file_src,
        power_mode: model_power_mode(db, &anim_meta),
        scale: model_scale(db, &anim_meta),
        animations: anim_meta.animations,
        idle_animation: anim_meta.idle_animation,
        click_animation: anim_meta.click_animation,
        boot_animation: anim_meta.boot_animation,
        return_idle_animation: anim_meta.return_idle_animation,
        drag_animation: anim_meta.drag_animation,
        random_animations: anim_meta.random_animations,
        random_min_sec: anim_meta.random_min_sec,
        random_max_sec: anim_meta.random_max_sec,
        lines: anim_meta.lines,
        window_width,
        window_height,
        offset_x,
        offset_y,
        bubble_enabled: is_bubble_enabled(db),
    })
}

/// 预加载专用：仅资源字段，避免读台词/窗口设置
pub fn get_model_preload_config(data_dir: &std::path::Path, model_id: &str) -> Result<PetConfigPayload, String> {
    let assets = models::resolve_assets(data_dir, model_id)?;
    Ok(PetConfigPayload {
        model_id: assets.model_id,
        model_name: assets.model_name,
        asset_base: assets.asset_base,
        config_file: assets.config_file,
        skel_file: assets.skel_file,
        atlas_file: assets.atlas_file,
        png_file: assets.png_file,
        use_file_src: assets.use_file_src,
        power_mode: "balanced".into(),
        scale: 0.55,
        animations: vec![],
        idle_animation: Some("normal".into()),
        click_animation: Some("touch".into()),
        boot_animation: Some("normal".into()),
        return_idle_animation: Some("normal".into()),
        drag_animation: Some("tuozhuai".into()),
        random_animations: vec![],
        random_min_sec: 30,
        random_max_sec: 120,
        lines: vec![],
        window_width: 0.0,
        window_height: 0.0,
        offset_x: 0.0,
        offset_y: 0.0,
        bubble_enabled: false,
    })
}

pub fn screen_bounds() -> PetScreenBounds {
    let (left, top, right, bottom) = virtual_screen_bounds();
    PetScreenBounds {
        left,
        top,
        right,
        bottom,
    }
}

fn virtual_screen_bounds() -> (i32, i32, i32, i32) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        (left, top, left + w, top + h)
    }
}

fn default_position() -> (i32, i32) {
    screen_bottom_right(DEFAULT_WIDTH as i32, DEFAULT_HEIGHT as i32)
}

fn screen_bottom_right(win_w: i32, win_h: i32) -> (i32, i32) {
    let (vl, vt, vr, vb) = virtual_screen_bounds();
    let margin = 24;
    let x = (vr - win_w - margin).max(vl + margin);
    let y = (vb - win_h - margin).max(vt + margin);
    (x, y)
}

/// 窗口是否至少有一部分落在虚拟桌面可见区域内
fn is_position_visible(x: i32, y: i32, win_w: i32, win_h: i32) -> bool {
    let (vl, vt, vr, vb) = virtual_screen_bounds();
    x + win_w > vl + SCREEN_MARGIN
        && x < vr - SCREEN_MARGIN
        && y + win_h > vt + SCREEN_MARGIN
        && y < vb - SCREEN_MARGIN
}

pub fn clamp_pet_position(x: i32, y: i32, win_w: i32, win_h: i32) -> (i32, i32) {
    let (vl, vt, vr, vb) = virtual_screen_bounds();
    let min_x = vl + SCREEN_MARGIN;
    let min_y = vt + SCREEN_MARGIN;
    let max_x = (vr - win_w - SCREEN_MARGIN).max(min_x);
    let max_y = (vb - win_h - SCREEN_MARGIN).max(min_y);
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

fn resolve_pet_position(x: i32, y: i32, win_w: i32, win_h: i32) -> (i32, i32) {
    if !is_position_visible(x, y, win_w, win_h) {
        return screen_bottom_right(win_w, win_h);
    }
    clamp_pet_position(x, y, win_w, win_h)
}

fn load_position(db: &rusqlite::Connection) -> (i32, i32) {
    let x = crate::db::get_setting(db, "pet_x")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| default_position().0);
    let y = crate::db::get_setting(db, "pet_y")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| default_position().1);
    (x, y)
}

pub fn save_position(
    db: &rusqlite::Connection,
    x: i32,
    y: i32,
    win_w: Option<i32>,
    win_h: Option<i32>,
) -> Result<PetPoint, String> {
    let (w, h) = match (win_w, win_h) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => {
            let (lw, lh) = get_window_size(db);
            (lw as i32, lh as i32)
        }
    };
    let (x, y) = clamp_pet_position(x, y, w, h);
    crate::db::set_setting(db, "pet_x", &x.to_string()).map_err(|e| e.to_string())?;
    crate::db::set_setting(db, "pet_y", &y.to_string()).map_err(|e| e.to_string())?;
    Ok(PetPoint { x, y })
}

fn clamp_position_for_window(win: &WebviewWindow, x: i32, y: i32) -> (i32, i32) {
    let (w, h) = win
        .outer_size()
        .map(|s| (s.width as i32, s.height as i32))
        .unwrap_or((DEFAULT_WIDTH as i32, DEFAULT_HEIGHT as i32));
    clamp_pet_position(x, y, w, h)
}

fn attach_position_guard(app: &AppHandle, win: &WebviewWindow) {
    let Some(rt) = app.try_state::<PetRuntimeState>() else {
        return;
    };
    if rt
        .position_guard_attached
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    let app = app.clone();
    let win = win.clone();
    win.clone().on_window_event(move |event| {
        let WindowEvent::Moved(pos) = event else {
            return;
        };
        let Some(rt) = app.try_state::<PetRuntimeState>() else {
            return;
        };
        if rt.position_clamp_suppress.load(Ordering::Relaxed) {
            return;
        }
        let (cx, cy) = clamp_position_for_window(&win, pos.x, pos.y);
        if cx == pos.x && cy == pos.y {
            return;
        }
        rt.position_clamp_suppress.store(true, Ordering::Relaxed);
        let _ = win.set_position(PhysicalPosition::new(cx, cy));
        rt.position_clamp_suppress.store(false, Ordering::Relaxed);
    });
}

fn set_pet_position(win: &WebviewWindow, rt: &PetRuntimeState, x: i32, y: i32) -> Result<(), String> {
    let (cx, cy) = clamp_position_for_window(win, x, y);
    rt.position_clamp_suppress.store(true, Ordering::Relaxed);
    win.set_position(PhysicalPosition::new(cx, cy))
        .map_err(|e| e.to_string())?;
    rt.position_clamp_suppress.store(false, Ordering::Relaxed);
    Ok(())
}

pub fn ensure_pet_window(app: &AppHandle, st: &AppState) -> Result<(), String> {
    fn reset_page_load_state(app: &AppHandle) {
        if let Some(rt) = app.try_state::<PetRuntimeState>() {
            rt.page_load_finished.store(false, Ordering::Release);
            rt.page_load_count.store(0, Ordering::Release);
            rt.spine_ready.store(false, Ordering::Release);
            if let Ok(mut guard) = rt.spine_ready_model_id.lock() {
                guard.clear();
            }
        }
    }

    fn create(app: &AppHandle, st: &AppState) -> Result<(), String> {
        if app.get_webview_window(PET_LABEL).is_some() {
            return Ok(());
        }

        reset_page_load_state(app);

        let (w, h) = {
            let db = crate::db::lock_conn(&st.db)?;
            get_window_size(&db)
        };

        let app_for_load = app.clone();
        WebviewWindowBuilder::new(app, PET_LABEL, pet_webview_url())
            .title("小寒桌宠")
            .inner_size(w, h)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .visible(false)
            .focused(false)
            .on_page_load(move |_window, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    if let Some(rt) = app_for_load.try_state::<PetRuntimeState>() {
                        rt.page_load_finished.store(true, Ordering::Release);
                        let count = rt.page_load_count.fetch_add(1, Ordering::Relaxed) + 1;
                        // 首启：show_pet 会在页面就绪后 nudge；此处 reload 会与 nudge 竞态致碎块
                        if count > 1 {
                            if let Some(win) = app_for_load.get_webview_window(PET_LABEL) {
                                if win.is_visible().unwrap_or(false) {
                                    let _ = app_for_load.emit_to(PET_LABEL, "pet-reload", ());
                                }
                            }
                        }
                    }
                }
            })
            .build()
            .map_err(|e| e.to_string())?;
        if app.get_webview_window(PET_LABEL).is_none() {
            return Err("桌宠窗口创建失败".into());
        }
        Ok(())
    }

    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        let _guard = rt
            .window_create_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        create(app, st)
    } else {
        create(app, st)
    }
}

/// 退出准备阶段：立刻隐藏桌宠/菜单并通知前端停止渲染（不等待 RunEvent::Exit）
pub fn hide_pet_windows_immediately(app: &AppHandle) {
    let _ = app.emit_to(PET_LABEL, "pet-app-exiting", ());
    let _ = app.emit_to(PET_MENU_LABEL, "pet-app-exiting", ());
    if let Some(win) = app.get_webview_window(PET_MENU_LABEL) {
        detach_menu_window_effects(&win);
    }
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        detach_pet_window_effects(&win);
    }
}

pub fn destroy_pet_window(app: &AppHandle) -> Result<(), String> {
    destroy_pet_menu_window(app);
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        detach_pet_window_effects(&win);
        win.close().map_err(|e| e.to_string())?;
        wait_pet_window_closed(app, Duration::from_millis(2000))?;
    }
    reset_pet_runtime_state(app);
    Ok(())
}

fn stop_pet_runtime(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.scheduler_running.store(false, Ordering::SeqCst);
    }
    if let Some(st) = app.try_state::<Arc<AppState>>() {
        companion_session_stop(st.inner());
    }
}

/// 立刻解除透明置顶窗对桌面的遮挡（隐藏 + 取消置顶 + 穿透）。退出路径复用，不调用 close()。
fn detach_pet_window_effects(win: &WebviewWindow) {
    let _ = win.set_ignore_cursor_events(true);
    let _ = win.set_always_on_top(false);
    let _ = win.hide();
}

#[cfg(windows)]
pub(crate) fn pump_ui_messages() {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, PeekMessageW, TranslateMessage, PM_REMOVE,
        };
        let mut msg = std::mem::zeroed();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn pump_ui_messages() {}

fn drain_ui_after_pet_exit(app: &AppHandle) {
    for _ in 0..3 {
        pump_ui_messages();
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = app;
}

/// 退出前隐藏全部 WebView 并泵送消息，减轻 Chrome_WidgetWin_0 1411/1412 噪声。
pub fn finalize_all_webviews_for_exit(app: &AppHandle) {
    destroy_pet_window_for_exit(app);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    drain_ui_after_pet_exit(app);
}

/// 应用退出：解除桌宠/菜单窗对桌面的遮挡。仅 detach（隐藏/取消置顶），不 close()，
/// 避免 WebView2 在进程退出前 PostMessage 到已销毁 HWND（1412 / 0x80070578）。
static PET_EXIT_DESTROYED: AtomicBool = AtomicBool::new(false);

pub fn destroy_pet_window_for_exit(app: &AppHandle) {
    if PET_EXIT_DESTROYED.swap(true, Ordering::SeqCst) {
        return;
    }
    stop_pet_runtime(app);
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        reset_menu_runtime(&rt);
    }
    let _ = app.emit_to(PET_LABEL, "pet-app-exiting", ());
    let _ = app.emit_to(PET_MENU_LABEL, "pet-app-exiting", ());
    for _ in 0..2 {
        pump_ui_messages();
        std::thread::sleep(Duration::from_millis(10));
    }
    if let Some(win) = app.get_webview_window(PET_MENU_LABEL) {
        detach_menu_window_effects(&win);
    }
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        detach_pet_window_effects(&win);
    }
    reset_pet_runtime_state(app);
}

fn reset_pet_runtime_state(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.position_guard_attached
            .store(false, Ordering::SeqCst);
        rt.page_load_finished.store(false, Ordering::Release);
        rt.page_load_count.store(0, Ordering::Release);
        rt.spine_ready.store(false, Ordering::Release);
        if let Ok(mut guard) = rt.spine_ready_model_id.lock() {
            guard.clear();
        }
        rt.fullscreen_suppressed.store(false, Ordering::Relaxed);
        rt.user_hidden_pet.store(false, Ordering::Release);
    }
}

fn wait_pet_window_closed(app: &AppHandle, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while app.get_webview_window(PET_LABEL).is_some() {
        if Instant::now() >= deadline {
            return Err("桌宠窗口关闭超时，请稍后重试".into());
        }
        pump_ui_messages();
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

pub fn show_pet(app: &AppHandle, st: &Arc<AppState>) -> Result<(), String> {
    ensure_pet_window(app, st)?;
    let win = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口创建失败".to_string())?;

    prepare_pet_webview(&win)?;

    let (w, h) = {
        let db = crate::db::lock_conn(&st.db)?;
        get_window_size(&db)
    };

    let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(w, h)));
    let (resolve_w, resolve_h) = win
        .outer_size()
        .map(|s| (s.width as i32, s.height as i32))
        .unwrap_or((w as i32, h as i32));
    let (x, y) = {
        let db = crate::db::lock_conn(&st.db)?;
        let pos = load_position(&db);
        let (cx, cy) = resolve_pet_position(pos.0, pos.1, resolve_w, resolve_h);
        if (cx, cy) != pos {
            let _ = save_position(&db, cx, cy, Some(resolve_w), Some(resolve_h));
            crate::log::info(format!(
                "桌宠: 位置 ({},{}) 在屏幕外，已重置为 ({},{})",
                pos.0, pos.1, cx, cy
            ));
        }
        (cx, cy)
    };
    let rt = app.state::<PetRuntimeState>();
    attach_position_guard(app, &win);
    set_pet_position(&win, &rt, x, y)?;
    let _ = win.set_always_on_top(true);
    win.show().map_err(|e| e.to_string())?;
    companion_session_start(st);
    if should_full_reload_pet(app) {
        schedule_pet_reload_after_show(app, st.clone());
    } else {
        schedule_pet_resume_after_show(app);
    }
    let _ = app.emit_to(PET_LABEL, "pet-sync-click-through", ());
    let _ = ensure_pet_menu_window(app);
    sync_menu_z_order_if_visible(app);
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.user_hidden_pet.store(false, Ordering::Release);
    }
    sync_pet_visibility_ui(app);
    Ok(())
}

pub fn hide_pet(app: &AppHandle, destroy: bool) -> Result<(), String> {
    hide_pet_impl(app, destroy, true)
}

fn hide_pet_transient(app: &AppHandle) -> Result<(), String> {
    hide_pet_impl(app, false, false)
}

fn hide_pet_impl(app: &AppHandle, destroy: bool, mark_user_hidden: bool) -> Result<(), String> {
    if mark_user_hidden {
        if let Some(rt) = app.try_state::<PetRuntimeState>() {
            rt.user_hidden_pet.store(true, Ordering::Release);
        }
    }
    stop_pet_runtime(app);
    let _ = hide_pet_menu(app);
    let _ = app.emit_to(PET_LABEL, "pet-hidden", ());
    if destroy {
        destroy_pet_window(app)?;
    } else if let Some(win) = app.get_webview_window(PET_LABEL) {
        detach_pet_window_effects(&win);
    }
    if mark_user_hidden || destroy {
        sync_pet_visibility_ui(app);
    }
    Ok(())
}

fn should_full_reload_pet(app: &AppHandle) -> bool {
    let Some(rt) = app.try_state::<PetRuntimeState>() else {
        return true;
    };
    if !rt.page_load_finished.load(Ordering::Acquire) {
        return true;
    }
    !rt.spine_ready.load(Ordering::Acquire)
}

fn schedule_pet_resume_after_show(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        if app
            .get_webview_window(PET_LABEL)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
        {
            resume_pet(&app);
        }
    });
}

pub fn mark_spine_ready(app: &AppHandle, model_id: &str) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.spine_ready.store(true, Ordering::Release);
        if let Ok(mut guard) = rt.spine_ready_model_id.lock() {
            *guard = model_id.to_string();
        }
    }
}

fn try_complete_switch_wait(app: &AppHandle, model_id: &str) {
    let target = app
        .try_state::<PetRuntimeState>()
        .and_then(|rt| rt.switch_target_model_id.lock().ok().and_then(|g| g.clone()));
    let Some(target) = target else {
        return;
    };
    if models::canonical_model_id(&target) != models::canonical_model_id(model_id) {
        return;
    }
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        if let Ok(mut tx_guard) = rt.switch_confirm_tx.lock() {
            if let Some(tx) = tx_guard.take() {
                let _ = tx.send(model_id.to_string());
                eprintln!("xiaohan-daily: pet menu switch confirmed model={model_id}");
            }
        }
    }
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        if let Ok(mut id_guard) = rt.switch_target_model_id.lock() {
            *id_guard = None;
        }
    }
}

fn cancel_switch_wait(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        if let Ok(mut tx_guard) = rt.switch_confirm_tx.lock() {
            tx_guard.take();
        }
        if let Ok(mut id_guard) = rt.switch_target_model_id.lock() {
            *id_guard = None;
        }
        if let Ok(mut req_guard) = rt.switch_request_id.lock() {
            *req_guard = None;
        }
    }
}

fn begin_switch_wait(app: &AppHandle, target_model_id: &str) -> (mpsc::Receiver<String>, u64) {
    cancel_switch_wait(app);
    clear_spine_ready(app);
    let switch_id = app
        .try_state::<PetRuntimeState>()
        .map(|rt| rt.switch_seq.fetch_add(1, Ordering::Relaxed) + 1)
        .unwrap_or(1);
    let (tx, rx) = mpsc::sync_channel(1);
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        if let Ok(mut tx_guard) = rt.switch_confirm_tx.lock() {
            *tx_guard = Some(tx);
        }
        if let Ok(mut id_guard) = rt.switch_target_model_id.lock() {
            *id_guard = Some(target_model_id.to_string());
        }
        if let Ok(mut req_guard) = rt.switch_request_id.lock() {
            *req_guard = Some(switch_id);
        }
    }
    eprintln!("xiaohan-daily: pet menu switch wait id={switch_id} target={target_model_id}");
    (rx, switch_id)
}

fn emit_pet_switch(app: &AppHandle, st: &AppState, switch_id: u64) -> Result<(), String> {
    let config = get_config(st)?;
    let payload = PetSwitchPayload { switch_id, config };
    let _ = app.emit_to(PET_LABEL, "pet-switch", payload);
    Ok(())
}

pub fn confirm_switch(app: &AppHandle, switch_id: u64, model_id: &str) {
    let expected = app
        .try_state::<PetRuntimeState>()
        .and_then(|rt| rt.switch_request_id.lock().ok().and_then(|g| *g));
    if expected != Some(switch_id) {
        eprintln!(
            "xiaohan-daily: pet switch confirm ignored id={switch_id} expected={expected:?}"
        );
        return;
    }
    mark_spine_ready(app, model_id);
    try_complete_switch_wait(app, model_id);
}

/// 菜单专用：切换模型并阻塞到桌宠前端确认（或超时）。同步实现，可在测试 API 线程直接调用。
pub fn menu_switch_to_model_blocking(
    app: &AppHandle,
    st: Arc<AppState>,
    raw_model_id: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let assets = models::resolve_assets(st.data_dir(), raw_model_id)?;
    let canonical = assets.model_id.clone();

    if app.get_webview_window(PET_LABEL).is_none() {
        set_active_model(app, st.clone(), raw_model_id)?;
        if await_spine_ready_blocking(app, &st, Some(canonical.clone()), timeout_ms) {
            return Ok(canonical);
        }
        return Err("模型加载超时，请稍后重试".into());
    }

    let (rx, switch_id) = begin_switch_wait(app, &canonical);
    if let Err(e) = emit_pet_switch(app, &st, switch_id) {
        cancel_switch_wait(app);
        return Err(e);
    }
    let _ = app.emit_to(PET_MENU_LABEL, "pet-menu-refresh-pickers", ());
    let wait_t0 = Instant::now();
    let result = match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(id) => {
            eprintln!(
                "xiaohan-daily: pet menu switch done id={switch_id} model={id} ms={}",
                wait_t0.elapsed().as_millis()
            );
            Ok(id)
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            cancel_switch_wait(app);
            eprintln!(
                "xiaohan-daily: pet menu switch timeout id={switch_id} target={canonical} ms={}",
                wait_t0.elapsed().as_millis()
            );
            Err("模型加载超时，请稍后重试".into())
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            cancel_switch_wait(app);
            Err("桌宠加载已中断".into())
        }
    };
    result
}

/// 菜单专用：切换模型并阻塞到桌宠前端确认（或超时）
pub async fn menu_switch_to_model(
    app: &AppHandle,
    st: Arc<AppState>,
    raw_model_id: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let app = app.clone();
    let raw = raw_model_id.to_string();
    tokio::task::spawn_blocking(move || menu_switch_to_model_blocking(&app, st, &raw, timeout_ms))
        .await
        .map_err(|e| format!("menu switch task failed: {e}"))?
}

pub async fn menu_switch_skin(
    app: &AppHandle,
    st: Arc<AppState>,
    character_id: &str,
    skin_id: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let raw_id = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::character::select_character_skin(st.data_dir(), &db, character_id, skin_id)?
    };
    menu_switch_to_model(app, st, &raw_id, timeout_ms).await
}

pub async fn menu_switch_character(
    app: &AppHandle,
    st: Arc<AppState>,
    character_id: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let raw_id = {
        let db = crate::db::lock_conn(&st.db)?;
        let manifest = crate::persona::load_manifest(st.data_dir());
        crate::character::set_active_character(st.data_dir(), &db, &manifest, character_id)?;
        models::active_model_id(&db)
    };
    menu_switch_to_model(app, st, &raw_id, timeout_ms).await
}

fn await_spine_ready_blocking(
    app: &AppHandle,
    _st: &AppState,
    expected_model_id: Option<String>,
    timeout_ms: u64,
) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let (ready, ready_model) = app
            .try_state::<PetRuntimeState>()
            .map(|rt| {
                (
                    rt.spine_ready.load(Ordering::Acquire),
                    rt.spine_ready_model_id
                        .lock()
                        .ok()
                        .map(|g| g.clone())
                        .unwrap_or_default(),
                )
            })
            .unwrap_or((false, String::new()));
        if ready {
            let ready_canon = models::canonical_model_id(&ready_model);
            if let Some(expected) = expected_model_id.as_deref() {
                if !ready_model.is_empty()
                    && ready_canon == models::canonical_model_id(expected)
                {
                    return true;
                }
            } else if !ready_model.is_empty() {
                return true;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub async fn await_spine_ready(
    app: &AppHandle,
    _st: &AppState,
    expected_model_id: Option<String>,
    timeout_ms: u64,
) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let (ready, ready_model) = app
            .try_state::<PetRuntimeState>()
            .map(|rt| {
                (
                    rt.spine_ready.load(Ordering::Acquire),
                    rt.spine_ready_model_id
                        .lock()
                        .ok()
                        .map(|g| g.clone())
                        .unwrap_or_default(),
                )
            })
            .unwrap_or((false, String::new()));
        if ready {
            let ready_canon = models::canonical_model_id(&ready_model);
            if let Some(expected) = expected_model_id.as_deref() {
                if !ready_model.is_empty()
                    && ready_canon == models::canonical_model_id(expected)
                {
                    return true;
                }
            } else if !ready_model.is_empty() {
                return true;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub fn clear_spine_ready(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.spine_ready.store(false, Ordering::Release);
        if let Ok(mut guard) = rt.spine_ready_model_id.lock() {
            guard.clear();
        }
    }
}

pub fn resume_pet(app: &AppHandle) {
    let _ = app.emit_to(PET_LABEL, "pet-resume", ());
    let _ = app.emit_to(PET_LABEL, "pet-sync-click-through", ());
}

fn schedule_pet_reload_after_show(app: &AppHandle, st: Arc<AppState>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let already_ready = app
            .try_state::<PetRuntimeState>()
            .map(|rt| rt.page_load_finished.load(Ordering::Acquire))
            .unwrap_or(false);
        if !already_ready && !wait_pet_page_ready(&app, Duration::from_secs(5)).await {
            crate::log::info("桌宠: pet.html 加载超时，仍尝试初始化");
        }
        if !already_ready {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        if app
            .get_webview_window(PET_LABEL)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
        {
            let spine_ready = app
                .try_state::<PetRuntimeState>()
                .map(|rt| rt.spine_ready.load(Ordering::Acquire))
                .unwrap_or(false);
            if spine_ready {
                resume_pet(&app);
            } else {
                nudge_pet(&app, &st);
            }
        }
    });
}

async fn wait_pet_page_ready(app: &AppHandle, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let ready = app
            .try_state::<PetRuntimeState>()
            .map(|rt| rt.page_load_finished.load(Ordering::Acquire))
            .unwrap_or(false);
        if ready {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub fn reload_pet(app: &AppHandle, st: &Arc<AppState>) -> Result<(), String> {
    if app.get_webview_window(PET_LABEL).is_some() {
        nudge_pet(app, st);
        return Ok(());
    }
    let enabled = st
        .db
        .lock()
        .ok()
        .map(|db| is_enabled(&db))
        .unwrap_or(false);
    if enabled {
        show_pet(app, st)?;
    }
    Ok(())
}

/// 通知桌宠前端重新读配置（不销毁窗口）；payload 为当前配置快照，避免切换时再走 IPC get_config
pub fn nudge_pet(app: &AppHandle, st: &AppState) {
    clear_spine_ready(app);
    let payload = get_config(st).ok();
    let _ = app.emit_to(PET_LABEL, "pet-reload", payload);
}

/// 设置页调整角色大小时仅更新 CSS scale，不重建 Spine
pub fn nudge_pet_scale(app: &AppHandle, scale: f64) {
    let s = scale.clamp(0.4, 1.5);
    let _ = app.emit_to(PET_LABEL, "pet-scale-changed", s);
}

/// 仅刷新动作配置（待机/点击/随机），不重建 Spine
pub fn nudge_pet_animations(app: &AppHandle) {
    let _ = app.emit_to(PET_LABEL, "pet-animations-changed", ());
}

#[derive(Clone, Serialize)]
pub struct PetPreviewAnimationPayload {
    pub animation: String,
    pub r#loop: bool,
}

/// 设置页点击动作名时，在桌宠窗口演示播放（不强行 show，避免透明窗挡桌面）
pub fn preview_animation(
    app: &AppHandle,
    _st: &AppState,
    animation: &str,
    loop_anim: bool,
) -> Result<(), String> {
    let name = animation.trim();
    if name.is_empty() {
        return Err("动作名不能为空".into());
    }
    if !pet_visible(app) {
        return Ok(());
    }
    let _ = app.emit_to(
        PET_LABEL,
        "pet-preview-animation",
        PetPreviewAnimationPayload {
            animation: name.to_string(),
            r#loop: loop_anim,
        },
    );
    Ok(())
}

pub fn set_active_model(app: &AppHandle, st: Arc<AppState>, model_id: &str) -> Result<(), String> {
    let data_dir = st.data_dir();
    let assets = models::resolve_assets(data_dir, model_id)?;
    let db = crate::db::lock_conn(&st.db)?;
    models::set_active_model_id(&db, &assets.model_id)?;
    crate::character::sync_from_model(data_dir, &db, &assets.model_id);
    let enabled = is_enabled(&db);
    drop(db);
    if enabled {
        if app.get_webview_window(PET_LABEL).is_some() {
            nudge_pet(app, &st);
            let _ = app.emit_to(PET_MENU_LABEL, "pet-menu-refresh-pickers", ());
        } else {
            show_pet(app, &st)?;
        }
    }
    Ok(())
}
pub fn wiki_bulk_import_progress(app: &AppHandle) -> Option<lines_import::PetWikiBulkImportProgress> {
    app.try_state::<PetRuntimeState>()
        .and_then(|rt| rt.wiki_bulk_last_progress.lock().ok().map(|g| g.clone()))
        .flatten()
}

pub fn wiki_bulk_import_is_running(app: &AppHandle) -> bool {
    app.try_state::<PetRuntimeState>()
        .map(|rt| rt.wiki_bulk_import_running.load(Ordering::SeqCst))
        .unwrap_or(false)
}

pub fn reset_wiki_bulk_control(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.wiki_bulk_stop_requested
            .store(false, Ordering::SeqCst);
        rt.wiki_bulk_paused.store(false, Ordering::SeqCst);
    }
}

pub fn request_wiki_bulk_stop(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.wiki_bulk_stop_requested
            .store(true, Ordering::SeqCst);
        rt.wiki_bulk_paused.store(false, Ordering::SeqCst);
    }
}

pub fn set_wiki_bulk_paused(app: &AppHandle, paused: bool) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.wiki_bulk_paused.store(paused, Ordering::SeqCst);
    }
}

pub fn wiki_bulk_stop_requested(app: &AppHandle) -> bool {
    app.try_state::<PetRuntimeState>()
        .map(|rt| rt.wiki_bulk_stop_requested.load(Ordering::SeqCst))
        .unwrap_or(false)
}

pub fn wiki_bulk_is_paused(app: &AppHandle) -> bool {
    app.try_state::<PetRuntimeState>()
        .map(|rt| rt.wiki_bulk_paused.load(Ordering::SeqCst))
        .unwrap_or(false)
}

pub async fn wiki_bulk_await_pause_or_stop(app: &AppHandle, st: &AppState) -> bool {
    loop {
        if st.stop_flag.load(Ordering::Relaxed) || wiki_bulk_stop_requested(app) {
            return true;
        }
        if !wiki_bulk_is_paused(app) {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

fn reset_stale_wiki_bulk_import(app: &AppHandle) {
    let Some(rt) = app.try_state::<PetRuntimeState>() else {
        return;
    };
    if !rt.wiki_bulk_import_running.load(Ordering::SeqCst) {
        return;
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    let should_reset = rt
        .wiki_bulk_last_progress
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|p| {
            if p.phase == "done" || p.phase == "error" {
                return true;
            }
            if p.phase == "scan"
                && p.index == 0
                && p.total == 0
                && p.updated_at_ms > 0
                && now_ms - p.updated_at_ms > 45_000
            {
                return true;
            }
            if (p.phase == "scan" || p.phase == "import")
                && p.updated_at_ms > 0
                && now_ms - p.updated_at_ms > 300_000
            {
                return true;
            }
            false
        }))
        .unwrap_or(false);
    if should_reset {
        rt.wiki_bulk_import_running
            .store(false, Ordering::SeqCst);
        if let Ok(mut guard) = rt.wiki_bulk_last_progress.lock() {
            *guard = None;
        }
    }
}

/// 由前端就绪后触发，避免事件在监听器挂载前丢失
pub fn start_wiki_bulk_import(app: AppHandle, st: Arc<AppState>) -> lines_import::PetWikiBulkImportStartResult {
    reset_stale_wiki_bulk_import(&app);
    if st.stop_flag.load(Ordering::Relaxed) {
        return lines_import::PetWikiBulkImportStartResult {
            started: false,
            already_running: false,
        };
    }
    let runtime = app.state::<PetRuntimeState>();
    if runtime.wiki_bulk_import_running.load(Ordering::SeqCst) {
        return lines_import::PetWikiBulkImportStartResult {
            started: false,
            already_running: true,
        };
    }
    if runtime
        .wiki_bulk_import_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return lines_import::PetWikiBulkImportStartResult {
            started: false,
            already_running: true,
        };
    }

    reset_wiki_bulk_control(&app);

    lines_import::emit_wiki_bulk_import_progress(
        &app,
        lines_import::PetWikiBulkImportProgress {
            phase: "scan".into(),
            index: 0,
            total: 0,
            model_id: String::new(),
            model_name: String::new(),
            message: "准备批量导入…".into(),
            lines_imported: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            updated_at_ms: 0,
        },
    );

    tauri::async_runtime::spawn(async move {
        struct BulkImportGuard(AppHandle);
        impl Drop for BulkImportGuard {
            fn drop(&mut self) {
                if let Some(rt) = self.0.try_state::<PetRuntimeState>() {
                    rt.wiki_bulk_import_running
                        .store(false, Ordering::SeqCst);
                }
            }
        }
        let _guard = BulkImportGuard(app.clone());

        if st.stop_flag.load(Ordering::Relaxed) {
            lines_import::emit_wiki_bulk_import_progress(
                &app,
                lines_import::PetWikiBulkImportProgress {
                    phase: "done".into(),
                    index: 0,
                    total: 0,
                    model_id: String::new(),
                    model_name: String::new(),
                    message: "导入已取消".into(),
                    lines_imported: 0,
                    succeeded: 0,
                    failed: 0,
                    skipped: 0,
                    updated_at_ms: 0,
                },
            );
            return;
        }

        let (ok, _skip, _fail) = wiki_scrape::run_bulk_wiki_lines_import(&app, &st).await;
        if ok > 0 {
            nudge_pet_animations(&app);
        }
    });
    lines_import::PetWikiBulkImportStartResult {
        started: true,
        already_running: false,
    }
}

pub fn pause_wiki_bulk_import(app: &AppHandle) -> bool {
    if !wiki_bulk_import_is_running(app) {
        return false;
    }
    set_wiki_bulk_paused(app, true);
    if let Some(mut progress) = wiki_bulk_import_progress(app) {
        if progress.phase == "scan" || progress.phase == "import" {
            progress.phase = "paused".into();
            progress.message = "导入已暂停，点击继续恢复".into();
            lines_import::emit_wiki_bulk_import_progress(app, progress);
        }
    }
    true
}

pub fn resume_wiki_bulk_import(app: &AppHandle) -> bool {
    if !wiki_bulk_import_is_running(app) {
        return false;
    }
    set_wiki_bulk_paused(app, false);
    if let Some(mut progress) = wiki_bulk_import_progress(app) {
        if progress.phase == "paused" {
            progress.phase = "import".into();
            progress.message = "继续导入…".into();
            lines_import::emit_wiki_bulk_import_progress(app, progress);
        }
    }
    true
}

pub fn stop_wiki_bulk_import(app: &AppHandle) -> bool {
    if !wiki_bulk_import_is_running(app) {
        return false;
    }
    request_wiki_bulk_stop(app);
    true
}

pub fn set_enabled(app: &AppHandle, st: Arc<AppState>, enabled: bool) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::db::set_setting(&db, "pet_enabled", if enabled { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    drop(db);

    if enabled {
        show_pet(app, &st)?;
        ensure_remark_scheduler(app.clone(), st);
    } else {
        hide_pet(app, true)?;
    }
    Ok(())
}

/// 启动时提前创建隐藏桌宠 WebView，与主窗口首屏并行，缩短首显等待。
pub fn prewarm_on_startup(app: &AppHandle, st: Arc<AppState>) -> Result<(), String> {
    {
        let db = crate::db::lock_conn(&st.db)?;
        if !is_enabled(&db) {
            return Ok(());
        }
    }
    #[cfg(not(debug_assertions))]
    {
        ensure_pet_window(app, &st)?;
    }
    Ok(())
}

pub fn sync_on_startup(app: &AppHandle, st: Arc<AppState>) -> Result<(), String> {
    {
        let db = crate::db::lock_conn(&st.db)?;
        if crate::db::get_setting(&db, "pet_enabled").is_none() {
            crate::db::set_setting(&db, "pet_enabled", "1").map_err(|e| e.to_string())?;
        }
        if !is_enabled(&db) {
            drop(db);
            if let Some(win) = app.get_webview_window(PET_LABEL) {
                let _ = win.hide();
            }
            return Ok(());
        }
    }

    // 异步显示：等 Vite 就绪后再创建/显示，避免透明空窗
    let app2 = app.clone();
    let st2 = st.clone();
    tauri::async_runtime::spawn(async move {
        #[cfg(debug_assertions)]
        wait_frontend_ready(12).await;

        let boot_delay = if crate::system::autostart::is_tray_launch() {
            Duration::from_millis(150)
        } else {
            Duration::from_millis(20)
        };
        tokio::time::sleep(boot_delay).await;
        for attempt in 0..8 {
            if st2.stop_flag.load(Ordering::Relaxed) {
                break;
            }
            let enabled = st2
                .db
                .lock()
                .ok()
                .map(|db| is_enabled(&db))
                .unwrap_or(false);
            if !enabled {
                return;
            }
            match show_pet(&app2, &st2) {
                Ok(()) => {}
                Err(e) => {
                    crate::log::warn(format!("桌宠 show_pet 失败 (attempt {attempt}): {e}"));
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    continue;
                }
            }
            if pet_visible(&app2) {
                ensure_remark_scheduler(app2.clone(), st2.clone());
                return;
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
            if attempt == 7 {
                crate::log::warn("桌宠启动失败: 窗口已创建但多次尝试后仍不可见");
            }
        }
    });

    Ok(())
}

async fn interruptible_sleep_secs(st: &AppState, secs: u64) {
    let mut remaining = Duration::from_secs(secs);
    let step = Duration::from_millis(100);
    while remaining > Duration::ZERO {
        if st.stop_flag.load(Ordering::Relaxed) {
            return;
        }
        let nap = remaining.min(step);
        tokio::time::sleep(nap).await;
        remaining = remaining.saturating_sub(nap);
    }
}

pub fn ensure_remark_scheduler(app: AppHandle, st: Arc<AppState>) {
    let runtime = app.state::<PetRuntimeState>();
    if runtime
        .scheduler_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    tauri::async_runtime::spawn(async move {
        struct SchedulerGuard(AppHandle);
        impl Drop for SchedulerGuard {
            fn drop(&mut self) {
                if let Some(rt) = self.0.try_state::<PetRuntimeState>() {
                    rt.scheduler_running.store(false, Ordering::SeqCst);
                }
            }
        }
        let _scheduler_guard = SchedulerGuard(app.clone());

        let mut last_fg_key = String::new();
        loop {
            if st.stop_flag.load(Ordering::Relaxed) {
                break;
            }

            let enabled = {
                let Ok(db) = st.lock_db() else {
                    interruptible_sleep_secs(&st, 5).await;
                    continue;
                };
                is_enabled(&db)
            };

            if !enabled {
                interruptible_sleep_secs(&st, 10).await;
                continue;
            }

            sync_fullscreen_visibility(&app, &st);

            let interval_sec = {
                let Ok(db) = st.lock_db() else {
                    interruptible_sleep_secs(&st, 5).await;
                    continue;
                };
                let model_id = models::active_model_id(&db);
                let meta = models::read_animation_meta(st.data_dir(), &db, &model_id);
                model_remark_interval_sec(&db, &meta)
            };

            let bubble_on = {
                let Ok(db) = st.lock_db() else {
                    interruptible_sleep_secs(&st, 5).await;
                    continue;
                };
                is_bubble_enabled(&db)
            };

            if interval_sec == 0 || !pet_visible(&app) || !bubble_on {
                interruptible_sleep_secs(&st, 5).await;
                continue;
            }

            if is_main_window_visible(&app) || is_pet_menu_visible(&app) || pet_menu_open_or_pending(&app)
            {
                interruptible_sleep_secs(&st, 5).await;
                continue;
            }

            let fg_key = current_context_key(&st);
            let context_changed = !fg_key.is_empty() && fg_key != last_fg_key;
            if context_changed {
                last_fg_key = fg_key.clone();
                let _ = app.emit_to(PET_LABEL, "pet-context-changed", fg_key);
            }

            let should_emit = context_changed || should_emit_by_interval(&app, interval_sec);
            if should_emit {
                // [live2d-only] 台词仅来自本地台词库，不走 AI / 时间线
                let remark = build_remark_from_lines(&st);
                if let Some(remark) = remark {
                    let _ = app.emit_to(PET_LABEL, "pet-remark", remark);
                    if let Some(rt) = app.try_state::<PetRuntimeState>() {
                        *rt.last_remark_at.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(Instant::now());
                    }
                }
            }

            let sleep_sec = if context_changed {
                60
            } else {
                interval_sec.max(30) as u64
            };
            interruptible_sleep_secs(&st, sleep_sec).await;
        }
    });
}

fn should_emit_by_interval(app: &AppHandle, interval_sec: i64) -> bool {
    let Some(rt) = app.try_state::<PetRuntimeState>() else {
        return true;
    };
    let guard = rt.last_remark_at.lock().unwrap_or_else(|e| e.into_inner());
    match *guard {
        None => true,
        Some(t) => t.elapsed() >= Duration::from_secs(interval_sec.max(60) as u64),
    }
}

fn current_context_key(st: &AppState) -> String {
    let fg = st.foreground.lock().ok().and_then(|g| g.clone());
    match fg {
        Some(f) => format!("{}|{}|{}", f.app_name, f.window_title, f.is_idle),
        None => String::new(),
    }
}

fn trim_remark(text: &str, max_chars: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= max_chars {
        return t.to_string();
    }
    format!("{}…", t.chars().take(max_chars).collect::<String>())
}

fn is_machine_text(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("开发：")
        || t.starts_with("文档：")
        || t.starts_with("浏览：")
        || t.contains("窗口「")
        || t.starts_with("[text·")
        || t.starts_with("[screenshot·")
}

fn sync_fullscreen_visibility(app: &AppHandle, st: &Arc<AppState>) {
    let fullscreen =
        crate::tracker::win32::is_foreground_fullscreen()
            && !crate::tracker::win32::is_foreground_own_process();
    let Some(rt) = app.try_state::<PetRuntimeState>() else {
        return;
    };
    if fullscreen {
        if pet_visible(app) && !rt.fullscreen_suppressed.load(Ordering::Relaxed) {
            let _ = hide_pet_transient(app);
            rt.fullscreen_suppressed.store(true, Ordering::Relaxed);
        }
    } else if rt.fullscreen_suppressed.swap(false, Ordering::Relaxed) {
        let user_hidden = rt.user_hidden_pet.load(Ordering::Acquire);
        let enabled = st
            .db
            .lock()
            .ok()
            .map(|db| is_enabled(&db))
            .unwrap_or(false);
        if enabled && !user_hidden {
            let _ = show_pet(app, st);
        }
    }
}

pub fn emit_remark(
    app: &AppHandle,
    st: &Arc<AppState>,
    text: &str,
    source: &str,
    animation: Option<String>,
) {
    let text = text.trim();
    if text.is_empty() || is_machine_text(text) {
        return;
    }

    let enabled = st
        .db
        .lock()
        .ok()
        .map(|db| is_enabled(&db))
        .unwrap_or(false);
    if !enabled {
        return;
    }

    let bubble_on = st
        .db
        .lock()
        .ok()
        .map(|db| is_bubble_enabled(&db))
        .unwrap_or(true);
    if !bubble_on {
        return;
    }

    if !pet_visible(app) {
        if let Some(rt) = app.try_state::<PetRuntimeState>() {
            if rt.fullscreen_suppressed.load(Ordering::Relaxed)
                || rt.user_hidden_pet.load(Ordering::Acquire)
            {
                return;
            }
            let _ = show_pet(app, st);
        }
    }
    if !pet_visible(app) {
        return;
    }

    if is_main_window_visible(app) || is_pet_menu_visible(app) || pet_menu_open_or_pending(app) {
        return;
    }

    let _ = app.emit_to(
        PET_LABEL,
        "pet-remark",
        PetRemarkPayload {
            text: trim_remark(text, 80),
            source: source.to_string(),
            animation,
        },
    );
}

/// Agent / MCP 测试：尽量展示台词，跳过气泡开关与 pet enabled 限制。
pub fn emit_remark_agent(
    app: &AppHandle,
    st: &Arc<AppState>,
    text: &str,
    animation: Option<String>,
) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() || is_machine_text(text) {
        return Err("台词为空或无效".into());
    }
    if !pet_visible(app) {
        if let Some(rt) = app.try_state::<PetRuntimeState>() {
            if rt.fullscreen_suppressed.load(Ordering::Relaxed) {
                return Err("全屏抑制中，无法显示桌宠".into());
            }
            let _ = show_pet(app, st);
        }
    }
    if !pet_visible(app) {
        return Err("桌宠窗口不可见".into());
    }
    let _ = app.emit_to(
        PET_LABEL,
        "pet-remark",
        PetRemarkPayload {
            text: trim_remark(text, 80),
            source: "mcp".into(),
            animation,
        },
    );
    Ok(())
}

pub fn emit_random_remark_agent(app: &AppHandle, st: &Arc<AppState>) -> Result<String, String> {
    let payload = build_remark_from_lines(st).ok_or_else(|| "当前模型没有可用台词".to_string())?;
    emit_remark_agent(app, st, &payload.text, payload.animation)?;
    Ok(payload.text)
}

pub fn build_remark_from_lines(st: &AppState) -> Option<PetRemarkPayload> {
    let db = st.lock_db().ok()?;
    let model_id = models::active_model_id(&db);
    let anim_meta = models::read_animation_meta(st.data_dir(), &db, &model_id);
    let line = models::pick_remark_line(&anim_meta, None)?;
    Some(PetRemarkPayload {
        text: trim_remark(&line.text, 80),
        source: "lines".into(),
        animation: line.animation,
    })
}

pub fn build_remark(st: &AppState) -> Option<PetRemarkPayload> {
    let db = st.lock_db().ok()?;
    let fg = st.foreground.lock().ok().and_then(|g| g.clone());

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    if let Ok(summary) = db.query_row(
        "SELECT summary FROM timeline_ai_cache WHERE substr(started_at, 1, 10) = ?1 ORDER BY started_at DESC LIMIT 1",
        [&today],
        |r| r.get::<_, String>(0),
    ) {
        let summary = summary.trim();
        if !summary.is_empty() && !is_machine_text(summary) {
            return Some(PetRemarkPayload {
                text: trim_remark(summary, 36),
                source: "timeline".into(),
                animation: None,
            });
        }
    }

    if let Ok(periods) = crate::db::periods::list_period_summaries_today(&db, 1) {
        if let Some(p) = periods.first() {
            let summary = p.summary.trim();
            if !summary.is_empty() {
                return Some(PetRemarkPayload {
                    text: trim_remark(summary, 36),
                    source: "period".into(),
                    animation: None,
                });
            }
        }
    }

    if let Ok(insights) = crate::db::insights::list_today(&db, 3) {
        for ins in insights {
            let summary = ins.summary.trim();
            if !summary.is_empty() && !is_machine_text(summary) {
                return Some(PetRemarkPayload {
                    text: trim_remark(summary, 36),
                    source: "insight".into(),
                    animation: None,
                });
            }
        }
    }

    let app_name = fg
        .as_ref()
        .map(|f| {
            crate::tracker::display_name::friendly_name(
                &f.exe_path,
                &f.app_name,
                &f.window_title,
            )
        })
        .unwrap_or_else(|| "电脑".into());
    let idle = fg.as_ref().map(|f| f.is_idle).unwrap_or(false);

    let text = if idle {
        format!("{app_name} 歇了一会儿，起来动动？")
    } else {
        match app_name.as_str() {
            "Cursor" | "VS Code" | "Visual Studio Code" => format!("又在 {app_name} 里写代码呢~"),
            "Microsoft Edge" | "Chrome" | "Firefox" => format!("在 {app_name} 里逛什么呢~"),
            _ => format!("正在用 {app_name} 呢~"),
        }
    };

    Some(PetRemarkPayload {
        text,
        source: "local".into(),
        animation: None,
    })
}


fn companion_session_start(st: &AppState) {
    if let Ok(db) = crate::db::lock_conn(&st.db) {
        let _ = crate::db::usage::open_companion_session(&db);
    }
}

fn companion_session_stop(st: &AppState) {
    if let Ok(db) = crate::db::lock_conn(&st.db) {
        let _ = crate::db::usage::close_companion_session(&db);
    }
}

/// 桌宠编辑/移动调试日志（JSONL，位于 data_dir/logs/pet-movement.jsonl）
pub fn append_movement_debug_logs(data_dir: &std::path::Path, lines: &[String]) -> Result<(), String> {
    append_jsonl_logs(data_dir, "pet-movement.jsonl", lines, false)
}

/// 桌宠显示层日志（JSONL + 终端 stderr）
pub fn append_display_debug_logs(data_dir: &std::path::Path, lines: &[String]) -> Result<(), String> {
    append_jsonl_logs(data_dir, "pet-display.jsonl", lines, true)
}

fn append_jsonl_logs(
    data_dir: &std::path::Path,
    filename: &str,
    lines: &[String],
    mirror_terminal: bool,
) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }
    let dir = data_dir.join("logs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(filename);
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    for line in lines {
        if mirror_terminal {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let scope = v.get("scope").and_then(|s| s.as_str()).unwrap_or("pet");
                let level = v.get("level").and_then(|s| s.as_str()).unwrap_or("info");
                let mut msg = v.get("message").and_then(|s| s.as_str()).unwrap_or(line).to_string();
                if matches!(level, "warn" | "error") {
                    if let Some(detail) = v.get("detail") {
                        if !detail.is_null() {
                            msg.push(' ');
                            msg.push_str(&detail.to_string());
                        }
                    }
                }
                let formatted = format!("[pet][{scope}] {msg}");
                if matches!(level, "warn" | "error") {
                    crate::log::warn(formatted);
                } else {
                    crate::log::info(formatted);
                }
            } else {
                crate::log::info(format!("[pet] {line}"));
            }
        }
        writeln!(file, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_remark_respects_chars() {
        let s = "一二三四五六七八九十";
        assert_eq!(trim_remark(s, 5), "一二三四五…");
    }

    #[test]
    fn point_in_rect_works() {
        assert!(point_in_rect(5, 5, 0, 0, 10, 10));
        assert!(!point_in_rect(10, 5, 0, 0, 10, 10));
        assert!(!point_in_rect(-1, 5, 0, 0, 10, 10));
    }

    #[test]
    fn machine_text_detected() {
        assert!(is_machine_text("开发：foo · 窗口「bar」"));
        assert!(!is_machine_text("在 Cursor 里改桌宠计划"));
    }
}
