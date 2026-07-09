//! 小寒日报 — 库根模块
//!
//! ## 架构：单核心 + 双 UI 外壳
//!
//! ```text
//!  [Rust 核心]  AppState + tracker 线程 + analysis + SQLite
//!        ↑ IPC（任意窗口/托盘均可调用）
//!   ┌────┴────┐
//!  main 窗口   pet 窗口（可选）
//!  （报表 UI）  （桌宠 UI）
//!        ↑
//!   系统托盘 — 进程常驻；仅「退出」才 stop_flag + flush
//! ```
//!
//! - **采集与落库**在 `setup()` 里启动，不依赖 main/pet 是否打开
//! - 关闭 main 仅 `hide()`，不销毁、不停止采集
//! - 关闭/隐藏桌宠不影响采集；只有托盘「退出」才结束进程
//!
//! 职责划分：
//! - `tracker` — Win32 前台窗口采集、idle 检测、采样循环、segment 合并/写入
//! - `db` — rusqlite 连接 + migrate + 聚合查询
//! - `ipc` — `#[tauri::command]` 处理器
//! - `state` — 跨线程共享的 `AppState`（DB + 聚合缓存 + 停机标志）

pub mod agent_http;
pub mod manifest_lock;
pub mod analysis;
pub mod ai;
pub mod db;
pub mod ipc;
pub mod live2d_import;
pub mod log;
pub mod character;
pub mod persona;
pub mod persona_builder;
pub mod pet;
pub mod prompts;
pub mod report;
pub mod timeline;
pub mod screenshot;
pub mod state;
pub mod system;
pub mod tracker;
pub mod vault;
pub mod wechat;
pub mod work_type;

use std::sync::Arc;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow, WindowEvent};

/// 主窗口首次显示前在隐藏状态下铺满工作区，避免先露出 960×640 再播放最大化动画。
fn prepare_main_window_for_first_show(win: &WebviewWindow) {
    let _ = win.unmaximize();
    if let Ok(Some(monitor)) = win.current_monitor() {
        let area = monitor.work_area();
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(
            area.position.x,
            area.position.y,
        )));
        let _ = win.set_size(Size::Physical(PhysicalSize::new(
            area.size.width,
            area.size.height,
        )));
    } else {
        let _ = win.maximize();
    }
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

            // 启动后后台任务：不阻塞首屏（时间线 AI 日志合并等）
            {
                let st = app_state.clone();
                std::thread::Builder::new()
                    .name("startup-maint".into())
                    .spawn(move || {
                        crate::tracker::dampen_thread_priority();
                        let data_dir = st.data_dir().to_path_buf();
                        if let Err(e) =
                            crate::timeline::json_log::consolidate_past_days_on_startup(&data_dir)
                        {
                            crate::log::warn(format!(
                                "timeline-ai startup consolidate failed: {e}"
                            ));
                        }
                    })
                    .ok();
            }

            if let Ok(db) = crate::db::lock_conn(&app_state.db) {
                crate::system::autostart::sync_on_startup(app.handle(), &db);
            }

            // 2. 启动后台采样线程
            let stop_handle = tracker::poller::spawn_poller(app_state.clone());
            let input_handle =
                tracker::input_monitor::spawn_input_monitor(app_state.clone());
            let file_handle =
                tracker::file_watcher::spawn_file_watcher(app_state.clone());
            let audio_handle =
                tracker::audio_monitor::spawn_audio_monitor(app_state.clone());

            // 3. 构建托盘
            tray::build_tray(app)?;

            app.manage(crate::pet::PetRuntimeState::default());

            // 4. 桌宠：若已启用则启动显示
            if let Err(e) = crate::pet::sync_on_startup(app.handle(), app_state.clone()) {
                crate::log::warn(format!("桌宠启动失败: {e}"));
            }

            // 5. 时间线 AI：后台自动补全今日未缓存简介（不依赖打开时间线页）
            crate::timeline::scheduler::spawn(app.handle().clone(), app_state.clone());

            // 6. 人物头像：启动后后台分批下载到本地，人物页只读缓存
            crate::character::avatar::spawn_sync_on_startup(app.handle().clone(), app_state.clone());

            crate::agent_http::restore_on_startup(app_state.clone());

            let wechat_rt = crate::wechat::on_startup(app_state.clone());
            app.manage(wechat_rt);

            // 手动启动显示主窗口；--tray 登录自启动仅托盘常驻
            if let Some(win) = app.get_webview_window("main") {
                prepare_main_window_for_first_show(&win);
                if !crate::system::autostart::is_tray_launch() {
                    let _ = win.show();
                    let _ = win.set_focus();
                    let _ = win.emit("main-window-visible", true);
                }
            }

            // 5. 存 join handle 供退出时 join
            app.manage(state::JoinState::new(stop_handle, input_handle, file_handle, audio_handle));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::commands::app_ping,
            ipc::commands::app_memory_stats,
            ipc::commands::app_exit,
            ipc::commands::app_get_data_path,
            ipc::commands::app_get_prompts_path,
            ipc::commands::app_get_vendors_config_path,
            ipc::commands::tracking_get_status,
            ipc::commands::tracking_set_enabled,
            ipc::commands::settings_get,
            ipc::commands::settings_save,
            ipc::commands::autostart_get_status,
            ipc::commands::autostart_set_enabled,
            ipc::commands::stats_today_overview,
            ipc::commands::stats_app_breakdown,
            ipc::commands::stats_hourly_activity,
            ipc::commands::stats_three_day_heatmap,
            ipc::commands::stats_timeline,
            ipc::commands::vault_get_status,
            ipc::commands::vault_setup,
            ipc::commands::vault_unlock,
            ipc::commands::vault_lock,
            ipc::commands::vault_list_entries,
            ipc::commands::vault_add_entry,
            ipc::commands::vault_update_entry,
            ipc::commands::vault_delete_entry,
            ipc::commands::vault_get_secret,
            ipc::commands::analysis_get_status,
            ipc::commands::analysis_list_insights,
            ipc::commands::stats_today_metrics,
            ipc::commands::ai_get_config,
            ipc::commands::ai_is_text_ready,
            ipc::commands::ai_save_config,
            ipc::commands::ai_list_models,
            ipc::commands::ai_import_models,
            ipc::commands::ai_test_vendor,
            ipc::commands::ai_add_custom_model,
            ipc::commands::persona_list,
            ipc::commands::persona_set_active,
            ipc::commands::persona_get_detail,
            ipc::commands::persona_import,
            ipc::commands::persona_import_text,
            ipc::commands::persona_import_wiki,
            ipc::commands::persona_import_blhx_local,
            ipc::commands::persona_regenerate_profile,
            ipc::commands::persona_batch_regenerate_profiles,
            ipc::commands::agent_get_status,
            ipc::commands::agent_set_enabled,
            ipc::commands::persona_update,
            ipc::commands::persona_delete,
            ipc::commands::characters_list,
            ipc::commands::characters_list_brief,
            ipc::commands::characters_list_page,
            ipc::commands::characters_remove_skin,
            ipc::commands::characters_import_avatars_batch,
            ipc::commands::characters_cache_avatar,
            ipc::commands::characters_cache_avatars_batch,
            ipc::commands::characters_read_avatar,
            ipc::commands::characters_skins_page,
            ipc::commands::characters_import_live2d,
            ipc::commands::live2d_import_batch,
            ipc::commands::characters_get_detail,
            ipc::commands::characters_set_active,
            ipc::commands::characters_set_skin,
            ipc::commands::character_import_wiki,
            ipc::commands::app_get_personas_path,
            ipc::commands::ai_test_persona,
            ipc::commands::character_list,
            ipc::commands::character_get,
            ipc::commands::character_create,
            ipc::commands::character_update_raw,
            ipc::commands::character_update_json,
            ipc::commands::character_save_skill,
            ipc::commands::character_delete,
            ipc::commands::character_preprocess,
            ipc::commands::character_merge_text,
            ipc::commands::character_generate_skill,
            ipc::commands::character_apply_persona,
            ipc::commands::report_generate,
            ipc::commands::report_list,
            ipc::commands::report_delete,
            ipc::commands::timeline_cached,
            ipc::commands::timeline_describe,
            ipc::commands::app_get_timeline_ai_logs_path,
            ipc::commands::work_types_get,
            ipc::commands::work_types_save,
            ipc::commands::period_list_summaries,
            ipc::commands::pet_get_status,
            ipc::commands::pet_show,
            ipc::commands::pet_hide,
            ipc::commands::pet_set_enabled,
            ipc::commands::pet_save_position,
            ipc::commands::pet_get_screen_bounds,
            ipc::commands::pet_open_main,
            ipc::commands::pet_reload,
            ipc::commands::pet_nudge,
            ipc::commands::pet_refresh_animations,
           ipc::commands::pet_preview_animation,
           ipc::commands::pet_mark_spine_ready,
           ipc::commands::pet_clear_spine_ready,
           ipc::commands::pet_get_bubble_enabled,
           ipc::commands::pet_set_bubble_enabled,
            ipc::commands::pet_get_model_status,
            ipc::commands::pet_get_config,
            ipc::commands::pet_list_models,
            ipc::commands::pet_set_model,
            ipc::commands::pet_save_model_settings,
            ipc::commands::pet_import_from_folder,
            ipc::commands::pet_import_files,
            ipc::commands::pet_pick_model_folder,
            ipc::commands::pet_stage_folder_import,
            ipc::commands::pet_stage_files_import,
            ipc::commands::pet_get_import_staging,
            ipc::commands::pet_clear_import_staging,
            ipc::commands::pet_commit_import,
            ipc::commands::pet_read_model_asset,
            ipc::commands::pet_read_model_bundle,
            ipc::commands::pet_delete_model,
            ipc::commands::pet_sync_animations,
            ipc::commands::pet_set_idle_animation,
            ipc::commands::pet_set_click_animation,
            ipc::commands::pet_set_random_animations,
            ipc::commands::pet_save_animation_layout,
            ipc::commands::pet_import_lines,
            ipc::commands::pet_ai_suggest_lines,
            ipc::commands::pet_ai_import_lines,
            ipc::commands::pet_wiki_import_lines,
            ipc::commands::pet_save_window_size,
            ipc::commands::pet_save_layout,
            ipc::commands::system_get_performance,
            ipc::commands::wechat_get_status,
            ipc::commands::wechat_start_qr,
            ipc::commands::wechat_poll_qr,
            ipc::commands::wechat_logout,
            ipc::commands::wechat_prepare_rebind,
            ipc::commands::wechat_set_push_enabled,
            ipc::commands::wechat_test_send,
            ipc::commands::wechat_import_hanagent,
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
                WindowEvent::Focused(false) => {
                    crate::pet::restore_pet_topmost_if_visible(window.app_handle());
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building 小寒日报")
        .run(handle_run_event);
}

/// 托盘构建（拆到独立模块避免 main.rs 膨胀）
pub mod tray;

/// RunEvent 处理：退出时设停机标志 + join 后台线程 + flush
///
/// Tauri 不会 join 后台线程（detached），进程退出时 OS 直接终止它。
/// 所以必须在 ExitRequested 时设 stop_flag，在 Exit 前 join + flush。
pub(crate) fn prepare_app_exit(app: &tauri::AppHandle, st: &Arc<state::AppState>) {
    st.stop_flag
        .store(true, std::sync::atomic::Ordering::Relaxed);
    crate::agent_http::stop_on_exit();
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    crate::tracker::writer::flush_on_exit(st);
    crate::pet::destroy_pet_window_for_exit(app);
}

fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    use tauri::RunEvent;
    match event {
        RunEvent::ExitRequested { api, code, .. } => {
            // 用户关窗触发的退出（code=None）→ 托盘常驻，阻止进程结束
            if code.is_none() {
                api.prevent_exit();
                return;
            }
            if let Some(st) = app.try_state::<Arc<state::AppState>>() {
                prepare_app_exit(app, &st);
            }
        }
        RunEvent::Exit => {
            if let Some(rt) = app.try_state::<crate::wechat::WechatRuntime>() {
                rt.join_all();
            }
            if let Some(st) = app.try_state::<Arc<state::AppState>>() {
                // prepare_app_exit 已 flush；此处仅 join 后台线程
                st.join_ai_workers();
            }
            // join 后台线程（后台线程在 stop_flag=true 时会跳出循环并 flush_on_exit）
            if let Some(js) = app.try_state::<state::JoinState>() {
                let _ = js.join_all();
            }
        }
        _ => {}
    }
}
