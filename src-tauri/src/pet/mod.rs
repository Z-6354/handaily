//! 桌宠：透明置顶窗口、位置持久化、气泡调度

pub mod lines_import;
pub mod models;
pub mod wiki_scrape;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::webview::{Color, PageLoadEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use crate::state::AppState;

pub const PET_LABEL: &str = "pet";

const DEFAULT_WIDTH: f64 = 240.0;
const DEFAULT_HEIGHT: f64 = 320.0;

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

fn model_scale(db: &rusqlite::Connection, meta: &models::PetAnimationMeta) -> f64 {
    meta.scale
        .map(|s| s.clamp(0.4, 1.5))
        .unwrap_or_else(|| get_scale(db))
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
) -> Result<(), String> {
    save_window_size(db, width, height)?;
    let s = scale.clamp(0.4, 1.5);
    crate::db::set_setting(db, "pet_scale", &s.to_string()).map_err(|e| e.to_string())?;
    crate::db::set_setting(db, "pet_offset_x", &offset_x.round().to_string())
        .map_err(|e| e.to_string())?;
    crate::db::set_setting(db, "pet_offset_y", &offset_y.round().to_string())
        .map_err(|e| e.to_string())?;
    let pos = load_position(db);
    let (w, h) = get_window_size(db);
    let (x, y) = clamp_pet_position(pos.0, pos.1, w as i32, h as i32);
    save_position(db, x, y)?;
    Ok(())
}

fn pet_webview_url() -> WebviewUrl {
    // 必须 App 协议：External(localhost) 为 remote origin，pet-capability 仅授权 local，
    // invoke 被拒 → 透明空窗（见 docs/questions/62、67）
    WebviewUrl::App("pet.html".into())
}

#[cfg(debug_assertions)]
async fn wait_frontend_ready(max_secs: u64) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
        .ok();
    let deadline = Instant::now() + Duration::from_secs(max_secs);
    while Instant::now() < deadline {
        let ok = if let Some(c) = &client {
            c.get(DEV_PET_PAGE)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        } else {
            tokio::net::TcpStream::connect("127.0.0.1:1420")
                .await
                .is_ok()
        };
        if ok {
            return;
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    crate::log::info("桌宠: 等待 Vite dev server 超时，仍将尝试显示");
}

fn prepare_pet_webview(win: &tauri::WebviewWindow) -> Result<(), String> {
    let _ = win.set_background_color(Some(Color(0, 0, 0, 0)));
    let _ = win.set_always_on_top(true);
    Ok(())
}

pub fn pet_visible(app: &AppHandle) -> bool {
    app.get_webview_window(PET_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// 显示主窗口；桌宠 `always_on_top` 会挡住主窗口，需先让桌宠降层。
pub fn show_main_window(app: &AppHandle, page: Option<&str>) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    if let Some(pet) = app.get_webview_window(PET_LABEL) {
        let _ = pet.set_always_on_top(false);
    }
    let _ = win.unminimize();
    win.show().map_err(|e| e.to_string())?;
    win.set_focus().map_err(|e| e.to_string())?;
    let _ = win.emit("main-window-visible", true);
    if let Some(page) = page.filter(|p| !p.is_empty()) {
        let _ = app.emit_to("main", "main-navigate", page.to_string());
    }
    Ok(())
}

/// 主窗口失焦/隐藏后恢复桌宠置顶（若仍可见）。
pub fn restore_pet_topmost_if_visible(app: &AppHandle) {
    if let Some(pet) = app.get_webview_window(PET_LABEL) {
        if pet.is_visible().unwrap_or(false) {
            let _ = pet.set_always_on_top(true);
        }
    }
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

pub fn get_config(st: &AppState) -> Result<PetConfigPayload, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let model_id = models::active_model_id(&db);
    let assets = models::resolve_assets(st.data_dir(), &model_id)?;
   let anim_meta = models::read_animation_meta(st.data_dir(), &db, &model_id);
   let (window_width, window_height) = get_window_size(&db);
    let (offset_x, offset_y) = get_model_offset(&db);
    Ok(PetConfigPayload {
        model_id: assets.model_id,
        model_name: assets.model_name,
        asset_base: assets.asset_base,
        config_file: assets.config_file,
        skel_file: assets.skel_file,
        atlas_file: assets.atlas_file,
        png_file: assets.png_file,
        use_file_src: assets.use_file_src,
        power_mode: model_power_mode(&db, &anim_meta),
        scale: model_scale(&db, &anim_meta),
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
        bubble_enabled: is_bubble_enabled(&db),
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

pub fn save_position(db: &rusqlite::Connection, x: i32, y: i32) -> Result<PetPoint, String> {
    let (w, h) = get_window_size(db);
    let (x, y) = clamp_pet_position(x, y, w as i32, h as i32);
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

pub fn destroy_pet_window(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        win.close().map_err(|e| e.to_string())?;
        wait_pet_window_closed(app, Duration::from_millis(2000))?;
    }
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.position_guard_attached
            .store(false, Ordering::SeqCst);
        rt.page_load_finished.store(false, Ordering::Release);
        rt.page_load_count.store(0, Ordering::Release);
        rt.spine_ready.store(false, Ordering::Release);
    }
    Ok(())
}

fn wait_pet_window_closed(app: &AppHandle, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while app.get_webview_window(PET_LABEL).is_some() {
        if Instant::now() >= deadline {
            return Err("桌宠窗口关闭超时，请稍后重试".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

pub fn show_pet(app: &AppHandle, st: &Arc<AppState>) -> Result<(), String> {
    ensure_pet_window(app, st)?;
    let win = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口创建失败".to_string())?;

    prepare_pet_webview(&win)?;

    let (x, y, w, h) = {
        let db = crate::db::lock_conn(&st.db)?;
        let pos = load_position(&db);
        let (pw, ph) = get_window_size(&db);
        let win_w = pw as i32;
        let win_h = ph as i32;
        let (x, y) = resolve_pet_position(pos.0, pos.1, win_w, win_h);
        if (x, y) != pos {
            let _ = save_position(&db, x, y);
            crate::log::info(format!(
                "桌宠: 位置 ({},{}) 在屏幕外，已重置为 ({},{})",
                pos.0, pos.1, x, y
            ));
        }
        (x, y, pw, ph)
    };

    let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(w, h)));
    let rt = app.state::<PetRuntimeState>();
    attach_position_guard(app, &win);
    set_pet_position(&win, &rt, x, y)?;
    let _ = win.set_always_on_top(true);
    win.show().map_err(|e| e.to_string())?;
    companion_session_start(st);
    if should_full_reload_pet(app) {
        schedule_pet_reload_after_show(app);
    } else {
        schedule_pet_resume_after_show(app);
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

pub fn mark_spine_ready(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.spine_ready.store(true, Ordering::Release);
    }
}

pub fn clear_spine_ready(app: &AppHandle) {
    if let Some(rt) = app.try_state::<PetRuntimeState>() {
        rt.spine_ready.store(false, Ordering::Release);
    }
}

pub fn resume_pet(app: &AppHandle) {
    let _ = app.emit_to(PET_LABEL, "pet-resume", ());
}

fn schedule_pet_reload_after_show(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if !wait_pet_page_ready(&app, Duration::from_secs(20)).await {
            crate::log::info("桌宠: pet.html 加载超时，仍尝试初始化");
        }
        // 可见后多等几帧，避免登录自启时 WebView 合成未就绪即组装 Spine
        tokio::time::sleep(Duration::from_millis(180)).await;
        if app
            .get_webview_window(PET_LABEL)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
        {
            nudge_pet(&app);
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

pub fn hide_pet(app: &AppHandle, destroy: bool) -> Result<(), String> {
    if let Some(st) = app.try_state::<Arc<AppState>>() {
        companion_session_stop(st.inner());
    }
    if destroy {
        destroy_pet_window(app)
    } else if let Some(win) = app.get_webview_window(PET_LABEL) {
        win.hide().map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

pub fn reload_pet(app: &AppHandle, st: &Arc<AppState>) -> Result<(), String> {
    if app.get_webview_window(PET_LABEL).is_some() {
        nudge_pet(app);
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

/// 通知桌宠前端重新读配置（不销毁窗口）
pub fn nudge_pet(app: &AppHandle) {
    clear_spine_ready(app);
    let _ = app.emit_to(PET_LABEL, "pet-reload", ());
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

/// 设置页点击动作名时，在桌宠窗口演示播放
pub fn preview_animation(
    app: &AppHandle,
    st: &AppState,
    animation: &str,
    loop_anim: bool,
) -> Result<(), String> {
    let name = animation.trim();
    if name.is_empty() {
        return Err("动作名不能为空".into());
    }
    ensure_pet_window(app, st)?;
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
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

/// 切换模型时重建桌宠 WebView，与「关闭桌宠再启用」同路径，避免同 canvas 热重载碎块。
fn restart_pet_window(app: &AppHandle, st: &Arc<AppState>) -> Result<(), String> {
    let had_window = app.get_webview_window(PET_LABEL).is_some();
    let was_visible = pet_visible(app);
    if had_window {
        companion_session_stop(st);
        destroy_pet_window(app)?;
    }
    if was_visible || !had_window {
        show_pet(app, st)
    } else {
        ensure_pet_window(app, st)
    }
}

pub fn set_active_model(app: &AppHandle, st: Arc<AppState>, model_id: &str) -> Result<(), String> {
    let data_dir = st.data_dir();
    let assets = models::resolve_assets(data_dir, model_id)?;
    // 切换前批量读入 OS 页缓存，缩短重建 WebView 后的资源加载
    if assets.use_file_src {
        let mut files = vec![
            assets.skel_file.clone(),
            assets.atlas_file.clone(),
            assets.png_file.clone(),
        ];
        if let Some(ref cfg) = assets.config_file {
            files.push(cfg.clone());
        }
        let _ = models::read_model_asset_bundle(data_dir, model_id, &files);
    }
    let db = crate::db::lock_conn(&st.db)?;
    models::set_active_model_id(&db, &assets.model_id)?;
    let enabled = is_enabled(&db);
    drop(db);
    if enabled {
        restart_pet_window(app, &st)?;
    }
    Ok(())
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
        wait_frontend_ready(45).await;

        let boot_delay = if crate::system::autostart::is_tray_launch() {
            Duration::from_millis(1200)
        } else {
            Duration::from_millis(400)
        };
        tokio::time::sleep(boot_delay).await;
        for attempt in 0..30 {
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
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
            if pet_visible(&app2) {
                ensure_remark_scheduler(app2.clone(), st2.clone());
                return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            if attempt == 29 {
                crate::log::warn("桌宠启动失败: 窗口已创建但多次尝试后仍不可见");
            }
        }
    });

    Ok(())
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
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                };
                is_enabled(&db)
            };

            if !enabled {
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }

            sync_fullscreen_visibility(&app, &st);

            let interval_sec = {
                let Ok(db) = st.lock_db() else {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                };
                let model_id = models::active_model_id(&db);
                let meta = models::read_animation_meta(st.data_dir(), &db, &model_id);
                model_remark_interval_sec(&db, &meta)
            };

            let bubble_on = {
                let Ok(db) = st.lock_db() else {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                };
                is_bubble_enabled(&db)
            };

            if interval_sec == 0 || !pet_visible(&app) || !bubble_on {
                tokio::time::sleep(Duration::from_secs(5)).await;
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
                let remark = if is_text_ai_ready_for(&st) {
                    build_remark_random_ai(&app, &st).await
                } else {
                    build_remark_from_lines(&st)
                };
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
            tokio::time::sleep(Duration::from_secs(sleep_sec)).await;
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
            let _ = hide_pet(app, false);
            rt.fullscreen_suppressed.store(true, Ordering::Relaxed);
        }
    } else if rt.fullscreen_suppressed.swap(false, Ordering::Relaxed) {
        let enabled = st
            .db
            .lock()
            .ok()
            .map(|db| is_enabled(&db))
            .unwrap_or(false);
        if enabled {
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
            if rt.fullscreen_suppressed.load(Ordering::Relaxed) {
                rt.fullscreen_suppressed.store(false, Ordering::Relaxed);
                let _ = show_pet(app, st);
            }
        }
    }
    let _ = ensure_pet_window(app, st);

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

async fn build_remark_random_ai(app: &AppHandle, st: &AppState) -> Option<PetRemarkPayload> {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u32)
        .unwrap_or(0);
    let start = (seed % 3) as usize;
    for i in 0..3 {
        let src = (start + i) % 3;
        let remark = match src {
            0 => build_remark_with_ai(app, st).await,
            1 => build_remark_from_timeline_random(st),
            _ => build_remark_from_lines(st),
        };
        if remark.is_some() {
            return remark;
        }
    }
    None
}

fn is_text_ai_ready_for(st: &AppState) -> bool {
    let Ok(db) = st.lock_db() else {
        return false;
    };
    let data_dir = st.data_dir();
    let config = crate::ai::AiConfig::load(&db, data_dir);
    let catalog = crate::ai::load_catalog(data_dir);
    crate::ai::is_text_ai_ready(&config, &catalog, &st.vault, &db)
}

fn build_remark_from_timeline_random(st: &AppState) -> Option<PetRemarkPayload> {
    let db = st.lock_db().ok()?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stmt = db
        .prepare(
            "SELECT summary FROM timeline_ai_cache \
             WHERE substr(started_at, 1, 10) = ?1 AND trim(summary) != '' \
             ORDER BY started_at DESC LIMIT 48",
        )
        .ok()?;
    let summaries: Vec<String> = stmt
        .query_map([&today], |r| r.get(0))
        .ok()?
        .filter_map(|r| r.ok())
        .filter(|s: &String| !s.trim().is_empty() && !is_machine_text(s))
        .collect();
    if summaries.is_empty() {
        return None;
    }
    let idx = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_nanos() as usize) % summaries.len())
        .unwrap_or(0);
    Some(PetRemarkPayload {
        text: trim_remark(summaries[idx].trim(), 80),
        source: "timeline".into(),
        animation: None,
    })
}

async fn build_remark_with_ai(_app: &AppHandle, st: &AppState) -> Option<PetRemarkPayload> {
    let data_dir = st.data_dir();
    let (anim_meta, prep_result) = {
        let db = st.lock_db().ok()?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        let model_id = models::active_model_id(&db);
        let anim_meta = models::read_animation_meta(data_dir, &db, &model_id);
        let fg = st.foreground.lock().ok().and_then(|g| g.clone());
        let prompt = build_ai_remark_prompt(data_dir, &fg, &anim_meta);
        let prep_result = crate::ai::PreparedTextChat::prepare(
            &config,
            &catalog,
            &st.vault,
            &db,
            data_dir,
            prompt,
        );
        (anim_meta, prep_result)
    };

    let prep = match prep_result {
        Ok(Some(p)) => p,
        _ => return None,
    };

    let raw = prep.run_async().await.ok()?;
    parse_ai_remark(&raw, &anim_meta)
}

fn build_ai_remark_prompt(
    data_dir: &Path,
    fg: &Option<crate::tracker::ForegroundPayload>,
    anim_meta: &models::PetAnimationMeta,
) -> String {
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
    let idle = anim_meta.idle_animation.as_deref().unwrap_or("（未设置）");
    let animations = if anim_meta.animations.is_empty() {
        "（暂无）".to_string()
    } else {
        anim_meta.animations.join("、")
    };
    let idle_flag = if fg.as_ref().map(|f| f.is_idle).unwrap_or(false) {
        "是"
    } else {
        "否"
    };
    crate::prompts::render(
        data_dir,
        "pet-remark",
        &[
            ("app_name", &app_name),
            ("is_idle", idle_flag),
            ("idle_animation", idle),
            ("animation_list", &animations),
        ],
    )
}

fn parse_ai_remark(raw: &str, anim_meta: &models::PetAnimationMeta) -> Option<PetRemarkPayload> {
    let json_str = crate::ai::json_util::extract_json_object(raw);
    let v: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    let text = v.get("text").and_then(|t| t.as_str())?.trim();
    if text.is_empty() || is_machine_text(text) {
        return None;
    }
    let animation = v
        .get("animation")
        .and_then(|a| a.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|s| {
            anim_meta.animations.is_empty() || anim_meta.animations.iter().any(|n| n == s)
        });
    Some(PetRemarkPayload {
        text: trim_remark(text, 80),
        source: "ai".into(),
        animation,
    })
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

pub async fn ai_suggest_lines(
    st: &AppState,
    model_id: &str,
    count: usize,
) -> Result<Vec<models::PetRemarkLine>, String> {
    let data_dir = st.data_dir();
    let (anim_meta, prep_result) = {
        let db = st.lock_db().map_err(|e| e.to_string())?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        let anim_meta = models::read_animation_meta(data_dir, &db, model_id);
        let animations = if anim_meta.animations.is_empty() {
            "（暂无）".to_string()
        } else {
            anim_meta.animations.join("、")
        };
        let prompt = format!(
            "你是桌宠台词助手。请为模型「{model_id}」生成 {count} 条中文短台词（每条 8~36 字，可爱口语）。\n\
             可选绑定动作：{animations}\n\
             仅输出 JSON 数组，每项 {{\"text\":\"...\",\"animation\":null或动作名}}。不要 markdown。"
        );
        let prep_result = crate::ai::PreparedTextChat::prepare(
            &config,
            &catalog,
            &st.vault,
            &db,
            data_dir,
            prompt,
        );
        (anim_meta, prep_result)
    };
    let prep = match prep_result {
        Ok(Some(p)) => p,
        Ok(None) => {
            return Err("未配置 AI 或密钥不可用".into());
        }
        Err(e) => return Err(e),
    };
    let raw = prep.run_async().await.map_err(|e| e.to_string())?;
    let json_str = crate::ai::json_util::extract_json_array(&raw);
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).or_else(|_| serde_json::from_str(&raw)).map_err(|e| e.to_string())?;
    let arr = parsed
        .as_array()
        .cloned()
        .or_else(|| parsed.get("lines").and_then(|v| v.as_array()).cloned())
        .ok_or_else(|| "AI 返回格式无效".to_string())?;
    let has = |name: &str| {
        anim_meta.animations.is_empty() || anim_meta.animations.iter().any(|n| n == name)
    };
    let lines: Vec<models::PetRemarkLine> = arr
        .iter()
        .filter_map(|item| {
            let text = item.get("text")?.as_str()?.trim();
            if text.is_empty() {
                return None;
            }
            let animation = item
                .get("animation")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && has(s));
            Some(models::PetRemarkLine {
                text: text.to_string(),
                animation,
            })
        })
        .collect();
    if lines.is_empty() {
        return Err("AI 未返回可用台词".into());
    }
    Ok(lines)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_remark_respects_chars() {
        let s = "一二三四五六七八九十";
        assert_eq!(trim_remark(s, 5), "一二三四五…");
    }

    #[test]
    fn machine_text_detected() {
        assert!(is_machine_text("开发：foo · 窗口「bar」"));
        assert!(!is_machine_text("在 Cursor 里改桌宠计划"));
    }
}
