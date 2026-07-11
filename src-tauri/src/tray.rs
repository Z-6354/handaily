//! 托盘构建（纯桌宠分支）

use std::sync::Arc;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

use crate::state::AppState;

pub struct TrayMenuState {
    pub pet_toggle_item: MenuItem<tauri::Wry>,
}

pub fn sync_pet_toggle_label(app: &tauri::AppHandle) {
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        let label = if crate::pet::pet_visible(app) {
            "隐藏桌宠"
        } else {
            "显示桌宠"
        };
        let _ = tray.pet_toggle_item.set_text(label);
    }
}

pub fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let pet_toggle = MenuItem::with_id(app, "pet_toggle", "显示桌宠", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &pet_toggle, &quit])?;

    app.manage(TrayMenuState {
        pet_toggle_item: pet_toggle.clone(),
    });

    let mut tray_builder = TrayIconBuilder::new().tooltip("小寒桌宠");
    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Err(e) = crate::pet::show_main_window(app, None) {
                    crate::log::warn(format!("显示主窗口失败: {e}"));
                }
            }
            "pet_toggle" => {
                if let Some(st) = app.try_state::<Arc<AppState>>() {
                    let visible = crate::pet::pet_visible(app);
                    if visible {
                        let _ = crate::pet::hide_pet(app, false);
                    } else if let Err(e) = crate::pet::show_pet(app, st.inner()) {
                        crate::log::warn(format!("显示桌宠失败: {e}"));
                    } else {
                        crate::pet::ensure_remark_scheduler(app.clone(), st.inner().clone());
                    }
                }
            }
            "quit" => {
                if let Some(st) = app.try_state::<Arc<AppState>>() {
                    crate::request_app_exit(app, &st);
                } else {
                    crate::APP_EXITING.store(true, std::sync::atomic::Ordering::SeqCst);
                    crate::prepare_app_exit_best_effort(app);
                    app.exit(0);
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, .. } = event {
                if button == MouseButton::Left {
                    let app = tray.app_handle();
                    if let Some(win) = app.get_webview_window("main") {
                        if win.is_visible().unwrap_or(false) {
                            let _ = win.hide();
                            let _ = win.emit("main-window-visible", false);
                            crate::pet::restore_pet_topmost_if_visible(app);
                        } else if let Err(e) = crate::pet::show_main_window(app, None) {
                            crate::log::warn(format!("显示主窗口失败: {e}"));
                        }
                    } else if let Err(e) = crate::pet::show_main_window(app, None) {
                        crate::log::warn(format!("显示主窗口失败: {e}"));
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
