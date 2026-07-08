//! 本地 Agent HTTP 服务（127.0.0.1:1421），供 Cursor 等外部工具调用。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::state::AppState;

pub const AGENT_PORT: u16 = 1421;

#[derive(Debug, Clone, Serialize)]
pub struct AgentStatus {
    pub enabled: bool,
    pub running: bool,
    pub port: u16,
    pub base_url: String,
}

struct AgentRuntime {
    enabled: AtomicBool,
    shutdown_tx: Mutex<Option<watch::Sender<bool>>>,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl AgentRuntime {
    fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            shutdown_tx: Mutex::new(None),
            task: Mutex::new(None),
        }
    }
}

static RUNTIME: std::sync::OnceLock<AgentRuntime> = std::sync::OnceLock::new();

fn runtime() -> &'static AgentRuntime {
    RUNTIME.get_or_init(AgentRuntime::new)
}

pub fn status(_st: Arc<AppState>) -> AgentStatus {
    let rt = runtime();
    let enabled = rt.enabled.load(Ordering::Relaxed);
    let running = rt.task.lock().map(|g| g.is_some()).unwrap_or(false);
    AgentStatus {
        enabled,
        running,
        port: AGENT_PORT,
        base_url: format!("http://127.0.0.1:{AGENT_PORT}"),
    }
}

pub fn set_enabled(st: Arc<AppState>, enabled: bool) -> Result<AgentStatus, String> {
    let rt = runtime();
    if enabled {
        start_server(st.clone())?;
        rt.enabled.store(true, Ordering::Relaxed);
        let db = st.lock_db()?;
        crate::db::set_setting(&db, "agent_http_enabled", "1").map_err(|e| e.to_string())?;
    } else {
        stop_server();
        rt.enabled.store(false, Ordering::Relaxed);
        let db = st.lock_db()?;
        crate::db::set_setting(&db, "agent_http_enabled", "0").map_err(|e| e.to_string())?;
    }
    Ok(status(st))
}

pub fn restore_on_startup(st: Arc<AppState>) {
    let enabled = st
        .lock_db()
        .ok()
        .and_then(|db| crate::db::get_setting(&db, "agent_http_enabled"))
        .map(|v| v == "1")
        .unwrap_or(false);
    if enabled {
        let _ = set_enabled(st, true);
    }
}

fn start_server(st: Arc<AppState>) -> Result<(), String> {
    let rt = runtime();
    if rt.task.lock().map(|g| g.is_some()).unwrap_or(false) {
        return Ok(());
    }
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    *rt.shutdown_tx.lock().map_err(|e| e.to_string())? = Some(shutdown_tx);
    let handle = tauri::async_runtime::spawn(async move {
        if let Err(e) = run_server(st, &mut shutdown_rx).await {
            crate::log::warn(&format!("Agent HTTP 服务退出: {e}"));
        }
    });
    *rt.task.lock().map_err(|e| e.to_string())? = Some(handle);
    Ok(())
}

fn stop_server() {
    let rt = runtime();
    if let Ok(mut tx_slot) = rt.shutdown_tx.lock() {
        if let Some(tx) = tx_slot.take() {
            let _ = tx.send(true);
        }
    }
    if let Ok(mut task_slot) = rt.task.lock() {
        if let Some(handle) = task_slot.take() {
            handle.abort();
        }
    }
}

/// 应用退出时停止 Agent HTTP，避免后台任务拖慢进程结束
pub fn stop_on_exit() {
    stop_server();
    runtime().enabled.store(false, Ordering::Relaxed);
}

async fn run_server(st: Arc<AppState>, shutdown: &mut watch::Receiver<bool>) -> Result<(), String> {
    let addr = format!("127.0.0.1:{AGENT_PORT}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Agent 端口 {AGENT_PORT} 绑定失败: {e}"))?;
    crate::log::info(&format!("Agent HTTP 已监听 {addr}"));

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
            accept = listener.accept() => {
                let (mut stream, _) = accept.map_err(|e| e.to_string())?;
                let st = Arc::clone(&st);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(st, &mut stream).await {
                        crate::log::warn(&format!("Agent 请求处理失败: {e}"));
                    }
                });
            }
        }
    }
    Ok(())
}

async fn handle_connection(
    st: Arc<AppState>,
    stream: &mut tokio::net::TcpStream,
) -> Result<(), String> {
    let mut buf = vec![0u8; 65536];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    let (status, body) = route_request(st, method, path, &req).await;
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn route_request(
    st: Arc<AppState>,
    method: &str,
    path: &str,
    raw: &str,
) -> (u16, String) {
    let path_only = path.split('?').next().unwrap_or(path);
    match (method, path_only) {
        ("GET", "/") | ("GET", "/api") => (
            200,
            serde_json::to_string(&serde_json::json!({
                "name": "小寒日报 Agent API",
                "version": 1,
                "endpoints": [
                    "GET /api/status",
                    "GET /api/personas",
                    "POST /api/personas/{id}/regenerate",
                    "POST /api/personas/batch-regenerate?limit=1&only_missing=true"
                ]
            }))
            .unwrap_or_else(|_| "{}".into()),
        ),
        ("GET", "/api/status") => {
            let s = status(st);
            (200, serde_json::to_string(&s).unwrap_or_else(|_| "{}".into()))
        }
        ("GET", "/api/personas") => {
            let data_dir = st.data_dir().to_path_buf();
            let list = {
                let db = match st.lock_db() {
                    Ok(db) => db,
                    Err(e) => return (500, json_err(&e)),
                };
                crate::persona::list_personas(&data_dir, &db)
            };
            (200, serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()))
        }
        ("GET", "/api/personas/regenerate-pending") => {
            let data_dir = st.data_dir();
            let only_missing = parse_query_bool(raw, "only_missing").unwrap_or(true);
            let count = crate::persona::import_reference::count_pending_regenerate(
                data_dir,
                only_missing,
            );
            (
                200,
                serde_json::to_string(&serde_json::json!({ "pending": count, "only_missing": only_missing }))
                    .unwrap_or_else(|_| "{}".into()),
            )
        }
        ("POST", p) if p.starts_with("/api/personas/") && p.ends_with("/regenerate") => {
            let id = p
                .trim_start_matches("/api/personas/")
                .trim_end_matches("/regenerate")
                .trim_matches('/');
            if id.is_empty() {
                return (400, json_err("缺少 persona id"));
            }
            let data_dir = st.data_dir();
            let ctx = crate::persona::import_reference::ImportReferenceContext {
                data_dir,
                db: &st.db,
                vault: &st.vault,
                app: None,
            };
            let _guard = match crate::persona::import_reference::try_acquire_persona_ai_batch() {
                Ok(g) => g,
                Err(e) => return (409, json_err(&e)),
            };
            match crate::persona::import_reference::regenerate_persona_profile(&ctx, id).await {
                Ok(r) => (200, serde_json::to_string(&r).unwrap_or_else(|_| "{}".into())),
                Err(e) => (400, json_err(&e)),
            }
        }
        ("POST", "/api/personas/batch-regenerate") => {
            let limit = parse_query_u64(raw, "limit").unwrap_or(1).min(50) as usize;
            let only_missing = parse_query_bool(raw, "only_missing").unwrap_or(true);
            let data_dir = st.data_dir();
            let ctx = crate::persona::import_reference::ImportReferenceContext {
                data_dir,
                db: &st.db,
                vault: &st.vault,
                app: None,
            };
            match crate::persona::import_reference::batch_regenerate_persona_profiles(
                &ctx, limit, only_missing,
            )
            .await
            {
                Ok(r) => (200, serde_json::to_string(&r).unwrap_or_else(|_| "{}".into())),
                Err(e) if e.contains("已有性格 AI 任务") => (409, json_err(&e)),
                Err(e) => (400, json_err(&e)),
            }
        }
        ("OPTIONS", _) => (204, String::new()),
        _ => (404, json_err("not found")),
    }
}

fn json_err(msg: &str) -> String {
    serde_json::json!({ "code": 1, "message": msg }).to_string()
}

fn parse_query_u64(raw: &str, key: &str) -> Option<u64> {
    let query = raw.lines().next()?;
    let query = query.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next()? == key {
            return kv.next()?.parse().ok();
        }
    }
    None
}

fn parse_query_bool(raw: &str, key: &str) -> Option<bool> {
    let v = parse_query_u64(raw, key).map(|n| n != 0);
    if v.is_some() {
        return v;
    }
    let query = raw.lines().next()?.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next()? == key {
            return match kv.next()? {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            };
        }
    }
    None
}
