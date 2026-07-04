//! 托盘构建
//!
//! Tauri 2 的 tray API（需 `features = ["tray-icon"]`）：
//! - 托盘点击 → show/hide 主窗口
//! - 右键菜单 → 暂停/恢复采集、退出

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use crate::state::AppState;

/// 托盘菜单项句柄（用于动态更新暂停/恢复文案）
pub struct TrayMenuState {
    pub pause_item: MenuItem<tauri::Wry>,
    pub pet_toggle_item: MenuItem<tauri::Wry>,
}

pub fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let enabled = app
        .try_state::<Arc<AppState>>()
        .map(|st| st.tracking_enabled.load(Ordering::Relaxed))
        .unwrap_or(true);

    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let pet_toggle = MenuItem::with_id(app, "pet_toggle", "显示桌宠", true, None::<&str>)?;
    let pause_label = if enabled { "暂停采集" } else { "恢复采集" };
    let pause = MenuItem::with_id(app, "pause", pause_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &pet_toggle, &pause, &quit])?;

    app.manage(TrayMenuState {
        pause_item: pause.clone(),
        pet_toggle_item: pet_toggle.clone(),
    });

    let mut tray_builder = TrayIconBuilder::new().tooltip("小寒日报");
    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "pet_toggle" => {
                if let Some(st) = app.try_state::<Arc<AppState>>() {
                    let visible = crate::pet::pet_visible(app);
                    if visible {
                        let _ = crate::pet::hide_pet(app, false);
                    } else if let Err(e) = crate::pet::show_pet(app, st.inner()) {
                        eprintln!("显示桌宠失败: {e}");
                    } else {
                        crate::pet::ensure_remark_scheduler(app.clone(), st.inner().clone());
                    }
                    if let Some(tray) = app.try_state::<TrayMenuState>() {
                        let label = if crate::pet::pet_visible(app) {
                            "隐藏桌宠"
                        } else {
                            "显示桌宠"
                        };
                        let _ = tray.pet_toggle_item.set_text(label);
                    }
                }
            }
            "pause" => {
                if let Some(st) = app.try_state::<Arc<AppState>>() {
                    let enabled = st.tracking_enabled.load(Ordering::Relaxed);
                    let next = !enabled;
                    let _ = st.set_tracking_enabled(next);
                    if let Some(tray) = app.try_state::<TrayMenuState>() {
                        let label = if next { "暂停采集" } else { "恢复采集" };
                        let _ = tray.pause_item.set_text(label);
                    }
                }
            }
            "quit" => {
                app.exit(0);
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
                        } else {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
