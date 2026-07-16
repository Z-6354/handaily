//! Localhost-only helpers for Cursor / AI agents.

use std::fs::File;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::api;
use crate::outbox::PushEntry;
use crate::receive;
use crate::server::AppState;

#[derive(Serialize)]
pub struct AgentSnapshot {
    pub online: bool,
    pub port: u16,
    pub lan_ip: String,
    pub device_name: String,
    pub trusted: Vec<AgentDevice>,
    pub pending_trust: Vec<AgentPendingTrust>,
    pub rejected: Vec<Uuid>,
    pub receive: AgentReceiveSummary,
    pub outbox: Vec<AgentOutboxItem>,
}

#[derive(Serialize)]
pub struct AgentDevice {
    pub device_id: Uuid,
    pub name: String,
    pub platform: String,
    pub ip: Option<String>,
}

#[derive(Serialize)]
pub struct AgentPendingTrust {
    pub device_id: Uuid,
    pub name: String,
    pub platform: String,
    pub ip: String,
}

#[derive(Serialize)]
pub struct AgentReceiveSummary {
    pub pending_count: usize,
    pub receiving_count: usize,
    pub pending: Vec<receive::PendingReceiveView>,
}

#[derive(Serialize)]
pub struct AgentOutboxItem {
    pub push_id: Uuid,
    pub target_device_id: Uuid,
    pub filename: String,
    pub size: u64,
}

fn forbid_non_loopback(addr: SocketAddr) -> Option<Response> {
    if addr.ip().is_loopback() {
        None
    } else {
        Some(api::err_response(
            StatusCode::FORBIDDEN,
            "LOCALHOST_ONLY",
            "agent API is localhost-only",
        ))
    }
}

pub async fn snapshot(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    if let Some(resp) = forbid_non_loopback(addr) {
        return resp;
    }
    let trusted = state
        .trust
        .store()
        .list_trusted()
        .into_iter()
        .map(|d| AgentDevice {
            device_id: d.device_id,
            name: d.name,
            platform: d.platform,
            ip: d.ip,
        })
        .collect();
    let pending_trust = state
        .trust
        .list_pending()
        .into_iter()
        .map(|p| AgentPendingTrust {
            device_id: p.request.device_id,
            name: p.request.name,
            platform: p.request.platform,
            ip: p.client_ip,
        })
        .collect();
    let pending_receive: Vec<_> = state
        .receive_queue
        .list()
        .iter()
        .map(|item| receive::pending_view(item, &state.trust))
        .collect();
    let outbox = state
        .outbox
        .list_all()
        .into_iter()
        .map(|o| AgentOutboxItem {
            push_id: o.push_id,
            target_device_id: o.target_device_id,
            filename: o.filename,
            size: o.size,
        })
        .collect();
    let lan_ip = crate::netutil::primary_lan_ipv4().unwrap_or_else(|| "127.0.0.1".into());
    api::ok(AgentSnapshot {
        online: true,
        port: state.config.port,
        lan_ip,
        device_name: state.config.device_name.clone(),
        trusted,
        pending_trust,
        rejected: state.trust.list_rejected(),
        receive: AgentReceiveSummary {
            pending_count: pending_receive.len(),
            receiving_count: state.transfers.list_active().len(),
            pending: pending_receive,
        },
        outbox,
    })
    .into_response()
}

#[derive(Deserialize)]
pub struct AgentPushBody {
    pub device_id: Uuid,
    pub paths: Vec<String>,
}

#[derive(Serialize)]
pub struct AgentPushResult {
    pub pushed: Vec<PushEntry>,
    pub failed: Vec<AgentPushFailure>,
}

#[derive(Serialize)]
pub struct AgentPushFailure {
    pub path: String,
    pub error: String,
}

pub async fn push_paths(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<AgentPushBody>,
) -> Response {
    if let Some(resp) = forbid_non_loopback(addr) {
        return resp;
    }
    if !state.trust.is_trusted(&body.device_id) {
        return api::err_response(
            StatusCode::BAD_REQUEST,
            "DEVICE_NOT_TRUSTED",
            "target device not trusted",
        );
    }
    if body.paths.is_empty() {
        return api::err_response(
            StatusCode::BAD_REQUEST,
            "MISSING_PATHS",
            "paths must not be empty",
        );
    }

    let device_name = state
        .trust
        .store()
        .list_trusted()
        .into_iter()
        .find(|d| d.device_id == body.device_id)
        .map(|d| d.name)
        .unwrap_or_else(|| "手机".to_string());

    let mut pushed = Vec::new();
    let mut failed = Vec::new();
    let total = body.paths.len();

    for (i, raw) in body.paths.iter().enumerate() {
        let path = PathBuf::from(raw);
        match push_one_path(&state, body.device_id, &path) {
            Ok(entry) => {
                if i + 1 == total {
                    if total > 1 {
                        crate::notify::notify_push_batch(&device_name, total);
                    } else {
                        crate::notify::notify_push_queued(&device_name, &entry.filename);
                    }
                }
                pushed.push(entry);
            }
            Err(err) => failed.push(AgentPushFailure {
                path: raw.clone(),
                error: err,
            }),
        }
    }

    api::ok(AgentPushResult { pushed, failed }).into_response()
}

fn push_one_path(
    state: &AppState,
    target: Uuid,
    path: &Path,
) -> Result<PushEntry, String> {
    if !path.is_file() {
        return Err(format!("not a file: {}", path.display()));
    }
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file.bin")
        .to_string();
    let temp = state
        .config
        .temp_dir
        .join(format!("agent-push-{}.part", Uuid::new_v4()));
    std::fs::copy(path, &temp).map_err(|e| format!("copy failed: {e}"))?;
    let hash = hash_file(&temp)?;
    state.outbox.create_from_temp(
        target,
        &filename,
        &state.config.device_name,
        &temp,
        hash,
    )
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

#[derive(Deserialize)]
pub struct AgentAcceptBody {
    /// When set, accept one transfer; otherwise accept all.
    pub id: Option<Uuid>,
}

pub async fn receive_accept(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<AgentAcceptBody>,
) -> Response {
    if let Some(resp) = forbid_non_loopback(addr) {
        return resp;
    }
    if let Some(id) = body.id {
        return match state.receive_queue.accept(
            &state.config,
            &state.settings,
            &state.transfers,
            &state.trust,
            &id,
        ) {
            Ok(path) => api::ok(serde_json::json!({
                "accepted": 1,
                "failed": 0,
                "path": path.display().to_string(),
                "id": id,
            }))
            .into_response(),
            Err(err) => {
                api::err_response(StatusCode::BAD_REQUEST, "ACCEPT_FAILED", err).into_response()
            }
        };
    }
    let results = state.receive_queue.accept_all(
        &state.config,
        &state.settings,
        &state.transfers,
        &state.trust,
    );
    let accepted = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.len().saturating_sub(accepted);
    api::ok(serde_json::json!({ "accepted": accepted, "failed": failed })).into_response()
}
