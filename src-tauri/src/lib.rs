//! 小寒桌宠 — 库根模块

pub mod character;
pub mod db;
pub mod ipc;
pub mod live2d;
pub mod live2d_import;
pub mod log;
pub mod manifest_lock;
pub mod persona;
pub mod persona_builder;
pub mod pet;
pub mod prompts;
pub mod state;
pub mod system;
pub mod tracker;
#[cfg(debug_assertions)]
pub mod test_api;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};
#[cfg(not(windows))]
use tauri::{PhysicalPosition, PhysicalSize, Position, Size};

/// 防止 `Resized` ↔ `unmaximize` 递归触发（非 Windows 最大化适配）。
#[cfg(not(windows))]
static MAIN_WINDOW_FITTING: AtomicBool = AtomicBool::new(false);
/// 托盘/命令行主动退出时为 true，避免销毁窗口时误触发 prevent_exit
pub(crate) static APP_EXITING: AtomicBool = AtomicBool::new(false);

/// debug 构建下 Ctrl+C 优雅退出：dev 控制台有附加终端，需拦截默认强杀。
#[cfg(debug_assertions)]
static DEV_SHUTDOWN_APP: OnceLock<AppHandle> = OnceLock::new();

#[cfg(debug_assertions)]
fn install_dev_console_shutdown_hook(app: &AppHandle) {
    let _ = DEV_SHUTDOWN_APP.set(app.clone());
    let _ = ctrlc::set_handler(dev_console_shutdown);
}

#[cfg(debug_assertions)]
fn dev_console_shutdown() {
    let Some(app) = DEV_SHUTDOWN_APP.get() else {
        std::process::exit(0);
    };
    if APP_EXITING.load(Ordering::SeqCst) {
        app.exit(0);
        return;
    }
    // 先 detach 全部 WebView，减轻 WebView2 强杀时的 Chrome_WidgetWin_0 噪声
    crate::pet::finalize_all_webviews_for_exit(app);
    if let Some(st) = app.try_state::<Arc<state::AppState>>() {
        request_app_exit(app, st.inner());
    } else {
        APP_EXITING.store(true, Ordering::SeqCst);
        prepare_app_exit_best_effort(app);
        app.exit(0);
    }
    std::thread::sleep(Duration::from_millis(200));
    for _ in 0..8 {
        crate::pet::pump_ui_messages();
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(not(debug_assertions))]
fn install_dev_console_shutdown_hook(_app: &AppHandle) {}

/// 统一退出入口（托盘菜单、IPC）
pub fn request_app_exit(app: &AppHandle, st: &Arc<state::AppState>) {
    if APP_EXITING.swap(true, Ordering::SeqCst) {
        app.exit(0);
        return;
    }
    prepare_app_exit(app, st);
    app.exit(0);
}

/// 将主窗口铺满当前显示器工作区（排除任务栏），不使用系统 maximize。
#[cfg(not(windows))]
fn fit_main_window_to_work_area(win: &WebviewWindow) -> bool {
    if MAIN_WINDOW_FITTING.swap(true, Ordering::SeqCst) {
        return false;
    }
    let ok = {
        #[cfg(windows)]
        {
            crate::system::win32_work_area::fit_window_to_work_area(win)
        }
        #[cfg(not(windows))]
        {
            let _ = win.unmaximize();
            win.current_monitor()
                .ok()
                .flatten()
                .map(|monitor| {
                    let area = monitor.work_area();
                    let pos_ok = win
                        .set_position(Position::Physical(PhysicalPosition::new(
                            area.position.x,
                            area.position.y,
                        )))
                        .is_ok();
                    let size_ok = win
                        .set_size(Size::Physical(PhysicalSize::new(
                            area.size.width,
                            area.size.height,
                        )))
                        .is_ok();
                    pos_ok && size_ok
                })
                .unwrap_or(false)
        }
    };
    MAIN_WINDOW_FITTING.store(false, Ordering::SeqCst);
    ok
}

/// 用户点标题栏最大化时，改为工作区铺满，避免底部被任务栏遮挡。
fn handle_main_window_maximize(win: &WebviewWindow) {
    #[cfg(windows)]
    {
        crate::system::win32_work_area::correct_if_zoomed(win);
    }
    #[cfg(not(windows))]
    if win.is_maximized().unwrap_or(false) {
        let _ = fit_main_window_to_work_area(win);
    }
}

/// 开局以默认尺寸居中显示，不铺满工作区；仅用户点最大化时才适配工作区。
fn prepare_main_window_for_first_show(win: &WebviewWindow) {
    let _ = win.unmaximize();
    #[cfg(windows)]
    {
        let win = win.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            crate::system::win32_work_area::restore_default_if_zoomed(&win);
        });
    }
}

fn install_main_window_maximize_hook(win: &WebviewWindow) {
    #[cfg(windows)]
    {
        match crate::system::win32_work_area::install_maximize_work_area_hook(win) {
            Ok(()) => {}
            Err(e) => {
                crate::log::warn(format!("工作区最大化钩子安装失败: {e}"));
                let win_for_hook = win.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(300));
                    if let Err(e2) =
                        crate::system::win32_work_area::install_maximize_work_area_hook(
                            &win_for_hook,
                        )
                    {
                        crate::log::warn(format!("工作区最大化钩子重试失败: {e2}"));
                    }
                });
            }
        }
    }
}

static MAIN_WINDOW_ENSURED: AtomicBool = AtomicBool::new(false);

/// 首次打开主窗口时再创建 WebView，启动阶段仅加载桌宠模型。
pub fn ensure_main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(win) = app.get_webview_window("main") {
        return Ok(win);
    }
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("小寒桌宠")
        .inner_size(960.0, 640.0)
        .min_inner_size(720.0, 480.0)
        .center()
        .visible(false)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    if MAIN_WINDOW_ENSURED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        install_main_window_maximize_hook(&win);
        prepare_main_window_for_first_show(&win);
    }
    Ok(win)
}

/// 应用启动入口：构建窗口、托盘、注册状态与 command、启动后台采样线程、处理退出 flush。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // 1. 初始化共享状态（DB 连接 + 聚合缓存 + 停机标志）
            let app_state = state::AppState::new(app.handle())?;
            app.manage(app_state.clone());

            if let Ok(db) = crate::db::lock_conn(&app_state.db) {
                crate::system::autostart::sync_on_startup(app.handle(), &db);
            }

            // 2. 后台线程：[live2d-only] 仅 poller 更新 foreground/idle 供桌宠
            let stop_handle = tracker::poller::spawn_poller(app_state.clone());
            let noop = |name: &str| {
                std::thread::Builder::new()
                    .name(name.into())
                    .spawn(|| {})
                    .expect("noop thread")
            };
            let input_handle = noop("input-mon-idle");
            let file_handle = noop("file-watch-idle");
            let audio_handle = noop("audio-mon-idle");

            // 3. 构建托盘
            tray::build_tray(app)?;

            app.manage(crate::pet::PetRuntimeState::default());

            // 人物 manifest 同步不阻塞首屏（AppState::new 已 seed）
            {
                let st_bg = app_state.clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(db) = st_bg.lock_db() {
                        let data_dir = st_bg.data_dir().to_path_buf();
                        if let Err(e) = crate::character::migrate_on_startup(&data_dir, &db) {
                            crate::log::warn(format!("人物 manifest 启动迁移失败: {e}"));
                        }
                    }
                });
            }

            // 4. 桌宠：预热 WebView，再异步显示
            if let Err(e) = crate::pet::prewarm_on_startup(app.handle(), app_state.clone()) {
                crate::log::warn(format!("桌宠预热失败: {e}"));
            }
            if let Err(e) = crate::pet::sync_on_startup(app.handle(), app_state.clone()) {
                crate::log::warn(format!("桌宠启动失败: {e}"));
            }

            // 6. 人物头像：启动后后台分批下载到本地，人物页只读缓存
            crate::character::avatar::spawn_sync_on_startup(app.handle().clone(), app_state.clone());

            // 7. Live2D：按 plan.json 后台批量导入缺失模型
            crate::live2d_import::spawn_batch_on_startup(app_state.clone());

            // 5. 存 join handle 供退出时 join
            app.manage(state::JoinState::new(stop_handle, input_handle, file_handle, audio_handle));

            install_dev_console_shutdown_hook(app.handle());

            #[cfg(debug_assertions)]
            if std::env::var("HANDAILY_DISABLE_TEST_API").is_err() {
                test_api::spawn_server(app.handle().clone(), app_state.clone());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // [live2d-only] 纯桌宠 IPC 子集
            ipc::live2d_commands::app_ping,
            ipc::live2d_commands::app_memory_stats,
            ipc::live2d_commands::app_exit,
            ipc::live2d_commands::app_get_data_path,
            ipc::live2d_commands::settings_get,
            ipc::live2d_commands::settings_save,
            ipc::live2d_commands::autostart_get_status,
            ipc::live2d_commands::autostart_set_enabled,
            ipc::live2d_commands::persona_list,
            ipc::live2d_commands::persona_set_active,
            ipc::live2d_commands::persona_get_detail,
            ipc::live2d_commands::persona_import,
            ipc::live2d_commands::persona_import_text,
            ipc::live2d_commands::persona_import_wiki,
            ipc::live2d_commands::persona_import_blhx_local,
            ipc::live2d_commands::persona_update,
            ipc::live2d_commands::persona_delete,
            ipc::live2d_commands::characters_list,
            ipc::live2d_commands::characters_list_brief,
            ipc::live2d_commands::characters_list_page,
            ipc::live2d_commands::characters_pet_menu_skins,
            ipc::live2d_commands::characters_pet_menu_skins_for,
            ipc::live2d_commands::characters_pet_menu_favorites,
            ipc::live2d_commands::characters_remove_skin,
            ipc::live2d_commands::characters_import_avatars_batch,
            ipc::live2d_commands::characters_cache_avatar,
            ipc::live2d_commands::characters_cache_avatars_batch,
            ipc::live2d_commands::characters_read_avatar,
            ipc::live2d_commands::characters_skins_page,
            ipc::live2d_commands::characters_import_live2d,
            ipc::live2d_commands::live2d_import_batch,
            ipc::live2d_commands::characters_get_detail,
            ipc::live2d_commands::characters_set_active,
            ipc::live2d_commands::characters_set_skin,
            ipc::live2d_commands::character_import_wiki,
            ipc::live2d_commands::app_get_personas_path,
            ipc::live2d_commands::pet_get_wiki_bulk_import_progress,
            ipc::live2d_commands::pet_start_wiki_bulk_import,
            ipc::live2d_commands::pet_pause_wiki_bulk_import,
            ipc::live2d_commands::pet_resume_wiki_bulk_import,
            ipc::live2d_commands::pet_stop_wiki_bulk_import,
            ipc::live2d_commands::pet_get_status,
            ipc::live2d_commands::pet_show,
            ipc::live2d_commands::pet_hide,
            ipc::live2d_commands::pet_set_enabled,
            ipc::live2d_commands::pet_save_position,
            ipc::live2d_commands::pet_get_screen_bounds,
            ipc::live2d_commands::pet_open_main,
            ipc::live2d_commands::pet_menu_show,
            ipc::live2d_commands::pet_menu_open_at_cursor,
            ipc::live2d_commands::pet_menu_toggle_at_cursor,
            ipc::live2d_commands::pet_is_right_mouse_down,
            ipc::live2d_commands::pet_is_left_mouse_down,
            ipc::live2d_commands::pet_poll_menu_dismiss,
            ipc::live2d_commands::pet_menu_contains_cursor,
            ipc::live2d_commands::pet_menu_hide,
            ipc::live2d_commands::pet_menu_sync_z_order,
            ipc::live2d_commands::pet_menu_toggle,
            ipc::live2d_commands::pet_enter_edit_bounds,
            ipc::live2d_commands::pet_reload,
            ipc::live2d_commands::pet_nudge,
            ipc::live2d_commands::pet_menu_switch_skin,
            ipc::live2d_commands::pet_menu_switch_character,
            ipc::live2d_commands::pet_await_spine_ready,
            ipc::live2d_commands::pet_refresh_animations,
            ipc::live2d_commands::pet_preview_animation,
            ipc::live2d_commands::pet_mark_spine_ready,
            ipc::live2d_commands::pet_confirm_switch,
            ipc::live2d_commands::pet_clear_spine_ready,
            ipc::live2d_commands::pet_get_bubble_enabled,
            ipc::live2d_commands::pet_set_bubble_enabled,
            ipc::live2d_commands::pet_get_model_status,
            ipc::live2d_commands::pet_get_config,
            ipc::live2d_commands::pet_resolve_model_preload_config,
            ipc::live2d_commands::pet_list_models,
            ipc::live2d_commands::pet_set_model,
            ipc::live2d_commands::pet_save_model_settings,
            ipc::live2d_commands::pet_set_scale,
            ipc::live2d_commands::pet_import_from_folder,
            ipc::live2d_commands::pet_import_files,
            ipc::live2d_commands::pet_pick_model_folder,
            ipc::live2d_commands::pet_stage_folder_import,
            ipc::live2d_commands::pet_stage_files_import,
            ipc::live2d_commands::pet_get_import_staging,
            ipc::live2d_commands::pet_clear_import_staging,
            ipc::live2d_commands::pet_commit_import,
            ipc::live2d_commands::pet_read_model_asset,
            ipc::live2d_commands::pet_read_model_bundle,
            ipc::live2d_commands::pet_delete_model,
            ipc::live2d_commands::pet_sync_animations,
            ipc::live2d_commands::pet_set_idle_animation,
            ipc::live2d_commands::pet_set_click_animation,
            ipc::live2d_commands::pet_set_random_animations,
            ipc::live2d_commands::pet_save_animation_layout,
            ipc::live2d_commands::pet_import_lines,
            ipc::live2d_commands::pet_wiki_import_lines,
            ipc::live2d_commands::pet_save_window_size,
            ipc::live2d_commands::pet_get_window_bounds,
            ipc::live2d_commands::pet_set_window_bounds,
            ipc::live2d_commands::pet_save_layout,
            ipc::live2d_commands::pet_append_movement_logs,
            ipc::live2d_commands::pet_append_display_logs,
            ipc::live2d_commands::system_get_performance,
        ])
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                // 关闭=最小化到托盘：阻止销毁，改为隐藏
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.emit("main-window-visible", false);
                    crate::pet::restore_pet_topmost_if_visible(window.app_handle());
                }
                WindowEvent::Resized(_) => {
                    if let Some(win) = window.app_handle().get_webview_window("main") {
                        handle_main_window_maximize(&win);
                    }
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    if let Some(win) = window.app_handle().get_webview_window("main") {
                        handle_main_window_maximize(&win);
                    }
                }
                WindowEvent::Focused(false) => {
                    crate::pet::restore_pet_topmost_if_visible(window.app_handle());
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building 小寒桌宠")
        .run(handle_run_event);
}

/// 托盘构建（拆到独立模块避免 main.rs 膨胀）
pub mod tray;

/// RunEvent 处理：退出时设停机标志 + join 后台线程 + flush
static EXIT_PREPARED: AtomicBool = AtomicBool::new(false);

pub(crate) fn prepare_app_exit_impl(app: &tauri::AppHandle, st: Option<&Arc<state::AppState>>) {
    if EXIT_PREPARED.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(st) = st {
        st.stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    if let Some(rt) = app.try_state::<crate::pet::PetRuntimeState>() {
        rt.wiki_bulk_import_running
            .store(false, Ordering::SeqCst);
    }
    if let Some(win) = app.get_webview_window("main") {
        #[cfg(windows)]
        crate::system::win32_work_area::uninstall_maximize_work_area_hook(&win);
    }
    crate::pet::hide_pet_windows_immediately(app);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    if let Some(st) = st {
        crate::tracker::writer::flush_on_exit(st);
    }
}

pub(crate) fn prepare_app_exit(app: &tauri::AppHandle, st: &Arc<state::AppState>) {
    prepare_app_exit_impl(app, Some(st));
}

pub(crate) fn prepare_app_exit_best_effort(app: &tauri::AppHandle) {
    let st = app.try_state::<Arc<state::AppState>>();
    prepare_app_exit_impl(app, st.as_ref().map(|s| s.inner()));
}

fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    use tauri::RunEvent;
    match event {
        RunEvent::ExitRequested { api, code, .. } => {
            // 用户关主窗口触发的退出（code=None）→ 托盘常驻
            if code.is_none() && !APP_EXITING.load(Ordering::SeqCst) {
                api.prevent_exit();
                return;
            }
            if !APP_EXITING.load(Ordering::SeqCst) {
                APP_EXITING.store(true, Ordering::SeqCst);
            }
            if let Some(st) = app.try_state::<Arc<state::AppState>>() {
                prepare_app_exit(app, &st);
            } else {
                prepare_app_exit_best_effort(app);
            }
        }
        RunEvent::Exit => {
            if !APP_EXITING.load(Ordering::SeqCst) {
                APP_EXITING.store(true, Ordering::SeqCst);
            }
            if !EXIT_PREPARED.load(Ordering::SeqCst) {
                prepare_app_exit_best_effort(app);
            }
            crate::pet::finalize_all_webviews_for_exit(app);
            if let Some(st) = app.try_state::<Arc<state::AppState>>() {
                st.finalize_shutdown();
            }
            if let Some(st) = app.try_state::<Arc<state::AppState>>() {
                st.join_ai_workers();
            }
            // join 后台线程（后台线程在 stop_flag=true 时会跳出循环并 flush_on_exit）
            if let Some(js) = app.try_state::<state::JoinState>() {
                js.join_all();
            }
        }
        _ => {}
    }
}
