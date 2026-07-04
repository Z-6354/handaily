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

pub mod analysis;
pub mod ai;
pub mod db;
pub mod ipc;
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
pub mod work_type;

use std::sync::Arc;
use tauri::{Manager, WindowEvent};

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
                eprintln!("桌宠启动失败: {e}");
            }

            // 5. 时间线 AI：后台自动补全今日未缓存简介（不依赖打开时间线页）
            crate::timeline::scheduler::spawn(app.handle().clone(), app_state.clone());

            // 5. 启动默认最大化；开发模式自动显示主窗口（release 仍默认隐藏到托盘）
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.maximize();
                #[cfg(debug_assertions)]
                {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }

            // 5. 存 join handle 供退出时 join
            app.manage(state::JoinState::new(stop_handle, input_handle, file_handle, audio_handle));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::commands::app_ping,
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
            ipc::commands::persona_update,
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
           ipc::commands::pet_log,
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
        ])
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            // 关闭=最小化到托盘：阻止销毁，改为隐藏
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
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
fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    use tauri::RunEvent;
    match event {
        RunEvent::ExitRequested { .. } => {
            // 设停机标志，后台线程会在下次循环检查时退出
            if let Some(st) = app.try_state::<Arc<state::AppState>>() {
                st.stop_flag
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        RunEvent::Exit => {
            if let Some(st) = app.try_state::<Arc<state::AppState>>() {
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
