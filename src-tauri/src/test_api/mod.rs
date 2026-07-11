//! Debug 构建专用 HTTP 测试 API（127.0.0.1），供 AI / 脚本驱动桌宠操作。
//! 设置 `HANDAILY_DISABLE_TEST_API=1` 可关闭；`HANDAILY_TEST_API_PORT` 改端口（默认 19420）。

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Instant;

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::state::AppState;

static TEST_API_CTX: OnceLock<(AppHandle, Arc<AppState>)> = OnceLock::new();

const DEFAULT_PORT: u16 = 19420;

pub fn spawn_server(app: AppHandle, st: Arc<AppState>) {
    if std::env::var("HANDAILY_DISABLE_TEST_API").is_ok() {
        crate::log::info("test-api: disabled (HANDAILY_DISABLE_TEST_API)");
        return;
    }
    let _ = TEST_API_CTX.set((app, st));
    thread::Builder::new()
        .name("handaily-test-api".into())
        .spawn(server_loop)
        .expect("test-api thread");
}

fn server_loop() {
    let port = std::env::var("HANDAILY_TEST_API_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let addr = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            crate::log::warn(format!("test-api: bind {addr} failed: {e}"));
            return;
        }
    };
    crate::log::info(format!("test-api: listening on http://{addr}"));
    for conn in listener.incoming().flatten() {
        let _ = conn.set_read_timeout(Some(std::time::Duration::from_secs(30)));
        let _ = conn.set_write_timeout(Some(std::time::Duration::from_secs(30)));
        if let Err(e) = handle_connection(conn) {
            crate::log::warn(format!("test-api: request error: {e}"));
        }
    }
}

fn handle_connection(mut stream: TcpStream) -> Result<(), String> {
    let conn_t0 = Instant::now();
    let mut buf = vec![0u8; 64 * 1024];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(());
    }
    let raw = String::from_utf8_lossy(&buf[..n]);
    let mut lines = raw.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return write_json(&mut stream, 400, json!({ "ok": false, "error": "bad request" }));
    }
    let method = parts[0];
    let full_path = parts[1];
    let path = full_path.split('?').next().unwrap_or(full_path);
    let query = full_path.split('?').nth(1).unwrap_or("");

    let mut content_length = 0usize;
    for line in lines.by_ref() {
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
    }
    let header_end = raw.find("\r\n\r\n").unwrap_or(n);
    let body = if content_length > 0 && header_end + 4 + content_length <= n {
        raw[header_end + 4..header_end + 4 + content_length].to_string()
    } else {
        String::new()
    };

    let (status, payload) = match route(method, path, query, &body) {
        Ok(v) => v,
        Err(e) => (500, json!({ "ok": false, "error": e })),
    };
    let route_ms = conn_t0.elapsed().as_millis();
    let out = write_json(&mut stream, status, payload);
    eprintln!(
        "xiaohan-daily: test-api {method} {path} route={route_ms}ms write={}ms",
        conn_t0.elapsed().as_millis() - route_ms
    );
    out
}

fn write_json(stream: &mut TcpStream, status: u16, body: Value) -> Result<(), String> {
    let text = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n{text}",
        text.len()
    );
    stream.write_all(response.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

fn ctx() -> Result<(&'static AppHandle, &'static Arc<AppState>), String> {
    TEST_API_CTX
        .get()
        .map(|(a, s)| (a, s))
        .ok_or_else(|| "test-api context not ready".into())
}

/// 在测试 API 线程同步执行菜单切换（内部用 std mpsc 等待，不占用 Tauri runtime）。
fn run_menu_switch_blocking(
    app: &AppHandle,
    st: Arc<AppState>,
    character_id: &str,
    skin_id: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let raw_id = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::character::select_character_skin(st.data_dir(), &db, character_id, skin_id)?
    };
    crate::pet::menu_switch_to_model_blocking(app, st, &raw_id, timeout_ms)
}

fn run_menu_switch_character_blocking(
    app: &AppHandle,
    st: Arc<AppState>,
    character_id: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let raw_id = {
        let db = crate::db::lock_conn(&st.db)?;
        let manifest = crate::persona::load_manifest(st.data_dir());
        crate::character::set_active_character(st.data_dir(), &db, &manifest, character_id)?;
        crate::pet::models::active_model_id(&db)
    };
    crate::pet::menu_switch_to_model_blocking(app, st, &raw_id, timeout_ms)
}

fn route(method: &str, path: &str, query: &str, body: &str) -> Result<(u16, Value), String> {
    match (method, path) {
        ("GET", "/health") => Ok((
            200,
            json!({
                "ok": true,
                "service": "handaily-test-api",
                "build": "debug",
            }),
        )),
        ("GET", "/") => Ok((200, api_index_json())),
        ("GET", "/pet/snapshot") => {
            let (app, st) = ctx()?;
            let snap = crate::pet::test_snapshot(app, st)?;
            Ok((200, json!({ "ok": true, "snapshot": snap })))
        }
        ("GET", "/pet/status") => {
            let (app, st) = ctx()?;
            let status = crate::pet::status(app, st)?;
            Ok((200, json!({ "ok": true, "status": status })))
        }
        ("GET", "/pet/menu/skins") => {
            let (_app, st) = ctx()?;
            let (character_id, active_model_id) = {
                let db = crate::db::lock_conn(&st.db)?;
                (
                    crate::character::active_character_id(&db, st.data_dir()),
                    crate::pet::models::active_model_id(&db),
                )
            };
            let payload = crate::character::build_pet_menu_skins_payload(
                st.data_dir(),
                &character_id,
                &active_model_id,
            )?;
            Ok((200, json!({ "ok": true, "menu": payload })))
        }
        ("GET", "/pet/characters/favorites") => {
            let (_app, st) = ctx()?;
            let list = {
                let db = crate::db::lock_conn(&st.db)?;
                crate::character::list_pet_menu_favorite_characters(st.data_dir(), &db)
            };
            Ok((200, json!({ "ok": true, "characters": list })))
        }
        ("GET", "/pet/characters") => {
            let (_app, st) = ctx()?;
            let db = crate::db::lock_conn(&st.db)?;
            let list = crate::character::list_characters(st.data_dir(), &db);
            Ok((200, json!({ "ok": true, "characters": list })))
        }
        ("GET", "/pet/logs/tail") => {
            let (_app, st) = ctx()?;
            let n = parse_query_usize(query, "n", 40).clamp(1, 200);
            let lines = tail_display_logs(st.data_dir(), n)?;
            Ok((200, json!({ "ok": true, "lines": lines })))
        }
        ("POST", "/pet/switch/skin") => {
            let input: SwitchSkinBody = parse_body(body)?;
            let (app, st) = ctx()?;
            let timeout = input.timeout_ms.unwrap_or(30_000);
            let model_id = run_menu_switch_blocking(
                app,
                st.clone(),
                &input.character_id,
                &input.skin_id,
                timeout,
            )?;
            let snap = crate::pet::test_snapshot_light(app, &model_id)?;
            Ok((
                200,
                json!({
                    "ok": true,
                    "model_id": model_id,
                    "snapshot": snap,
                }),
            ))
        }
        ("POST", "/pet/switch/character") => {
            let input: SwitchCharacterBody = parse_body(body)?;
            let (app, st) = ctx()?;
            let timeout = input.timeout_ms.unwrap_or(30_000);
            let model_id = run_menu_switch_character_blocking(
                app,
                st.clone(),
                &input.character_id,
                timeout,
            )?;
            let snap = crate::pet::test_snapshot_light(app, &model_id)?;
            Ok((
                200,
                json!({
                    "ok": true,
                    "model_id": model_id,
                    "snapshot": snap,
                }),
            ))
        }
        ("POST", "/pet/switch/next-skin") => {
            let input: NextSkinBody = parse_body(body).unwrap_or(NextSkinBody {
                timeout_ms: None,
            });
            let (app, st) = ctx()?;
            let t0 = Instant::now();
            let (character_id, skin_id) = pick_next_skin(st)?;
            let pick_ms = t0.elapsed().as_millis();
            let timeout = input.timeout_ms.unwrap_or(30_000);
            let t1 = Instant::now();
            let model_id = run_menu_switch_blocking(app, st.clone(), &character_id, &skin_id, timeout)?;
            let switch_ms = t1.elapsed().as_millis();
            let t2 = Instant::now();
            let snap = crate::pet::test_snapshot_light(app, &model_id)?;
            let snap_ms = t2.elapsed().as_millis();
            eprintln!(
                "xiaohan-daily: test-api switch next-skin pick={pick_ms}ms switch={switch_ms}ms snapshot={snap_ms}ms model={model_id}"
            );
            Ok((
                200,
                json!({
                    "ok": true,
                    "character_id": character_id,
                    "skin_id": skin_id,
                    "model_id": model_id,
                    "snapshot": snap,
                }),
            ))
        }
        ("POST", "/pet/switch/next-character") => {
            let input: NextCharacterBody = parse_body(body).unwrap_or(NextCharacterBody {
                timeout_ms: None,
            });
            let (app, st) = ctx()?;
            let t0 = Instant::now();
            let (character_id, skin_id) = pick_next_favorite_character_skin(st)?;
            let pick_ms = t0.elapsed().as_millis();
            let timeout = input.timeout_ms.unwrap_or(30_000);
            let t1 = Instant::now();
            let model_id =
                run_menu_switch_blocking(app, st.clone(), &character_id, &skin_id, timeout)?;
            let switch_ms = t1.elapsed().as_millis();
            let t2 = Instant::now();
            let snap = crate::pet::test_snapshot_light(app, &model_id)?;
            let snap_ms = t2.elapsed().as_millis();
            eprintln!(
                "xiaohan-daily: test-api switch next-character pick={pick_ms}ms switch={switch_ms}ms snapshot={snap_ms}ms character={character_id} skin={skin_id} model={model_id}"
            );
            Ok((
                200,
                json!({
                    "ok": true,
                    "character_id": character_id,
                    "skin_id": skin_id,
                    "model_id": model_id,
                    "timing_ms": {
                        "pick": pick_ms,
                        "switch": switch_ms,
                        "snapshot": snap_ms,
                    },
                    "snapshot": snap,
                }),
            ))
        }
        ("POST", "/pet/menu/open") => {
            let (app, _st) = ctx()?;
            crate::pet::show_pet_menu_at_cursor(app)?;
            Ok((200, json!({ "ok": true })))
        }
        ("POST", "/pet/menu/hide") => {
            let (app, _st) = ctx()?;
            crate::pet::hide_pet_menu(app)?;
            Ok((200, json!({ "ok": true })))
        }
        ("POST", "/app/exit") => {
            let (app, st) = ctx()?;
            crate::request_app_exit(app, st);
            Ok((200, json!({ "ok": true, "exiting": true })))
        }
        _ => Ok((404, json!({ "ok": false, "error": "not found", "path": path }))),
    }
}

fn parse_body<T: for<'de> Deserialize<'de>>(body: &str) -> Result<T, String> {
    if body.trim().is_empty() {
        return Err("empty JSON body".into());
    }
    serde_json::from_str(body).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct SwitchSkinBody {
    character_id: String,
    skin_id: String,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct SwitchCharacterBody {
    character_id: String,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize, Default)]
struct NextSkinBody {
    timeout_ms: Option<u64>,
}

#[derive(Deserialize, Default)]
struct NextCharacterBody {
    timeout_ms: Option<u64>,
}

fn pick_next_skin(st: &AppState) -> Result<(String, String), String> {
    let (character_id, active_model_id) = {
        let db = crate::db::lock_conn(&st.db)?;
        (
            crate::character::active_character_id(&db, st.data_dir()),
            crate::pet::models::active_model_id(&db),
        )
    };
    let menu = crate::character::build_pet_menu_skins_payload(
        st.data_dir(),
        &character_id,
        &active_model_id,
    )?;
    let skins: Vec<_> = menu.skins.iter().filter(|s| s.model_ready).collect();
    if skins.is_empty() {
        return Err("没有可切换的皮肤".to_string());
    }
    let active_idx = skins.iter().position(|s| s.active);
    let next = if let Some(i) = active_idx {
        skins.iter().cycle().skip(i + 1).next()
    } else {
        skins.first()
    };
    let next = next.ok_or_else(|| "没有可切换的下一套皮肤".to_string())?;
    Ok((menu.character_id.clone(), next.id.clone()))
}

fn pick_next_favorite_character(st: &AppState) -> Result<String, String> {
    let list = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::character::list_pet_menu_favorite_characters(st.data_dir(), &db)
    };
    if list.is_empty() {
        return Err("没有收藏角色，请先在人物页收藏".to_string());
    }
    let active_idx = list.iter().position(|c| c.active);
    let next = if let Some(i) = active_idx {
        list.iter().cycle().skip(i + 1).next()
    } else {
        list.first()
    };
    next.map(|c| c.id.clone())
        .ok_or_else(|| "没有可切换的下一个收藏角色".to_string())
}

fn pick_default_skin_for_character(st: &AppState, character_id: &str) -> Result<String, String> {
    let menu = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::character::list_pet_menu_skins_for_character(st.data_dir(), &db, character_id)?
    };
    let ready: Vec<_> = menu.skins.iter().filter(|s| s.model_ready).collect();
    if ready.is_empty() {
        return Err(format!("角色 {character_id} 没有可切换的模型"));
    }
    let active = ready.iter().find(|s| s.active);
    let pick = active
        .or_else(|| ready.first())
        .ok_or_else(|| "没有可切换的皮肤".to_string())?;
    Ok(pick.id.clone())
}

fn pick_next_favorite_character_skin(st: &AppState) -> Result<(String, String), String> {
    let character_id = pick_next_favorite_character(st)?;
    let skin_id = pick_default_skin_for_character(st, &character_id)?;
    Ok((character_id, skin_id))
}

fn parse_query_usize(query: &str, key: &str, default: usize) -> usize {
    for part in query.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            if k == key {
                return v.parse().unwrap_or(default);
            }
        }
    }
    default
}

fn tail_display_logs(data_dir: &std::path::Path, n: usize) -> Result<Vec<String>, String> {
    let path = data_dir.join("logs").join("pet-display.jsonl");
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let start = lines.len().saturating_sub(n);
    Ok(lines[start..].to_vec())
}

pub fn api_index_json() -> Value {
    json!({
        "base": "http://127.0.0.1:19420",
        "endpoints": [
            { "method": "GET", "path": "/health" },
            { "method": "GET", "path": "/pet/snapshot" },
            { "method": "GET", "path": "/pet/status" },
            { "method": "GET", "path": "/pet/characters/favorites" },
            { "method": "GET", "path": "/pet/menu/skins" },
            { "method": "GET", "path": "/pet/characters" },
            { "method": "GET", "path": "/pet/logs/tail?n=40" },
            { "method": "POST", "path": "/pet/switch/skin", "body": { "character_id": "", "skin_id": "", "timeout_ms": 30000 } },
            { "method": "POST", "path": "/pet/switch/character", "body": { "character_id": "", "timeout_ms": 30000 } },
            { "method": "POST", "path": "/pet/switch/next-skin", "body": { "timeout_ms": 30000 } },
            { "method": "POST", "path": "/pet/switch/next-character", "body": { "timeout_ms": 30000 } },
            { "method": "POST", "path": "/pet/menu/open" },
            { "method": "POST", "path": "/pet/menu/hide" },
            { "method": "POST", "path": "/app/exit" }
        ]
    })
}
