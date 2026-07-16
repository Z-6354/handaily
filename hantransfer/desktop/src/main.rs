#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod agent;
mod api;
mod config;
mod discovery;
mod firewall;
mod importer;
mod netutil;
mod notify;
mod outbox;
mod paths;
mod receive;
mod release;
mod settings;
mod server;
mod transfer;
mod tray;
mod trust;

use std::time::Duration;

use clap::Parser;
use tokio::sync::watch;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "hantransfer-desktop",
    version,
    about = "HANDAILY LAN file bridge — Windows desktop service"
)]
struct Cli {
    /// HTTP listen port
    #[arg(short, long)]
    port: Option<u16>,

    /// Run with system tray (blocks until quit)
    #[arg(long)]
    tray: bool,

    /// Auto-trust new devices without confirmation (dev only)
    #[arg(long)]
    auto_trust: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("hantransfer=info".parse()?))
        .init();

    let cli = Cli::parse();
    let mut config = config::Config::load()?;
    if let Some(port) = cli.port {
        config.port = port;
    }
    let settings = settings::SettingsStore::load();
    config.ensure_data_dirs()?;
    if let Err(err) = settings.ensure_inbox_dir() {
        tracing::warn!("inbox dir: {err}");
    }
    transfer::cleanup_stale_temp(&config.temp_dir, Duration::from_secs(24 * 3600));
    firewall::ensure_inbound_rule(config.port);

    let auto_trust = cli.auto_trust
        || std::env::var("HANTRANSFER_AUTO_TRUST")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

    let trust_store = trust::TrustStore::load_or_create()?;
    let trust_gate = trust::TrustGate::new(trust_store, auto_trust);
    let transfers = transfer::TransferRegistry::new();
    let receive_queue = receive::ReceiveQueue::default();
    let inbox_log = settings.inbox_dir().display().to_string();
    let state = server::AppState::new(
        config.clone(),
        trust_gate.clone(),
        transfers,
        settings,
        receive_queue,
    );

    let _mdns_guard = match discovery::MdnsRegistrar::register(&config) {
        Ok(guard) => Some(guard),
        Err(err) => {
            tracing::warn!(
                "mDNS unavailable: {err} — phone can still connect via http://<PC_IP>:{}/m/",
                config.port
            );
            None
        }
    };

    let listen = config.listen_addr();
    tracing::info!("hantransfer listening on http://{listen}");
    tracing::info!("inbox: {inbox_log}");
    for url in netutil::mobile_urls(config.port, config.lan_ipv4.as_deref()) {
        tracing::info!("mobile web: {url}");
    }
    if let Some(ip) = &config.lan_ipv4 {
        tracing::info!("lan ipv4: {ip}");
    }
    if auto_trust {
        tracing::warn!("auto-trust enabled — new devices are trusted without confirmation");
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server_handle = tokio::spawn(async move {
        if let Err(err) = server::run(state, &listen, shutdown_rx).await {
            tracing::error!("server error: {err}");
        }
    });

    if cli.tray {
        let trust_for_tray = trust_gate.clone();
        let shutdown_for_tray = shutdown_tx.clone();
        let tray_config = config.clone();
        let tray_handle = std::thread::spawn(move || {
            if let Err(err) = tray::run_blocking(&tray_config, trust_for_tray, move || {
                let _ = shutdown_for_tray.send(true);
            }) {
                tracing::error!("tray error: {err}");
            }
        });
        server_handle.await?;
        let _ = tray_handle.join();
    } else if auto_trust {
        tracing::info!("console mode — Ctrl+C to stop");
        tokio::signal::ctrl_c().await?;
        let _ = shutdown_tx.send(true);
        server_handle.await?;
    } else {
        let trust_worker = trust_gate.clone();
        let console_port = config.port;
        tokio::spawn(async move {
            trust_console_worker(trust_worker, console_port).await;
        });
        tracing::info!("console mode — open http://127.0.0.1:{}/ to approve trust; Ctrl+C to stop", config.port);
        tokio::signal::ctrl_c().await?;
        let _ = shutdown_tx.send(true);
        server_handle.await?;
    }

    drop(_mdns_guard);
    Ok(())
}

async fn trust_console_worker(trust: trust::TrustGate, port: u16) {
    use std::collections::HashSet;
    let mut notified: HashSet<uuid::Uuid> = HashSet::new();
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        for pending in trust.list_pending() {
            let id = pending.request.device_id;
            if notified.insert(id) {
                eprintln!(
                    "\n[hantransfer] 新设备请求连接: {} ({}) from {}\n请在浏览器打开 http://127.0.0.1:{}/ 点击「允许连接」\n",
                    pending.request.name,
                    pending.request.platform,
                    pending.client_ip,
                    port
                );
            }
        }
        let pending_ids: HashSet<uuid::Uuid> = trust
            .list_pending()
            .into_iter()
            .map(|p| p.request.device_id)
            .collect();
        notified.retain(|id| pending_ids.contains(id));
    }
}
