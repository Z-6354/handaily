use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tokio_util::io::ReaderStream;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

use crate::api::{self, ApiOk};
use crate::config::Config;
use crate::importer::FileMetadata;
use crate::outbox::{PushEntry, PushStore};
use crate::receive::{self, PendingReceiveView, ReceiveQueue};
use crate::release::{AppRelease, SetLatestRequest};
use crate::settings::SettingsStore;
use crate::transfer::{finalize_received_file, TransferRegistry, TransferProgress, TransferStatus};
use crate::trust::{HandshakeRequest, HandshakeStatus, TrustedDevice, TrustGate};

const MAX_UPLOAD_BYTES: usize = 512 * 1024 * 1024;

async fn no_cache_mobile_assets(request: axum::http::Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, must-revalidate"),
    );
    response
}

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub trust: TrustGate,
    pub transfers: TransferRegistry,
    pub outbox: PushStore,
    pub settings: SettingsStore,
    pub receive_queue: ReceiveQueue,
}

impl AppState {
    pub fn new(
        config: Config,
        trust: TrustGate,
        transfers: TransferRegistry,
        settings: SettingsStore,
        receive_queue: ReceiveQueue,
    ) -> Self {
        let outbox = PushStore::from_config(&config.outbox_dir);
        Self {
            config,
            trust,
            transfers,
            outbox,
            settings,
            receive_queue,
        }
    }
}

#[derive(Serialize)]
struct StatusResponse {
    name: String,
    platform: &'static str,
    version: &'static str,
    device_id: Uuid,
    features: Vec<&'static str>,
    port: u16,
    lan_ip: Option<String>,
    inbox_dir: String,
    auto_accept: bool,
    trusted_devices: usize,
}

#[derive(Serialize)]
struct HandshakeResponse {
    status: HandshakeStatus,
}

#[derive(Serialize)]
struct TransferResult {
    transfer_id: Uuid,
    path: String,
    hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

pub async fn run(
    state: AppState,
    listen: &str,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mobile_dir = crate::paths::mobile_web_dir();
    let mobile_index = mobile_dir.join("index.html");
    if !mobile_index.is_file() {
        tracing::warn!("mobile-web not found at {}", mobile_dir.display());
    }
    let mobile_service = ServeDir::new(&mobile_dir).fallback(ServeFile::new(mobile_index));
    let mobile_router = Router::new()
        .fallback_service(mobile_service)
        .layer(middleware::from_fn(no_cache_mobile_assets));

    let app = Router::new()
        .route("/", get(root_page))
        .route("/m", get(|| async { Redirect::permanent("/m/") }))
        .route("/api/v1/status", get(status))
        .route("/api/v1/info", get(status))
        .route("/api/v1/trust", get(list_trust))
        .route("/api/v1/pending", get(list_pending))
        .route("/api/v1/pending/{device_id}/approve", post(approve_pending))
        .route("/api/v1/pending/{device_id}/reject", post(reject_pending))
        .route("/api/v1/trust/{device_id}", delete(revoke_trust))
        .route("/api/v1/rejected", get(list_rejected))
        .route("/api/v1/rejected/{device_id}", delete(clear_rejection))
        .route("/api/v1/agent/snapshot", get(crate::agent::snapshot))
        .route("/api/v1/agent/push", post(crate::agent::push_paths))
        .route("/api/v1/agent/receive/accept", post(crate::agent::receive_accept))
        .route("/api/v1/handshake", post(handshake))
        .route("/api/v1/receive/queue", get(receive_queue_list))
        .route("/api/v1/receive/{id}/accept", post(receive_accept))
        .route("/api/v1/receive/{id}/reject", post(receive_reject))
        .route("/api/v1/receive/accept-all", post(receive_accept_all))
        .route("/api/v1/settings", get(settings_get).post(settings_update))
        .route("/api/v1/files", post(upload_file))
        .route("/api/v1/files/check", post(check_file_duplicate))
        .route("/api/v1/transfers/{id}", get(transfer_status))
        .route("/api/v1/push", post(push_file))
        .route("/api/v1/push/pending", get(push_pending))
        .route("/api/v1/push/outbox", get(push_outbox))
        .route("/api/v1/push/{id}/file", get(push_download))
        .route("/api/v1/push/{id}/ack", post(push_ack))
        .route("/api/v1/push/{id}", delete(push_cancel))
        .route("/api/v1/app/release", get(app_release_info))
        .route("/api/v1/app/release/download", get(app_release_download))
        .route("/api/v1/app/release/list", get(app_release_list))
        .route("/api/v1/app/release/latest", post(app_release_set_latest))
        .route("/api/v1/app/release/upload", post(app_release_upload))
        .nest_service("/m/", mobile_router)
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
        .layer(RequestBodyLimitLayer::new(MAX_UPLOAD_BYTES))
        .with_state(Arc::new(state));

    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        loop {
            if *shutdown.borrow() {
                break;
            }
            if shutdown.changed().await.is_err() {
                break;
            }
        }
        tracing::info!("hantransfer shutting down");
    })
    .await?;
    Ok(())
}

async fn root_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let mobile_urls = crate::netutil::mobile_urls(state.config.port, state.config.lan_ipv4.as_deref());
    let qr_block = mobile_urls
        .first()
        .map(|url| {
            let svg = qr_svg(url);
            format!("<div class=\"qr-wrap\">{svg}<a href=\"{url}\">{url}</a></div>")
        })
        .unwrap_or_else(|| "<p>手机页：<a href=\"/m/\">/m/</a></p>".to_string());

    let trusted: Vec<TrustedDevice> = state.trust.store().list_trusted();
    let trust_rows: String = if trusted.is_empty() {
        "<li class=\"empty\"><div class=\"empty-icon\">📱</div>暂无已信任设备</li>".to_string()
    } else {
        trusted
            .iter()
            .map(|d| {
                let initial = d.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                format!(
                    "<li class=\"list-item\"><div class=\"list-item-row device-row\">\
                     <div class=\"device-avatar\">{initial}</div>\
                     <div class=\"device-info\"><strong>{name}</strong><span>{platform} · {id}</span></div>\
                     <button type=\"button\" class=\"btn btn-ghost btn-sm\" onclick=\"revokeTrust('{id}')\">撤销</button></div></li>",
                    name = html_escape(&d.name),
                    platform = html_escape(&d.platform),
                    id = d.device_id,
                    initial = html_escape(&initial),
                )
            })
            .collect()
    };

    let settings = state.settings.snapshot();
    let inbox = html_escape(&settings.inbox_dir.display().to_string());
    let default_inbox = html_escape(&crate::paths::model_inbox().display().to_string());
    let auto_accept_checked = if settings.auto_accept { "checked" } else { "" };
    let recent = read_recent_history(&state.config.history_dir, 8);
    let history_rows: String = if recent.is_empty() {
        "<li class=\"empty\"><div class=\"empty-icon\">📥</div>暂无记录</li>".to_string()
    } else {
        recent
            .into_iter()
            .map(|line| format!("<li class=\"list-item\"><span class=\"list-item-meta\">{line}</span></li>"))
            .collect()
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>hantransfer</title>
<style>
:root {{
  --accent: #0d9488;
  --accent-hover: #0f766e;
  --accent-soft: rgba(13,148,136,.12);
  --accent-glow: rgba(13,148,136,.25);
  --indigo: #4f46e5;
  --bg: #e8ecf2;
  --bg-grid: rgba(15,23,42,.04);
  --surface: #fff;
  --surface-2: #f8fafc;
  --text: #0f172a;
  --muted: #64748b;
  --border: #e2e8f0;
  --border-strong: #cbd5e1;
  --success: #059669;
  --danger: #dc2626;
  --warn: #d97706;
  --radius: 16px;
  --radius-sm: 10px;
  --shadow: 0 1px 2px rgba(15,23,42,.06), 0 8px 24px rgba(15,23,42,.06);
  --shadow-lg: 0 4px 6px rgba(15,23,42,.04), 0 20px 48px rgba(15,23,42,.1);
}}
* {{ box-sizing: border-box; margin: 0; }}
html {{ scroll-behavior: smooth; }}
body {{
  font-family: "Segoe UI Variable", "Segoe UI", system-ui, -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
  min-height: 100vh;
  background: var(--bg);
  background-image:
    linear-gradient(var(--bg-grid) 1px, transparent 1px),
    linear-gradient(90deg, var(--bg-grid) 1px, transparent 1px);
  background-size: 24px 24px;
  color: var(--text);
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
}}
.app {{
  max-width: 1140px;
  margin: 0 auto;
  padding: 0 1.25rem 3rem;
}}
.topbar {{
  position: sticky;
  top: 0;
  z-index: 50;
  margin: 0 -1.25rem;
  padding: 1rem 1.25rem 1.25rem;
  background: rgba(232,236,242,.85);
  backdrop-filter: blur(12px);
  border-bottom: 1px solid rgba(226,232,240,.8);
}}
.topbar-inner {{
  max-width: 1140px;
  margin: 0 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 1rem 1.5rem;
}}
.brand {{
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex: 1;
  min-width: 200px;
}}
.brand-mark {{
  width: 42px;
  height: 42px;
  border-radius: 12px;
  background: linear-gradient(135deg, var(--accent), #0891b2);
  display: grid;
  place-items: center;
  color: #fff;
  font-weight: 800;
  font-size: 1.1rem;
  box-shadow: 0 4px 14px var(--accent-glow);
}}
.brand h1 {{
  font-size: 1.35rem;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.2;
}}
.brand p {{
  font-size: 0.78rem;
  color: var(--muted);
  margin-top: 0.1rem;
}}
.status-pill {{
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.4rem 0.85rem;
  border-radius: 999px;
  background: #ecfdf5;
  border: 1px solid #a7f3d0;
  color: #047857;
  font-size: 0.82rem;
  font-weight: 600;
}}
.status-dot {{
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #10b981;
  box-shadow: 0 0 0 0 rgba(16,185,129,.5);
  animation: pulse 2s infinite;
}}
@keyframes pulse {{
  0%, 100% {{ box-shadow: 0 0 0 0 rgba(16,185,129,.45); }}
  50% {{ box-shadow: 0 0 0 6px rgba(16,185,129,0); }}
}}
.chips {{
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}}
.chip {{
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.35rem 0.7rem;
  border-radius: 999px;
  background: var(--surface);
  border: 1px solid var(--border);
  font-size: 0.78rem;
  color: var(--muted);
}}
.chip b {{ color: var(--text); font-weight: 600; }}
.main-grid {{
  display: grid;
  grid-template-columns: 1.15fr 0.85fr;
  gap: 1.25rem;
  margin-top: 1.25rem;
}}
@media (max-width: 900px) {{
  .main-grid {{ grid-template-columns: 1fr; }}
}}
.ai-panel {{
  margin-top: 1.25rem;
  border-color: var(--accent);
}}
.ai-stats {{ display: flex; flex-wrap: wrap; gap: 0.45rem; }}
.ai-json summary {{ cursor: pointer; user-select: none; }}
.ai-json pre {{
  margin: 0.5rem 0 0;
  max-height: 220px;
  overflow: auto;
  font-size: 0.72rem;
}}
.col {{ display: flex; flex-direction: column; gap: 1.25rem; }}
.panel {{
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  box-shadow: var(--shadow);
  overflow: hidden;
}}
.panel-head {{
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 1rem 1.15rem 0.85rem;
  border-bottom: 1px solid var(--border);
  background: linear-gradient(180deg, var(--surface-2), var(--surface));
}}
.panel-head h2 {{
  font-size: 0.95rem;
  font-weight: 700;
  letter-spacing: -0.01em;
}}
.panel-head p {{
  font-size: 0.78rem;
  color: var(--muted);
  margin-top: 0.15rem;
}}
.panel-body {{ padding: 1rem 1.15rem 1.15rem; }}
.panel-highlight {{
  border-color: rgba(13,148,136,.35);
  box-shadow: var(--shadow), 0 0 0 1px rgba(13,148,136,.08);
}}
.badge {{
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 1.35rem;
  height: 1.35rem;
  padding: 0 0.4rem;
  border-radius: 999px;
  background: var(--accent);
  color: #fff;
  font-size: 0.72rem;
  font-weight: 700;
}}
.badge.hidden {{ display: none; }}
.badge.warn {{ background: var(--warn); }}
.toolbar {{
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.65rem;
  margin-bottom: 0.85rem;
}}
.toolbar .hint {{ margin: 0; flex: 1; min-width: 140px; }}
label {{
  display: block;
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--muted);
  margin-bottom: 0.35rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}}
input[type="text"], select, input[type="file"] {{
  width: 100%;
  padding: 0.65rem 0.8rem;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  font-size: 0.9rem;
  background: var(--surface);
  color: var(--text);
  transition: border-color .15s, box-shadow .15s;
}}
input[type="text"]:focus, select:focus {{
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
}}
input.mono {{ font-family: "Cascadia Code", "JetBrains Mono", Consolas, monospace; font-size: 0.82rem; }}
.field {{ margin-bottom: 0.85rem; }}
.field:last-child {{ margin-bottom: 0; }}
.btn {{
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
  padding: 0.55rem 1rem;
  border: none;
  border-radius: var(--radius-sm);
  font-size: 0.88rem;
  font-weight: 600;
  cursor: pointer;
  transition: transform .12s, background .15s, box-shadow .15s;
  white-space: nowrap;
}}
.btn:active {{ transform: scale(.98); }}
.btn:disabled {{ opacity: .45; cursor: not-allowed; transform: none; }}
.btn-primary {{ background: var(--accent); color: #fff; }}
.btn-primary:hover:not(:disabled) {{ background: var(--accent-hover); }}
.btn-success {{ background: var(--success); color: #fff; }}
.btn-danger {{ background: var(--danger); color: #fff; }}
.btn-ghost {{
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border-strong);
}}
.btn-ghost:hover:not(:disabled) {{ background: var(--surface-2); }}
.btn-sm {{ padding: 0.35rem 0.7rem; font-size: 0.8rem; }}
.btn-row {{ display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 0.65rem; }}
.toggle-row {{
  display: flex;
  align-items: center;
  gap: 0.55rem;
  margin: 0.85rem 0 0.25rem;
  font-size: 0.88rem;
  color: var(--text);
  cursor: pointer;
}}
.toggle-row input {{ width: auto; accent-color: var(--accent); }}
.hint {{ color: var(--muted); font-size: 0.82rem; line-height: 1.45; }}
.hint.ok {{ color: var(--success); }}
.hint.err {{ color: var(--danger); }}
code {{
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 0.85em;
  background: #f1f5f9;
  border: 1px solid var(--border);
  padding: 0.1rem 0.35rem;
  border-radius: 5px;
}}
.list {{ list-style: none; display: flex; flex-direction: column; gap: 0.55rem; }}
.list-item {{
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 0.75rem 0.85rem;
}}
.list-item-row {{
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem 0.75rem;
}}
.list-item-row strong {{ flex: 1; min-width: 120px; font-size: 0.9rem; }}
.list-item-meta {{ font-size: 0.78rem; color: var(--muted); width: 100%; }}
.device-row {{
  display: flex;
  align-items: center;
  gap: 0.65rem;
}}
.device-avatar {{
  width: 34px;
  height: 34px;
  border-radius: 10px;
  background: linear-gradient(135deg, #e0e7ff, #c7d2fe);
  color: var(--indigo);
  font-weight: 700;
  font-size: 0.85rem;
  display: grid;
  place-items: center;
  flex-shrink: 0;
}}
.device-info {{ flex: 1; min-width: 0; }}
.device-info strong {{ display: block; font-size: 0.88rem; }}
.device-info span {{ font-size: 0.76rem; color: var(--muted); }}
.empty {{
  text-align: center;
  padding: 1.5rem 1rem;
  color: var(--muted);
  font-size: 0.85rem;
}}
.empty-icon {{
  font-size: 1.75rem;
  margin-bottom: 0.35rem;
  opacity: .55;
}}
.receive-card {{
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 0.85rem;
}}
.receive-card + .receive-card {{ margin-top: 0.55rem; }}
.receive-head {{
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 0.5rem;
  margin-bottom: 0.55rem;
}}
.receive-head strong {{ font-size: 0.9rem; word-break: break-all; }}
.receive-meta {{ font-size: 0.76rem; color: var(--muted); white-space: nowrap; }}
.progress-track {{
  height: 6px;
  background: var(--border);
  border-radius: 999px;
  overflow: hidden;
}}
.progress-fill {{
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, var(--accent), #0891b2);
  transition: width .25s ease;
}}
.progress-label {{
  display: flex;
  justify-content: space-between;
  font-size: 0.74rem;
  color: var(--muted);
  margin-top: 0.3rem;
}}
#push-drop-zone {{
  border: 2px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  padding: 1.25rem 1rem;
  text-align: center;
  background: var(--surface-2);
  transition: border-color .15s, background .15s;
}}
#push-drop-zone.drag-over {{
  border-color: var(--accent);
  background: var(--accent-soft);
}}
.drop-icon {{ font-size: 1.75rem; margin-bottom: 0.35rem; opacity: .6; }}
.qr-wrap {{
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.65rem;
  padding: 0.5rem 0;
}}
.qr-wrap svg {{
  width: 160px;
  height: 160px;
  border-radius: 12px;
  background: #fff;
  padding: 8px;
  border: 1px solid var(--border);
}}
.qr-wrap a {{
  font-size: 0.82rem;
  color: var(--accent);
  word-break: break-all;
  text-align: center;
}}
progress {{ width: 100%; height: 6px; border-radius: 999px; margin-top: 0.5rem; accent-color: var(--accent); }}
.hidden {{ display: none !important; }}
.modal {{
  position: fixed;
  inset: 0;
  background: rgba(15,23,42,.5);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
  padding: 1rem;
}}
.modal.hidden {{ display: none; }}
.modal-box {{
  background: var(--surface);
  border-radius: var(--radius);
  padding: 1.5rem;
  max-width: 420px;
  width: 100%;
  box-shadow: var(--shadow-lg);
  animation: modalIn .2s ease;
}}
@keyframes modalIn {{
  from {{ opacity: 0; transform: scale(.96) translateY(8px); }}
  to {{ opacity: 1; transform: none; }}
}}
.trust-card {{
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 1rem;
  margin-top: 0.75rem;
  background: var(--surface-2);
}}
.trust-card p {{ margin: 0.25rem 0; font-size: 0.88rem; }}
.trust-actions {{ display: flex; gap: 0.5rem; margin-top: 0.75rem; }}
.toast-host {{
  position: fixed;
  bottom: 1.25rem;
  right: 1.25rem;
  z-index: 300;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  pointer-events: none;
}}
.toast {{
  padding: 0.65rem 1rem;
  border-radius: var(--radius-sm);
  background: var(--text);
  color: #fff;
  font-size: 0.85rem;
  box-shadow: var(--shadow-lg);
  animation: toastIn .25s ease;
  pointer-events: auto;
}}
.toast.ok {{ background: #065f46; }}
.toast.err {{ background: #991b1b; }}
@keyframes toastIn {{
  from {{ opacity: 0; transform: translateY(8px); }}
  to {{ opacity: 1; transform: none; }}
}}
@media (prefers-reduced-motion: reduce) {{
  *, *::before, *::after {{ animation: none !important; transition: none !important; }}
}}
</style>
</head>
<body>
<div id="trust-modal" class="modal hidden">
  <div class="modal-box">
    <h2 style="margin:0 0 0.5rem;font-size:1.1rem">新设备请求连接</h2>
    <p class="hint" style="margin-bottom:0.75rem">确认是否允许该设备访问本机。</p>
    <div id="trust-modal-body"></div>
  </div>
</div>
<div id="toast-host" class="toast-host"></div>
<div class="app">
<header class="topbar">
  <div class="topbar-inner">
    <div class="brand">
      <div class="brand-mark">H</div>
      <div>
        <h1>hantransfer</h1>
        <p>局域网文件桥 · v{version}</p>
      </div>
    </div>
    <div class="status-pill"><span class="status-dot"></span>在线 · {platform}</div>
    <div class="chips">
      <span class="chip"><b>{name}</b></span>
      <span class="chip">端口 <b>{port}</b></span>
      <span class="chip">IP <b>{lan_ip}</b></span>
      <span class="chip">已信任 <b id="trusted-count">{trusted_count}</b></span>
    </div>
  </div>
</header>
<section class="panel ai-panel" id="ai-panel" data-agent-api="/api/v1/agent/snapshot">
  <div class="panel-head">
    <div>
      <h2>AI 状态</h2>
      <p>供 Cursor Agent 查看设备与收件；同源 API：<code>/api/v1/agent/snapshot</code></p>
    </div>
    <button type="button" class="btn btn-ghost btn-sm" id="btn-ai-refresh">刷新</button>
  </div>
  <div class="panel-body">
    <div class="ai-stats" id="ai-stats">
      <span class="chip">已信任 <b id="ai-trusted">-</b></span>
      <span class="chip">待确认 <b id="ai-pending-trust">-</b></span>
      <span class="chip">待收件 <b id="ai-pending-recv">-</b></span>
      <span class="chip">推送队列 <b id="ai-outbox">-</b></span>
    </div>
    <ul id="ai-device-list" class="list" data-ai="devices" style="margin-top:0.75rem"></ul>
    <details class="ai-json" style="margin-top:0.75rem">
      <summary class="hint">原始 snapshot JSON（Agent 可读）</summary>
      <pre id="ai-snapshot-json" class="probe-box" data-ai="snapshot-json">加载中…</pre>
    </details>
  </div>
</section>
<div class="main-grid">
  <div class="col">
    <section class="panel panel-highlight" id="receive-panel">
      <div class="panel-head">
        <div>
          <h2>收件</h2>
          <p id="receive-summary">加载中…</p>
        </div>
        <span id="pending-badge" class="badge hidden">0</span>
      </div>
      <div class="panel-body">
        <div class="toolbar">
          <button type="button" class="btn btn-success" id="btn-accept-all">全部接受</button>
        </div>
        <div id="receiving-list"></div>
        <ul id="pending-receive-list" class="list"></ul>
      </div>
    </section>
    <section class="panel">
      <div class="panel-head">
        <div><h2>发送到手机</h2><p>拖放或选择文件推送到已信任设备</p></div>
      </div>
      <div class="panel-body">
        <div class="field">
          <label for="push-target">目标设备</label>
          <select id="push-target"><option value="">加载中…</option></select>
        </div>
        <div id="push-drop-zone">
          <div class="drop-icon">📁</div>
          <p class="hint">拖放文件到此处，或点击选择（可多选）</p>
          <input type="file" id="push-file" multiple />
        </div>
        <div class="btn-row">
          <button type="button" class="btn btn-primary" id="btn-push-file">发送</button>
        </div>
        <progress id="push-progress" value="0" max="100" class="hidden"></progress>
        <p id="push-msg" class="hint"></p>
      </div>
    </section>
    <section class="panel">
      <div class="panel-head">
        <div><h2>待手机接收</h2><p>已推送、等待手机拉取的文件</p></div>
      </div>
      <div class="panel-body">
        <ul id="outbox-list" class="list"><li class="empty"><div class="empty-icon">📤</div>加载中…</li></ul>
      </div>
    </section>
    <section class="panel">
      <div class="panel-head">
        <div><h2>手机 App 更新</h2><p>管理 APK 版本，供手机自动检测更新</p></div>
      </div>
      <div class="panel-body" id="release-card">
        <p id="release-current" class="hint">当前最新版：加载中…</p>
        <div class="field">
          <label for="release-select">release 目录中的 APK</label>
          <select id="release-select"><option value="">加载中…</option></select>
        </div>
        <div class="btn-row">
          <button type="button" class="btn btn-primary" id="btn-set-release">设为最新版</button>
        </div>
        <div class="field" style="margin-top:1rem">
          <label for="release-upload">或上传 APK</label>
          <input type="file" id="release-upload" accept=".apk,application/vnd.android.package-archive" />
        </div>
        <div class="btn-row">
          <button type="button" class="btn btn-ghost" id="btn-upload-release">上传并设为最新版</button>
        </div>
        <p id="release-msg" class="hint"></p>
      </div>
    </section>
    <section class="panel">
      <div class="panel-head">
        <div><h2>最近收件</h2><p>已成功保存的文件记录</p></div>
      </div>
      <div class="panel-body">
        <ul id="history-list" class="list">{history_rows}</ul>
      </div>
    </section>
  </div>
  <div class="col">
    <section class="panel">
      <div class="panel-head">
        <div><h2>收件目录</h2><p>保存位置 · 碧蓝资源写入 <code>azurlane/</code></p></div>
      </div>
      <div class="panel-body" id="inbox-card">
        <div class="field">
          <label for="inbox-dir">目录路径</label>
          <input type="text" class="mono" id="inbox-dir" value="{inbox}" spellcheck="false" />
        </div>
        <div class="btn-row">
          <button type="button" class="btn btn-ghost btn-sm" id="btn-inbox-default">恢复默认路径</button>
        </div>
        <label class="toggle-row"><input type="checkbox" id="auto-accept" {auto_accept_checked} /> 自动接受（跳过确认）</label>
        <div class="btn-row">
          <button type="button" class="btn btn-primary" id="btn-save-inbox">保存设置</button>
        </div>
        <p id="inbox-msg" class="hint"></p>
      </div>
    </section>
    <section class="panel">
      <div class="panel-head">
        <div><h2>手机连接</h2><p>扫码或打开链接配对</p></div>
      </div>
      <div class="panel-body">{qr_block}</div>
    </section>
    <section class="panel" id="pending-section">
      <div class="panel-head">
        <div><h2>待确认设备</h2><p>等待你批准的新设备</p></div>
        <span id="trust-pending-badge" class="badge warn hidden">0</span>
      </div>
      <div class="panel-body">
        <ul id="pending-list" class="list"><li class="empty">加载中…</li></ul>
      </div>
    </section>
    <section class="panel">
      <div class="panel-head">
        <div><h2>已信任设备</h2><p>可收发文件的设备</p></div>
      </div>
      <div class="panel-body">
        <ul id="trust-list" class="list">{trust_rows}</ul>
      </div>
    </section>
    <section class="panel">
      <div class="panel-head">
        <div><h2>已拒绝设备</h2><p>解除后可重新请求连接</p></div>
      </div>
      <div class="panel-body">
        <ul id="rejected-list" class="list"><li class="empty">加载中…</li></ul>
      </div>
    </section>
  </div>
</div>
</div>
<script>
function toast(msg, kind) {{
  const host = document.getElementById('toast-host');
  if (!host) return;
  const el = document.createElement('div');
  el.className = 'toast' + (kind ? ' ' + kind : '');
  el.textContent = msg;
  host.appendChild(el);
  setTimeout(() => el.remove(), 3200);
}}
function deviceAvatar(name) {{
  const c = (name || '?').trim()[0] || '?';
  return c.toUpperCase();
}}
function showTrustModal(devices) {{
  const modal = document.getElementById('trust-modal');
  const body = document.getElementById('trust-modal-body');
  if (!devices.length) {{
    modal.classList.add('hidden');
    return;
  }}
  modal.classList.remove('hidden');
  body.innerHTML = devices.map(d => `
    <div class="trust-card">
      <div class="device-row">
        <div class="device-avatar">${{deviceAvatar(d.name)}}</div>
        <div class="device-info"><strong>${{d.name}}</strong><span>${{d.platform}} · ${{d.ip}}</span></div>
      </div>
      <div class="trust-actions">
        <button type="button" class="btn btn-primary btn-sm" onclick="approveTrust('${{d.device_id}}')">允许连接</button>
        <button type="button" class="btn btn-ghost btn-sm" onclick="rejectTrust('${{d.device_id}}')">拒绝</button>
      </div>
    </div>`).join('');
}}
function fmtSize(n) {{
  if (!n) return '0 B';
  if (n < 1024) return n + ' B';
  if (n < 1048576) return (n / 1024).toFixed(1) + ' KB';
  return (n / 1048576).toFixed(1) + ' MB';
}}
function pct(received, total) {{
  if (!total) return 0;
  return Math.min(100, Math.round((received / total) * 100));
}}
async function refreshReceiveQueue() {{
  const receivingEl = document.getElementById('receiving-list');
  const pendingEl = document.getElementById('pending-receive-list');
  const summaryEl = document.getElementById('receive-summary');
  const acceptAllBtn = document.getElementById('btn-accept-all');
  const badge = document.getElementById('pending-badge');
  const panel = document.getElementById('receive-panel');
  if (!receivingEl || !pendingEl) return;
  try {{
    const body = await fetch('/api/v1/receive/queue').then(r => r.json());
    const data = body.data || {{}};
    const receiving = (data.receiving || []).filter(t => t.status === 'receiving' || t.status === 'verifying');
    const pending = data.pending || [];
    if (acceptAllBtn) acceptAllBtn.disabled = pending.length === 0;
    if (badge) {{
      badge.textContent = String(pending.length);
      badge.classList.toggle('hidden', pending.length === 0);
    }}
    if (panel) panel.classList.toggle('panel-highlight', pending.length > 0 || receiving.length > 0);
    if (summaryEl) {{
      summaryEl.textContent = pending.length
        ? `${{pending.length}} 个文件待确认`
        : receiving.length ? `${{receiving.length}} 个文件传输中` : '暂无待处理文件';
    }}
    receivingEl.innerHTML = receiving.length
      ? receiving.map(t => {{
          const p = pct(t.bytes_received, t.total);
          const name = t.filename || t.transfer_id;
          const statusLabel = t.status === 'verifying' ? '校验中' : '接收中';
          return `<div class="receive-card">
            <div class="receive-head"><strong>${{name}}</strong><span class="receive-meta">${{statusLabel}}</span></div>
            <div class="progress-track"><div class="progress-fill" style="width:${{p}}%"></div></div>
            <div class="progress-label"><span>${{fmtSize(t.bytes_received)}} / ${{fmtSize(t.total)}}</span><span>${{p}}%</span></div>
          </div>`;
        }}).join('')
      : '';
    pendingEl.innerHTML = pending.length
      ? pending.map(p => `<li class="list-item receive-card">
          <div class="receive-head"><strong>${{p.filename}}</strong><span class="receive-meta">${{fmtSize(p.size)}}</span></div>
          <div class="receive-meta">${{p.device_name}} · ${{p.received_at.replace('T',' ').slice(0,19)}}</div>
          <div class="btn-row">
            <button type="button" class="btn btn-success btn-sm" onclick="acceptReceive('${{p.transfer_id}}')">接受</button>
            <button type="button" class="btn btn-ghost btn-sm" onclick="rejectReceive('${{p.transfer_id}}')">拒绝</button>
          </div>
        </li>`).join('')
      : '<li class="empty"><div class="empty-icon">✓</div>暂无待确认文件</li>';
  }} catch (_) {{}}
}}
async function acceptReceive(id) {{
  const r = await fetch('/api/v1/receive/' + id + '/accept', {{ method: 'POST' }});
  const body = await r.json();
  if (!body.ok) {{ toast(body.error?.message || '接受失败', 'err'); return; }}
  toast('文件已保存', 'ok');
  refreshReceiveQueue();
}}
async function rejectReceive(id) {{
  if (!confirm('拒绝接收此文件？')) return;
  const r = await fetch('/api/v1/receive/' + id + '/reject', {{ method: 'POST' }});
  const body = await r.json();
  if (!body.ok) {{ toast(body.error?.message || '拒绝失败', 'err'); return; }}
  toast('已拒绝');
  refreshReceiveQueue();
}}
document.getElementById('btn-accept-all')?.addEventListener('click', async () => {{
  const r = await fetch('/api/v1/receive/accept-all', {{ method: 'POST' }});
  const body = await r.json();
  if (!body.ok) {{ toast(body.error?.message || '批量接受失败', 'err'); return; }}
  toast(`已接受 ${{body.data?.accepted || 0}} 个文件`, 'ok');
  refreshReceiveQueue();
}});
async function refreshInboxSettings() {{
  try {{
    const body = await fetch('/api/v1/settings').then(r => r.json());
    if (!body.ok) return;
    const data = body.data || {{}};
    const input = document.getElementById('inbox-dir');
    const auto = document.getElementById('auto-accept');
    if (input && data.inbox_dir) input.value = data.inbox_dir;
    if (auto) auto.checked = !!data.auto_accept;
    window.__defaultInbox = data.default_inbox_dir || '{default_inbox}';
  }} catch (_) {{}}
}}
document.getElementById('btn-inbox-default')?.addEventListener('click', () => {{
  const input = document.getElementById('inbox-dir');
  if (input) input.value = window.__defaultInbox || '{default_inbox}';
}});
document.getElementById('btn-save-inbox')?.addEventListener('click', async () => {{
  const input = document.getElementById('inbox-dir');
  const auto = document.getElementById('auto-accept');
  const msg = document.getElementById('inbox-msg');
  if (msg) msg.textContent = '保存中…';
  const r = await fetch('/api/v1/settings', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    body: JSON.stringify({{
      inbox_dir: input?.value?.trim() || null,
      auto_accept: !!auto?.checked,
    }}),
  }});
  const body = await r.json();
  if (!body.ok) {{ if (msg) {{ msg.textContent = body.error?.message || '保存失败'; msg.className = 'hint err'; }} return; }}
  if (msg) {{ msg.textContent = '已保存收件设置'; msg.className = 'hint ok'; }}
  toast('收件设置已保存', 'ok');
  refreshInboxSettings();
}});
async function refreshRelease() {{
  const sel = document.getElementById('release-select');
  const cur = document.getElementById('release-current');
  if (!sel || !cur) return;
  try {{
    const r = await fetch('/api/v1/app/release/list');
    const body = await r.json();
    if (!body.ok) {{
      cur.textContent = body.error?.message || '无法加载 APK 列表';
      sel.innerHTML = '<option value="">无</option>';
      return;
    }}
    const data = body.data || {{}};
    const latest = data.latest;
    const files = data.files || [];
    cur.textContent = latest
      ? `当前最新版：${{latest.display}} · ${{latest.filename}} · ${{(latest.size / 1048576).toFixed(1)}} MB`
      : '当前最新版：未设置（请上传或选择 APK）';
    sel.innerHTML = files.length
      ? files.map(f => `<option value="${{f.filename}}" ${{latest && latest.filename === f.filename ? 'selected' : ''}}>${{f.display}} · ${{f.filename}} (${{(f.size / 1048576).toFixed(1)}} MB)</option>`).join('')
      : '<option value="">release 目录暂无 APK</option>';
  }} catch (e) {{
    cur.textContent = '加载失败: ' + e;
  }}
}}
document.getElementById('btn-set-release')?.addEventListener('click', async () => {{
  const filename = document.getElementById('release-select')?.value;
  const msg = document.getElementById('release-msg');
  if (!filename) {{ if (msg) msg.textContent = '请选择 APK 文件'; return; }}
  if (msg) msg.textContent = '设置中…';
  const r = await fetch('/api/v1/app/release/latest', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    body: JSON.stringify({{ filename }}),
  }});
  const body = await r.json();
  if (!body.ok) {{ if (msg) msg.textContent = body.error?.message || '设置失败'; return; }}
  if (msg) msg.textContent = `已设为最新版：${{body.data?.display || filename}}`;
  refreshRelease();
}});
document.getElementById('btn-upload-release')?.addEventListener('click', async () => {{
  const input = document.getElementById('release-upload');
  const msg = document.getElementById('release-msg');
  const file = input?.files?.[0];
  if (!file) {{ if (msg) msg.textContent = '请选择 APK 文件'; return; }}
  if (msg) msg.textContent = '上传中…';
  const form = new FormData();
  form.append('file', file, file.name);
  form.append('set_latest', '1');
  const r = await fetch('/api/v1/app/release/upload', {{ method: 'POST', body: form }});
  const body = await r.json();
  if (!body.ok) {{ if (msg) msg.textContent = body.error?.message || '上传失败'; return; }}
  if (msg) msg.textContent = `已上传并设为最新版：${{body.data?.display || file.name}}`;
  if (input) input.value = '';
  refreshRelease();
}});
async function refreshDevices() {{
  try {{
    const pending = await fetch('/api/v1/pending').then(r => r.json());
    const trust = await fetch('/api/v1/trust').then(r => r.json());
    const pendingList = document.getElementById('pending-list');
    const trustList = document.getElementById('trust-list');
    const rejectedList = document.getElementById('rejected-list');
    const pushTarget = document.getElementById('push-target');
    const pendingData = pending.data || [];
    const trustData = trust.data || [];
    const rejected = await fetch('/api/v1/rejected').then(r => r.json()).catch(() => ({{ data: [] }}));
    const rejectedData = rejected.data || [];
    const trustBadge = document.getElementById('trust-pending-badge');
    if (trustBadge) {{
      trustBadge.textContent = String(pendingData.length);
      trustBadge.classList.toggle('hidden', pendingData.length === 0);
    }}
    showTrustModal(pendingData);
    pendingList.innerHTML = pendingData.length
      ? pendingData.map(d => `<li class="list-item"><div class="list-item-row device-row">
          <div class="device-avatar">${{deviceAvatar(d.name)}}</div>
          <div class="device-info"><strong>${{d.name}}</strong><span>${{d.platform}} · ${{d.ip}}</span></div>
          <button type="button" class="btn btn-primary btn-sm" onclick="approveTrust('${{d.device_id}}')">允许</button>
          <button type="button" class="btn btn-ghost btn-sm" onclick="rejectTrust('${{d.device_id}}')">拒绝</button>
        </div></li>`).join('')
      : '<li class="empty"><div class="empty-icon">✓</div>暂无待确认设备</li>';
    trustList.innerHTML = trustData.length
      ? trustData.map(d => `<li class="list-item"><div class="list-item-row device-row">
          <div class="device-avatar">${{deviceAvatar(d.name)}}</div>
          <div class="device-info"><strong>${{d.name}}</strong><span>${{d.platform}} · ${{d.device_id}}</span></div>
          <button type="button" class="btn btn-ghost btn-sm" onclick="revokeTrust('${{d.device_id}}')">撤销</button>
        </div></li>`).join('')
      : '<li class="empty"><div class="empty-icon">📱</div>暂无已信任设备</li>';
    if (rejectedList) {{
      rejectedList.innerHTML = rejectedData.length
        ? rejectedData.map(id => `<li class="list-item"><div class="list-item-row device-row">
            <div class="device-avatar">!</div>
            <div class="device-info"><strong>已拒绝</strong><span>${{id}}</span></div>
            <button type="button" class="btn btn-primary btn-sm" onclick="clearRejection('${{id}}')">解除拒绝</button>
          </div></li>`).join('')
        : '<li class="empty"><div class="empty-icon">✓</div>暂无已拒绝设备</li>';
    }}
    if (pushTarget) {{
      const prev = pushTarget.value;
      pushTarget.innerHTML = trustData.length
        ? trustData.map(d => `<option value="${{d.device_id}}">${{d.name}} (${{d.platform}})</option>`).join('')
        : '<option value="">暂无已信任设备</option>';
      if (prev) pushTarget.value = prev;
    }}
    const outboxList = document.getElementById('outbox-list');
    const outbox = await fetch('/api/v1/push/outbox').then(r => r.json()).catch(() => ({{ data: [] }}));
    const outboxData = outbox.data || [];
    if (outboxList) {{
      const fmtSize = (n) => {{
        if (!n) return '0 B';
        if (n < 1024) return n + ' B';
        if (n < 1048576) return (n / 1024).toFixed(1) + ' KB';
        return (n / 1048576).toFixed(1) + ' MB';
      }};
      outboxList.innerHTML = outboxData.length
        ? outboxData.map(o => {{
            const target = trustData.find(t => t.device_id === o.target_device_id);
            const label = target ? target.name : o.target_device_id;
            const when = o.created_at ? o.created_at.replace('T', ' ').slice(0, 19) : '';
            return `<li class="list-item"><div class="list-item-row">
              <strong>${{o.filename}}</strong>
              <span class="receive-meta">${{fmtSize(o.size)}} → ${{label}}</span>
              <button type="button" class="btn btn-ghost btn-sm" onclick="cancelPush('${{o.push_id}}')">取消</button>
            </div><div class="list-item-meta">${{when}}</div></li>`;
          }}).join('')
        : '<li class="empty"><div class="empty-icon">📤</div>暂无待接收推送</li>';
    }}
    document.getElementById('trusted-count').textContent = trustData.length;
  }} catch (_) {{}}
}}
document.getElementById('btn-push-file')?.addEventListener('click', async () => {{
  const fileInput = document.getElementById('push-file');
  await pushFilesToPhone(Array.from(fileInput?.files || []), fileInput);
}});
async function pushFilesToPhone(files, fileInput) {{
  const target = document.getElementById('push-target')?.value;
  const msg = document.getElementById('push-msg');
  const prog = document.getElementById('push-progress');
  if (!target) {{ msg.textContent = '请选择已信任的手机设备'; return; }}
  if (!files.length) {{ msg.textContent = '请选择要发送的文件'; return; }}
  prog?.classList.remove('hidden');
  let ok = 0;
  for (let i = 0; i < files.length; i++) {{
    const file = files[i];
    msg.textContent = `推送 ${{i + 1}}/${{files.length}}: ${{file.name}}`;
    if (prog) prog.value = 0;
    const form = new FormData();
    form.append('target_device_id', target);
    form.append('file', file, file.name);
    form.append('notify', (i === files.length - 1) ? '1' : '0');
    if (files.length > 1) form.append('batch_total', String(files.length));
    try {{
      const body = await new Promise((resolve, reject) => {{
        const xhr = new XMLHttpRequest();
        xhr.upload.onprogress = (e) => {{
          if (e.lengthComputable && prog) {{
            const base = (ok / files.length) * 100;
            const part = (e.loaded / e.total) * (100 / files.length);
            prog.value = Math.round(base + part);
          }}
        }};
        xhr.onload = () => {{
          let parsed = {{}};
          try {{ parsed = JSON.parse(xhr.responseText || '{{}}'); }} catch (_) {{}}
          resolve({{ ok: xhr.status >= 200 && xhr.status < 300, status: xhr.status, body: parsed }});
        }};
        xhr.onerror = () => reject(new Error('网络错误'));
        xhr.open('POST', '/api/v1/push');
        xhr.send(form);
      }});
      if (!body.ok) {{
        msg.textContent = body.body.error?.message || `${{file.name}} 发送失败 ${{body.status}}`;
        break;
      }}
      ok++;
      if (prog) prog.value = Math.round((ok / files.length) * 100);
    }} catch (e) {{
      msg.textContent = String(e);
      break;
    }}
  }}
  if (ok === files.length) {{
    msg.textContent = `已推送 ${{ok}} 个文件到手机`;
    toast(`已推送 ${{ok}} 个文件`, 'ok');
    if (fileInput) fileInput.value = '';
  }} else if (ok > 0) {{
    msg.textContent = `已推送 ${{ok}}/${{files.length}} 个文件，其余失败`;
  }}
  prog?.classList.add('hidden');
  refreshDevices();
}}
const pushDrop = document.getElementById('push-drop-zone');
const pushFileInput = document.getElementById('push-file');
if (pushDrop && pushFileInput) {{
  ['dragenter', 'dragover'].forEach((ev) => {{
    pushDrop.addEventListener(ev, (e) => {{ e.preventDefault(); pushDrop.classList.add('drag-over'); }});
  }});
  ['dragleave', 'drop'].forEach((ev) => {{
    pushDrop.addEventListener(ev, (e) => {{ e.preventDefault(); pushDrop.classList.remove('drag-over'); }});
  }});
  pushDrop.addEventListener('drop', (e) => {{
    const files = Array.from(e.dataTransfer?.files || []);
    if (!files.length) return;
    pushFilesToPhone(files, null);
  }});
}}
async function cancelPush(id) {{
  if (!confirm('取消此次推送？')) return;
  const r = await fetch('/api/v1/push/' + id, {{ method: 'DELETE' }});
  const body = await r.json();
  if (!body.ok) {{ alert(body.error?.message || '取消失败'); return; }}
  refreshDevices();
}}
async function refreshAiSnapshot() {{
  const list = document.getElementById('ai-device-list');
  const jsonEl = document.getElementById('ai-snapshot-json');
  try {{
    const body = await fetch('/api/v1/agent/snapshot').then(r => r.json());
    if (!body.ok) throw new Error(body.error?.message || 'snapshot failed');
    const d = body.data || {{}};
    const set = (id, v) => {{ const el = document.getElementById(id); if (el) el.textContent = String(v); }};
    set('ai-trusted', (d.trusted || []).length);
    set('ai-pending-trust', (d.pending_trust || []).length);
    set('ai-pending-recv', d.receive?.pending_count ?? 0);
    set('ai-outbox', (d.outbox || []).length);
    if (list) {{
      const devices = d.trusted || [];
      list.innerHTML = devices.length
        ? devices.map(x => `<li class="list-item" data-device-id="${{x.device_id}}"><div class="list-item-row device-row">
            <div class="device-avatar">${{deviceAvatar(x.name)}}</div>
            <div class="device-info"><strong>${{x.name}}</strong><span>${{x.platform}} · ${{x.ip || '-'}} · ${{x.device_id}}</span></div>
          </div></li>`).join('')
        : '<li class="empty"><div class="empty-icon">📱</div>暂无已信任设备</li>';
    }}
    if (jsonEl) jsonEl.textContent = JSON.stringify(d, null, 2);
  }} catch (e) {{
    if (jsonEl) jsonEl.textContent = String(e);
  }}
}}
document.getElementById('btn-ai-refresh')?.addEventListener('click', () => refreshAiSnapshot());
setInterval(() => {{ refreshDevices(); refreshReceiveQueue(); refreshAiSnapshot(); }}, 1500);
refreshDevices();
refreshReceiveQueue();
refreshAiSnapshot();
refreshInboxSettings();
refreshRelease();
async function approveTrust(id) {{
  const r = await fetch('/api/v1/pending/' + id + '/approve', {{ method: 'POST' }});
  const body = await r.json();
  if (!body.ok) {{ toast(body.error?.message || '批准失败', 'err'); return; }}
  toast('设备已信任', 'ok');
  refreshDevices();
}}
async function rejectTrust(id) {{
  const r = await fetch('/api/v1/pending/' + id + '/reject', {{ method: 'POST' }});
  const body = await r.json();
  if (!body.ok) {{ toast(body.error?.message || '拒绝失败', 'err'); return; }}
  refreshDevices();
}}
async function revokeTrust(id) {{
  if (!confirm('撤销该设备的信任？')) return;
  const r = await fetch('/api/v1/trust/' + id, {{ method: 'DELETE' }});
  const body = await r.json();
  if (!body.ok) {{ toast(body.error?.message || '撤销失败', 'err'); return; }}
  toast('已撤销信任');
  refreshDevices();
}}
async function clearRejection(id) {{
  const r = await fetch('/api/v1/rejected/' + id, {{ method: 'DELETE' }});
  const body = await r.json();
  if (!body.ok) {{ toast(body.error?.message || '解除失败', 'err'); return; }}
  toast('已解除拒绝，手机可重新连接', 'ok');
  refreshDevices();
}}
</script>
</body>
</html>"#,
        version = crate::config::VERSION,
        platform = crate::config::PLATFORM,
        name = html_escape(&state.config.device_name),
        port = state.config.port,
        lan_ip = state
            .config
            .lan_ipv4
            .as_deref()
            .map(html_escape)
            .unwrap_or_else(|| "未检测到".into()),
        inbox = inbox,
        default_inbox = default_inbox,
        auto_accept_checked = auto_accept_checked,
        trusted_count = state.trust.store().trusted_count(),
        qr_block = qr_block,
        trust_rows = trust_rows,
        history_rows = history_rows,
    );
    Html(html)
}

async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snap = state.settings.snapshot();
    api::ok(StatusResponse {
        name: state.config.device_name.clone(),
        platform: crate::config::PLATFORM,
        version: crate::config::VERSION,
        device_id: state.config.device_id,
        features: state.config.features(),
        port: state.config.port,
        lan_ip: state.config.lan_ipv4.clone(),
        inbox_dir: snap.inbox_dir.display().to_string(),
        auto_accept: snap.auto_accept,
        trusted_devices: state.trust.store().trusted_count(),
    })
}

#[derive(Serialize)]
struct AppReleaseResponse {
    version_name: String,
    build: u32,
    display: String,
    filename: String,
    size: u64,
    download_url: String,
}

async fn app_release_info() -> impl IntoResponse {
    match crate::release::find_latest_apk() {
        Some(info) => api::ok(release_to_response(&info)).into_response(),
        None => api::err_response(
            StatusCode::NOT_FOUND,
            "APK_NOT_FOUND",
            "no hantransfer APK found under hantransfer/release",
        )
        .into_response(),
    }
}

async fn app_release_download() -> impl IntoResponse {
    let Some(info) = crate::release::find_latest_apk() else {
        return api::err_response(
            StatusCode::NOT_FOUND,
            "APK_NOT_FOUND",
            "no hantransfer APK found under hantransfer/release",
        )
        .into_response();
    };
    let path = crate::release::apk_release_dir().join(&info.filename);
    match tokio::fs::read(&path).await {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/vnd.android.package-archive"),
            )
            .header(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!("attachment; filename=\"{}\"", info.filename))
                    .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
            )
            .body(Body::from(bytes))
            .unwrap()
            .into_response(),
        Err(e) => api::err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "APK_READ_FAILED",
            format!("failed to read apk: {e}"),
        )
        .into_response(),
    }
}

fn release_to_response(info: &AppRelease) -> AppReleaseResponse {
    AppReleaseResponse {
        version_name: info.version_name.clone(),
        build: info.build,
        display: info.display.clone(),
        filename: info.filename.clone(),
        size: info.size,
        download_url: "/api/v1/app/release/download".into(),
    }
}

fn localhost_forbidden() -> Response {
    api::err_response(
        StatusCode::FORBIDDEN,
        "LOCALHOST_ONLY",
        "release management is localhost-only",
    )
    .into_response()
}

async fn app_release_list(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    if !addr.ip().is_loopback() {
        return localhost_forbidden();
    }
    match crate::release::list_all_apks() {
        Ok(list) => api::ok(list).into_response(),
        Err(e) => api::err_response(StatusCode::INTERNAL_SERVER_ERROR, "RELEASE_LIST_FAILED", e)
            .into_response(),
    }
}

async fn app_release_set_latest(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::Json(req): axum::Json<SetLatestRequest>,
) -> impl IntoResponse {
    if !addr.ip().is_loopback() {
        return localhost_forbidden();
    }
    match crate::release::set_latest_apk(req) {
        Ok(info) => api::ok(release_to_response(&info)).into_response(),
        Err(e) => api::err_response(StatusCode::BAD_REQUEST, "SET_LATEST_FAILED", e).into_response(),
    }
}

async fn app_release_upload(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if !addr.ip().is_loopback() {
        return localhost_forbidden();
    }
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = String::new();
    let mut set_latest = true;
    while let Ok(Some(mut field)) = multipart.next_field().await {
        match field.name().unwrap_or_default() {
            "file" => {
                filename = field.file_name().unwrap_or("hantransfer-upload.apk").to_string();
                match field.bytes().await {
                    Ok(b) => file_bytes = Some(b.to_vec()),
                    Err(e) => {
                        return api::err_response(
                            StatusCode::BAD_REQUEST,
                            "UPLOAD_READ_FAILED",
                            format!("read upload failed: {e}"),
                        )
                        .into_response();
                    }
                }
            }
            "set_latest" => {
                if let Ok(text) = field.text().await {
                    set_latest = text != "0" && text != "false";
                }
            }
            _ => {}
        }
    }
    let Some(bytes) = file_bytes else {
        return api::err_response(StatusCode::BAD_REQUEST, "MISSING_FILE", "missing file part")
            .into_response();
    };
    match crate::release::save_uploaded_apk(&bytes, &filename, set_latest) {
        Ok(info) => api::ok(release_to_response(&info)).into_response(),
        Err(e) => api::err_response(StatusCode::BAD_REQUEST, "UPLOAD_FAILED", e).into_response(),
    }
}

async fn list_trust(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    api::ok(state.trust.store().list_trusted())
}

#[derive(Serialize)]
struct PendingDeviceView {
    device_id: Uuid,
    name: String,
    platform: String,
    ip: String,
}

async fn list_pending(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pending: Vec<PendingDeviceView> = state
        .trust
        .list_pending()
        .into_iter()
        .map(|p| PendingDeviceView {
            device_id: p.request.device_id,
            name: p.request.name,
            platform: p.request.platform,
            ip: p.client_ip,
        })
        .collect();
    api::ok(pending)
}

async fn approve_pending(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(device_id): Path<Uuid>,
) -> Response {
    if !addr.ip().is_loopback() {
        return api::err_response(
            StatusCode::FORBIDDEN,
            "LOCALHOST_ONLY",
            "trust management is localhost-only",
        );
    }
    match state.trust.approve_by_id(&device_id) {
        Ok(()) => api::ok(serde_json::json!({ "approved": device_id })).into_response(),
        Err(err) => api::err_response(StatusCode::NOT_FOUND, "PENDING_NOT_FOUND", err),
    }
}

async fn reject_pending(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(device_id): Path<Uuid>,
) -> Response {
    if !addr.ip().is_loopback() {
        return api::err_response(
            StatusCode::FORBIDDEN,
            "LOCALHOST_ONLY",
            "trust management is localhost-only",
        );
    }
    match state.trust.reject_by_id(&device_id) {
        Ok(()) => api::ok(serde_json::json!({ "rejected": device_id })).into_response(),
        Err(err) => api::err_response(StatusCode::NOT_FOUND, "PENDING_NOT_FOUND", err),
    }
}

async fn revoke_trust(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(device_id): Path<Uuid>,
) -> Response {
    if !addr.ip().is_loopback() {
        return api::err_response(
            StatusCode::FORBIDDEN,
            "LOCALHOST_ONLY",
            "trust management is localhost-only",
        );
    }
    match state.trust.revoke(&device_id) {
        Ok(true) => api::ok(serde_json::json!({ "revoked": device_id })).into_response(),
        Ok(false) => api::err_response(
            StatusCode::NOT_FOUND,
            "DEVICE_NOT_FOUND",
            "device not in trust list",
        ),
        Err(err) => api::err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "TRUST_REVOKE_FAILED",
            err,
        ),
    }
}

async fn list_rejected(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    api::ok(state.trust.list_rejected())
}

async fn clear_rejection(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(device_id): Path<Uuid>,
) -> Response {
    if !addr.ip().is_loopback() {
        return api::err_response(
            StatusCode::FORBIDDEN,
            "LOCALHOST_ONLY",
            "trust management is localhost-only",
        );
    }
    match state.trust.clear_rejection(&device_id) {
        Ok(true) => api::ok(serde_json::json!({ "cleared": device_id })).into_response(),
        Ok(false) => api::err_response(
            StatusCode::NOT_FOUND,
            "DEVICE_NOT_FOUND",
            "device not in rejected list",
        ),
        Err(err) => api::err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CLEAR_REJECTION_FAILED",
            err,
        ),
    }
}

async fn handshake(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::Json(req): axum::Json<HandshakeRequest>,
) -> impl IntoResponse {
    let client_ip = addr.ip().to_string();
    let status = state.trust.evaluate_handshake(&req, &client_ip);
    let code = match status {
        HandshakeStatus::Trusted => StatusCode::OK,
        HandshakeStatus::Pending => StatusCode::ACCEPTED,
        HandshakeStatus::Rejected => StatusCode::FORBIDDEN,
    };
    api::ok_status(code, HandshakeResponse { status })
}

async fn check_file_duplicate(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(meta): axum::Json<FileMetadata>,
) -> impl IntoResponse {
    let device_id = match header_uuid(&headers, "x-hantransfer-device-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !state.trust.is_trusted(&device_id) {
        return api::err_response(
            StatusCode::UNAUTHORIZED,
            "DEVICE_NOT_TRUSTED",
            "device not trusted",
        );
    }
    let inbox = state.settings.inbox_dir();
    match crate::transfer::inbox_duplicate_path(&inbox, &meta) {
        Some(path) => api::ok(serde_json::json!({
            "exists": true,
            "skipped": true,
            "path": path.display().to_string(),
            "size": meta.size,
            "filename": meta.filename,
        }))
        .into_response(),
        None => api::ok(serde_json::json!({
            "exists": false,
            "skipped": false,
            "filename": meta.filename,
            "size": meta.size,
        }))
        .into_response(),
    }
}

async fn upload_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let device_id = match header_uuid(&headers, "x-hantransfer-device-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let transfer_id = match header_uuid(&headers, "x-hantransfer-transfer-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if !state.trust.is_trusted(&device_id) {
        return api::err_response(
            StatusCode::UNAUTHORIZED,
            "DEVICE_NOT_TRUSTED",
            "device not trusted",
        );
    }

    state.transfers.set(TransferProgress {
        transfer_id,
        status: TransferStatus::Receiving,
        bytes_received: 0,
        total: 0,
        filename: None,
        device_name: None,
        path: None,
        error: None,
    });

    let mut metadata: Option<FileMetadata> = None;
    let mut temp_path: Option<PathBuf> = None;
    let mut computed_hash: Option<String> = None;

    while let Ok(Some(mut field)) = multipart.next_field().await {
        match field.name().unwrap_or_default() {
            "metadata" => {
                let bytes = match field.bytes().await {
                    Ok(b) => b,
                    Err(e) => {
                        return bad_request("METADATA_READ_FAILED", format!("metadata read failed: {e}"))
                    }
                };
                metadata = match serde_json::from_slice(&bytes) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        return bad_request("INVALID_METADATA", format!("invalid metadata json: {e}"))
                    }
                };
            }
            "file" => {
                if temp_path.is_some() {
                    return bad_request("DUPLICATE_FILE_PART", "duplicate file part".into());
                }
                let path = state
                    .config
                    .temp_dir
                    .join(format!("{transfer_id}.part"));
                match stream_field_to_temp(&mut field, &path, &state.transfers, transfer_id).await
                {
                    Ok(hash) => {
                        temp_path = Some(path);
                        computed_hash = Some(hash);
                    }
                    Err(message) => return bad_request("FILE_READ_FAILED", message),
                }
            }
            _ => {}
        }
    }

    let meta = match metadata {
        Some(m) => m,
        None => return bad_request("MISSING_METADATA", "missing metadata part".into()),
    };
    let temp = match temp_path {
        Some(p) => p,
        None => return bad_request("MISSING_FILE", "missing file part".into()),
    };
    let hash = match computed_hash {
        Some(h) => h,
        None => return bad_request("MISSING_HASH", "file hash missing".into()),
    };

    state.transfers.set(TransferProgress {
        transfer_id,
        status: TransferStatus::Receiving,
        bytes_received: 0,
        total: meta.size,
        filename: Some(meta.filename.clone()),
        device_name: None,
        path: None,
        error: None,
    });

    let inbox = state.settings.inbox_dir();
    if state.settings.auto_accept() {
        match finalize_received_file(
            &state.config,
            &inbox,
            transfer_id,
            &meta,
            &temp,
            hash.clone(),
            &state.transfers,
        ) {
            Ok(path) => {
                let skipped = state
                    .transfers
                    .get(&transfer_id)
                    .and_then(|p| p.error.clone())
                    .as_deref()
                    == Some("skipped_duplicate");
                let status = if skipped {
                    Some("skipped".to_string())
                } else {
                    None
                };
                return transfer_success(transfer_id, &meta, &hash, path, status);
            }
            Err(err) => {
                tracing::warn!(
                    transfer_id = %transfer_id,
                    device_id = %device_id,
                    filename = %meta.filename,
                    reason = %err,
                    "upload failed"
                );
                return api::err_response(StatusCode::BAD_REQUEST, "UPLOAD_FAILED", err);
            }
        }
    }

    receive::queue_pending(
        &state.receive_queue,
        &state.transfers,
        transfer_id,
        device_id,
        &meta,
        temp,
        hash.clone(),
    );
    tracing::info!(
        transfer_id = %transfer_id,
        device_id = %device_id,
        filename = %meta.filename,
        size = meta.size,
        "upload queued for approval"
    );
    transfer_success(
        transfer_id,
        &meta,
        &hash,
        PathBuf::from("pending"),
        Some("pending_approval".into()),
    )
        .into_response()
}

fn transfer_success(
    transfer_id: Uuid,
    meta: &FileMetadata,
    hash: &str,
    path: PathBuf,
    status: Option<String>,
) -> Response {
    let stored_hash = if crate::transfer::client_hash_required(&meta.hash) {
        meta.hash.clone()
    } else {
        hash.to_string()
    };
    (
        StatusCode::CREATED,
        axum::Json(ApiOk {
            ok: true,
            data: TransferResult {
                transfer_id,
                path: path.display().to_string(),
                hash: stored_hash,
                status,
            },
        }),
    )
        .into_response()
}

async fn stream_field_to_temp(
    field: &mut axum::extract::multipart::Field<'_>,
    path: &std::path::Path,
    registry: &TransferRegistry,
    transfer_id: Uuid,
) -> Result<String, String> {
    let mut file = tokio::fs::File::create(path)
        .await
        .map_err(|e| format!("temp create failed: {e}"))?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;

    loop {
        match field.chunk().await {
            Ok(Some(chunk)) => {
                file.write_all(&chunk)
                    .await
                    .map_err(|e| format!("temp write failed: {e}"))?;
                hasher.update(&chunk);
                received += chunk.len() as u64;
                registry.update_bytes(&transfer_id, received);
            }
            Ok(None) => break,
            Err(e) => return Err(format!("file read failed: {e}")),
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("temp flush failed: {e}"))?;
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

async fn transfer_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.transfers.get(&id) {
        Some(progress) => api::ok(progress).into_response(),
        None => api::err_response(
            StatusCode::NOT_FOUND,
            "TRANSFER_NOT_FOUND",
            "transfer not found",
        ),
    }
}

async fn push_file(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if !addr.ip().is_loopback() {
        return api::err_response(
            StatusCode::FORBIDDEN,
            "LOCALHOST_ONLY",
            "push is localhost-only",
        )
        .into_response();
    }

    let mut target_device_id: Option<Uuid> = None;
    let mut filename = "file.bin".to_string();
    let mut temp_path: Option<PathBuf> = None;
    let mut computed_hash: Option<String> = None;
    let mut notify = true;
    let mut batch_total: Option<u32> = None;
    let push_temp_id = Uuid::new_v4();

    while let Ok(Some(mut field)) = multipart.next_field().await {
        match field.name().unwrap_or_default() {
            "target_device_id" => {
                let text = match field.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        return bad_request("TARGET_READ_FAILED", format!("target read failed: {e}"))
                    }
                };
                target_device_id = match Uuid::parse_str(text.trim()) {
                    Ok(id) => Some(id),
                    Err(_) => {
                        return bad_request("INVALID_TARGET", "invalid target_device_id".into())
                    }
                };
            }
            "file" => {
                if temp_path.is_some() {
                    return bad_request("DUPLICATE_FILE_PART", "duplicate file part".into());
                }
                filename = field.file_name().unwrap_or("file.bin").to_string();
                let path = state
                    .config
                    .temp_dir
                    .join(format!("push-{push_temp_id}.part"));
                match stream_field_to_temp(&mut field, &path, &state.transfers, push_temp_id).await
                {
                    Ok(hash) => {
                        temp_path = Some(path);
                        computed_hash = Some(hash);
                    }
                    Err(message) => return bad_request("FILE_READ_FAILED", message),
                }
            }
            "notify" => {
                let text = field.text().await.unwrap_or_default();
                notify = !matches!(text.trim(), "0" | "false" | "no");
            }
            "batch_total" => {
                if let Ok(n) = field.text().await.unwrap_or_default().trim().parse::<u32>() {
                    if n > 0 {
                        batch_total = Some(n);
                    }
                }
            }
            _ => {}
        }
    }

    let target = match target_device_id {
        Some(id) => id,
        None => return bad_request("MISSING_TARGET", "missing target_device_id".into()),
    };
    if !state.trust.is_trusted(&target) {
        return api::err_response(
            StatusCode::BAD_REQUEST,
            "DEVICE_NOT_TRUSTED",
            "target device not trusted",
        )
        .into_response();
    }
    let temp = match temp_path {
        Some(p) => p,
        None => return bad_request("MISSING_FILE", "missing file part".into()),
    };
    let hash = match computed_hash {
        Some(h) => h,
        None => return bad_request("MISSING_HASH", "file hash missing".into()),
    };

    match state.outbox.create_from_temp(
        target,
        &filename,
        &state.config.device_name,
        &temp,
        hash,
    ) {
        Ok(entry) => {
            let device_name = state
                .trust
                .store()
                .list_trusted()
                .into_iter()
                .find(|d| d.device_id == target)
                .map(|d| d.name)
                .unwrap_or_else(|| "手机".to_string());
            if notify {
                if let Some(n) = batch_total.filter(|&n| n > 1) {
                    crate::notify::notify_push_batch(&device_name, n as usize);
                } else {
                    crate::notify::notify_push_queued(&device_name, &entry.filename);
                }
            }
            tracing::info!(
                push_id = %entry.push_id,
                target = %target,
                filename = %entry.filename,
                size = entry.size,
                "push queued for phone"
            );
            api::ok(entry).into_response()
        }
        Err(err) => api::err_response(StatusCode::INTERNAL_SERVER_ERROR, "PUSH_FAILED", err)
            .into_response(),
    }
}

async fn push_pending(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let device_id = match header_uuid(&headers, "x-hantransfer-device-id") {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !state.trust.is_trusted(&device_id) {
        return api::err_response(
            StatusCode::UNAUTHORIZED,
            "DEVICE_NOT_TRUSTED",
            "device not trusted",
        );
    }
    let pending = state.outbox.list_pending_for(&device_id);
    api::ok(pending).into_response()
}

async fn push_outbox(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    if !addr.ip().is_loopback() {
        return api::err_response(
            StatusCode::FORBIDDEN,
            "LOCALHOST_ONLY",
            "outbox is localhost-only",
        );
    }
    api::ok(state.outbox.list_all()).into_response()
}

async fn push_cancel(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(push_id): Path<Uuid>,
) -> impl IntoResponse {
    if !addr.ip().is_loopback() {
        return api::err_response(
            StatusCode::FORBIDDEN,
            "LOCALHOST_ONLY",
            "push cancel is localhost-only",
        )
        .into_response();
    }
    match state.outbox.cancel(&push_id) {
        Ok(entry) => {
            tracing::info!(push_id = %push_id, filename = %entry.filename, "push cancelled");
            api::ok(serde_json::json!({ "cancelled": push_id })).into_response()
        }
        Err(err) => api::err_response(StatusCode::NOT_FOUND, "PUSH_NOT_FOUND", err)
            .into_response(),
    }
}

async fn push_download(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<DeviceIdQuery>,
    Path(push_id): Path<Uuid>,
) -> Response {
    let device_id = match device_id_from(&headers, query.device_id.as_deref()) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !state.trust.is_trusted(&device_id) {
        return api::err_response(
            StatusCode::UNAUTHORIZED,
            "DEVICE_NOT_TRUSTED",
            "device not trusted",
        );
    }
    let entry = match state.outbox.get(&push_id) {
        Some(e) => e,
        None => {
            return api::err_response(
                StatusCode::NOT_FOUND,
                "PUSH_NOT_FOUND",
                "push not found",
            )
        }
    };
    if entry.target_device_id != device_id {
        return api::err_response(
            StatusCode::FORBIDDEN,
            "PUSH_NOT_FOR_DEVICE",
            "push not for this device",
        );
    }
    let path = state.outbox.file_path(&push_id);
    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => {
            return api::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PUSH_READ_FAILED",
                format!("read push file failed: {e}"),
            )
        }
    };
    let stream = ReaderStream::new(file);
    let disposition = format!(
        "attachment; filename=\"{}\"",
        entry.filename.replace('"', "_")
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(
            axum::http::header::CONTENT_TYPE,
            "application/octet-stream",
        )
        .header(axum::http::header::CONTENT_DISPOSITION, disposition)
        .header(axum::http::header::CONTENT_LENGTH, entry.size)
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| {
            api::err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PUSH_RESPONSE_FAILED",
                "failed to build response",
            )
        })
}

async fn push_ack(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<DeviceIdQuery>,
    Path(push_id): Path<Uuid>,
) -> impl IntoResponse {
    let device_id = match device_id_from(&headers, query.device_id.as_deref()) {
        Ok(id) => id,
        Err(resp) => return resp.into_response(),
    };
    if !state.trust.is_trusted(&device_id) {
        return api::err_response(
            StatusCode::UNAUTHORIZED,
            "DEVICE_NOT_TRUSTED",
            "device not trusted",
        )
        .into_response();
    }
    match state.outbox.acknowledge(&push_id, &device_id) {
        Ok(entry) => {
            log_push_history(&state.config.history_dir, &entry);
            tracing::info!(
                push_id = %push_id,
                device_id = %device_id,
                filename = %entry.filename,
                "push acknowledged"
            );
            api::ok(serde_json::json!({ "acknowledged": push_id })).into_response()
        }
        Err(err) => api::err_response(StatusCode::BAD_REQUEST, "PUSH_ACK_FAILED", err)
            .into_response(),
    }
}

fn log_push_history(history_dir: &std::path::Path, entry: &PushEntry) {
    let file = history_dir.join("transfers.jsonl");
    let line = serde_json::json!({
        "transfer_id": entry.push_id,
        "filename": entry.filename,
        "size": entry.size,
        "path": format!("phone:{}", entry.filename),
        "type": "push",
        "direction": "pc_to_phone",
        "target_device_id": entry.target_device_id,
        "at": entry.created_at,
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

fn header_uuid(
    headers: &HeaderMap,
    name: &str,
) -> Result<Uuid, Response> {
    let raw = headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            api::err_response(
                StatusCode::BAD_REQUEST,
                "MISSING_HEADER",
                format!("missing header {name}"),
            )
        })?;
    Uuid::parse_str(raw).map_err(|_| {
        api::err_response(
            StatusCode::BAD_REQUEST,
            "INVALID_HEADER",
            format!("invalid uuid in {name}"),
        )
    })
}

#[derive(Debug, Deserialize)]
struct DeviceIdQuery {
    device_id: Option<String>,
}

fn device_id_from(headers: &HeaderMap, query_device_id: Option<&str>) -> Result<Uuid, Response> {
    if let Some(raw) = headers
        .get("x-hantransfer-device-id")
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(id) = Uuid::parse_str(raw) {
            return Ok(id);
        }
        return Err(api::err_response(
            StatusCode::BAD_REQUEST,
            "INVALID_HEADER",
            "invalid uuid in x-hantransfer-device-id",
        ));
    }
    if let Some(raw) = query_device_id {
        return Uuid::parse_str(raw).map_err(|_| {
            api::err_response(
                StatusCode::BAD_REQUEST,
                "INVALID_DEVICE_ID",
                "invalid device_id query parameter",
            )
        });
    }
    Err(api::err_response(
        StatusCode::BAD_REQUEST,
        "MISSING_DEVICE_ID",
        "missing X-Hantransfer-Device-ID header or device_id query parameter",
    ))
}

fn bad_request(code: &str, message: String) -> Response {
    api::err_response(StatusCode::BAD_REQUEST, code, message)
}

#[derive(Serialize)]
struct ReceiveQueueResponse {
    receiving: Vec<TransferProgress>,
    pending: Vec<PendingReceiveView>,
}

async fn receive_queue_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pending: Vec<PendingReceiveView> = state
        .receive_queue
        .list()
        .iter()
        .map(|item| receive::pending_view(item, &state.trust))
        .collect();
    api::ok(ReceiveQueueResponse {
        receiving: state.transfers.list_active(),
        pending,
    })
}

async fn receive_accept(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.receive_queue.accept(
        &state.config,
        &state.settings,
        &state.transfers,
        &state.trust,
        &id,
    ) {
        Ok(path) => api::ok(TransferResult {
            transfer_id: id,
            path: path.display().to_string(),
            hash: String::new(),
            status: Some("done".into()),
        })
        .into_response(),
        Err(err) => api::err_response(StatusCode::BAD_REQUEST, "ACCEPT_FAILED", err).into_response(),
    }
}

async fn receive_reject(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.receive_queue.reject(&state.transfers, &id) {
        Ok(()) => api::ok(serde_json::json!({ "rejected": id })).into_response(),
        Err(err) => api::err_response(StatusCode::BAD_REQUEST, "REJECT_FAILED", err).into_response(),
    }
}

async fn receive_accept_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let results = state.receive_queue.accept_all(
        &state.config,
        &state.settings,
        &state.transfers,
        &state.trust,
    );
    let accepted = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.len().saturating_sub(accepted);
    api::ok(serde_json::json!({ "accepted": accepted, "failed": failed }))
}

#[derive(Serialize)]
struct SettingsResponse {
    inbox_dir: String,
    auto_accept: bool,
    default_inbox_dir: String,
}

async fn settings_get(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snap = state.settings.snapshot();
    api::ok(SettingsResponse {
        inbox_dir: snap.inbox_dir.display().to_string(),
        auto_accept: snap.auto_accept,
        default_inbox_dir: crate::paths::model_inbox().display().to_string(),
    })
}

#[derive(Deserialize)]
struct SettingsUpdateRequest {
    inbox_dir: Option<String>,
    auto_accept: Option<bool>,
}

async fn settings_update(
    State(state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<SettingsUpdateRequest>,
) -> impl IntoResponse {
    let inbox = body
        .inbox_dir
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty());
    match state.settings.update(inbox, body.auto_accept) {
        Ok(snap) => {
            if let Err(err) = state.settings.ensure_inbox_dir() {
                return api::err_response(StatusCode::BAD_REQUEST, "INBOX_CREATE_FAILED", err)
                    .into_response();
            }
            api::ok(SettingsResponse {
                inbox_dir: snap.inbox_dir.display().to_string(),
                auto_accept: snap.auto_accept,
                default_inbox_dir: crate::paths::model_inbox().display().to_string(),
            })
            .into_response()
        }
        Err(err) => api::err_response(StatusCode::BAD_REQUEST, "SETTINGS_INVALID", err).into_response(),
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn qr_svg(url: &str) -> String {
    let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else {
        return String::new();
    };
    code.render()
        .min_dimensions(180, 180)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build()
}

fn read_recent_history(history_dir: &std::path::Path, limit: usize) -> Vec<String> {
    let file = history_dir.join("transfers.jsonl");
    let Ok(raw) = std::fs::read_to_string(&file) else {
        return Vec::new();
    };
    raw.lines()
        .rev()
        .take(limit)
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            Some(format!(
                "{} · {} → {}",
                value.get("filename")?.as_str()?,
                value.get("size")?.as_u64()?,
                value.get("path")?.as_str()?
            ))
        })
        .collect()
}
