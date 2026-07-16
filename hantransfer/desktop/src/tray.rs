use std::collections::HashSet;
use std::process::Command;
use std::time::Duration;

use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    TrayIconBuilder, TrayIconEvent,
};
use uuid::Uuid;

use crate::config::Config;
use crate::netutil;
use crate::notify;
use crate::paths;
use crate::trust::TrustGate;

const ID_OPEN_INBOX: &str = "open-inbox";
const ID_OPEN_MOBILE: &str = "open-mobile";
const ID_OPEN_STATUS: &str = "open-status";
const ID_QUIT: &str = "quit";

pub fn run_blocking(
    config: &Config,
    trust: TrustGate,
    on_quit: impl Fn() + Send + 'static,
) -> Result<(), String> {
    let open_id = MenuId::new(ID_OPEN_INBOX);
    let mobile_id = MenuId::new(ID_OPEN_MOBILE);
    let status_id = MenuId::new(ID_OPEN_STATUS);
    let quit_id = MenuId::new(ID_QUIT);

    let menu = Menu::new();
    let title_item = MenuItem::with_id(
        "title",
        format!("hantransfer v{}", crate::config::VERSION),
        false,
        None,
    );
    let status_item = MenuItem::with_id(
        "status",
        format!(
            "● 在线 · {} · {} 已信任",
            config.device_name,
            trust.store().trusted_count()
        ),
        false,
        None,
    );
    let open_item = MenuItem::with_id(open_id.clone(), "打开收件目录", true, None);
    let mobile_item = MenuItem::with_id(mobile_id.clone(), "打开手机页", true, None);
    let status_page = MenuItem::with_id(status_id.clone(), "打开管理页", true, None);
    let quit_item = MenuItem::with_id(quit_id.clone(), "退出", true, None);
    menu.append(&title_item).map_err(|e| e.to_string())?;
    menu.append(&status_item).map_err(|e| e.to_string())?;
    menu.append(&open_item).map_err(|e| e.to_string())?;
    menu.append(&mobile_item).map_err(|e| e.to_string())?;
    menu.append(&status_page).map_err(|e| e.to_string())?;
    menu.append(&quit_item).map_err(|e| e.to_string())?;

    let icon = build_icon()?;
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("hantransfer")
        .with_icon(icon)
        .build()
        .map_err(|e| e.to_string())?;

    tracing::info!("tray running — trust confirmation via web page");

    let mut quit = false;
    let mut notified: HashSet<Uuid> = HashSet::new();
    while !quit {
        trust.wait_for_pending_signal();
        for pending in trust.list_pending() {
            let id = pending.request.device_id;
            if notified.insert(id) {
                tracing::info!(
                    device = %pending.request.name,
                    ip = %pending.client_ip,
                    "new device pending — open web page to approve"
                );
                open_status_page(config.port);
                notify::notify_trust_pending(&pending.request.name, config.port);
            }
        }
        let pending_ids: HashSet<Uuid> = trust
            .list_pending()
            .into_iter()
            .map(|p| p.request.device_id)
            .collect();
        notified.retain(|id| pending_ids.contains(id));

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == open_id {
                open_inbox(&config.inbox_dir);
            } else if event.id == mobile_id {
                open_mobile_page(&config);
            } else if event.id == status_id {
                open_status_page(config.port);
            } else if event.id == quit_id {
                quit = true;
                on_quit();
            }
        }

        let _ = TrayIconEvent::receiver().try_recv();
        std::thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}

fn open_inbox(inbox: &std::path::Path) {
    let _ = paths::ensure_dir(inbox);
    let _ = Command::new("explorer").arg(inbox).spawn();
}

fn open_mobile_page(config: &crate::config::Config) {
    let url = netutil::mobile_urls(config.port, config.lan_ipv4.as_deref())
        .into_iter()
        .next()
        .unwrap_or_else(|| format!("http://127.0.0.1:{}/m/", config.port));
    let _ = Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn();
}

fn open_status_page(port: u16) {
    let url = format!("http://127.0.0.1:{port}/#pending-section");
    let _ = Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn();
}

fn build_icon() -> Result<tray_icon::Icon, String> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            rgba[i] = 0x4a;
            rgba[i + 1] = 0x90;
            rgba[i + 2] = 0xd9;
            rgba[i + 3] = 0xff;
        }
    }
    tray_icon::Icon::from_rgba(rgba, size, size).map_err(|e| e.to_string())
}
