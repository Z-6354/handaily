//! 本地 HTTP Agent 控制 API（127.0.0.1），供 MCP / 脚本驱动桌宠。
//! 需在设置中开启「Agent 控制接口」；`HANDAILY_DISABLE_TEST_API=1` 可强制关闭。

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

static TEST_API_CTX: OnceLock<(AppHandle, Arc<AppState>)> = OnceLock::new();
static TEST_API_SHUTDOWN: AtomicBool = AtomicBool::new(false);

const DEFAULT_PORT: u16 = 19420;

pub fn spawn_server(app: AppHandle, st: Arc<AppState>) {
    if std::env::var("HANDAILY_DISABLE_TEST_API").is_ok() {
        crate::log::info("test-api: disabled (HANDAILY_DISABLE_TEST_API)");
        return;
    }
    TEST_API_SHUTDOWN.store(false, Ordering::SeqCst);
    let _ = TEST_API_CTX.set((app, st));
    thread::Builder::new()
        .name("handaily-test-api".into())
        .spawn(server_loop)
        .expect("test-api thread");
}

/// 退出时唤醒 accept 循环，避免 `incoming()` 阻塞进程结束。
pub fn shutdown_server() {
    TEST_API_SHUTDOWN.store(true, Ordering::SeqCst);
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
    if let Err(e) = listener.set_nonblocking(true) {
        crate::log::warn(format!("test-api: set_nonblocking failed: {e}"));
        return;
    }
    crate::log::info(format!("test-api: listening on http://{addr}"));
    loop {
        if shutting_down() {
            break;
        }
        match listener.accept() {
            Ok((conn, _)) => {
                if shutting_down() {
                    break;
                }
                thread::spawn(move || {
                    if let Err(e) = handle_connection(conn) {
                        crate::log::warn(format!("test-api: request error: {e}"));
                    }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                crate::log::warn(format!("test-api: accept error: {e}"));
                break;
            }
        }
    }
}

fn shutting_down() -> bool {
    TEST_API_SHUTDOWN.load(Ordering::Relaxed)
        || crate::APP_EXITING.load(Ordering::Relaxed)
        || TEST_API_CTX
            .get()
            .map(|(_, st)| st.stop_flag.load(Ordering::Relaxed))
            .unwrap_or(false)
}

fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = match stream.read(&mut chunk) {
            Ok(0) if buf.is_empty() => return Ok(String::new()),
            Ok(0) => break,
            Ok(n) => n,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                if buf.is_empty() {
                    return Err(e.to_string());
                }
                break;
            }
            Err(e) => return Err(e.to_string()),
        };
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            let raw = String::from_utf8_lossy(&buf);
            let mut content_length = 0usize;
            for line in raw.split("\r\n") {
                if line.is_empty() {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("content-length:") {
                    content_length = rest.trim().parse().unwrap_or(0);
                }
            }
            let header_end = raw.find("\r\n\r\n").unwrap_or(raw.len());
            let have_body = buf.len().saturating_sub(header_end + 4);
            if have_body >= content_length {
                break;
            }
        }
        if buf.len() > 256 * 1024 {
            return Err("request too large".into());
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn handle_connection(mut stream: TcpStream) -> Result<(), String> {
    let _ = stream.set_nonblocking(false);
    let conn_t0 = Instant::now();
    let read_timeout = if shutting_down() {
        Duration::from_millis(250)
    } else {
        Duration::from_secs(8)
    };
    let _ = stream.set_read_timeout(Some(read_timeout));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(8)));
    let raw = read_http_request(&mut stream)?;
    if raw.is_empty() {
        return Ok(());
    }
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
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
    }
    let header_end = raw.find("\r\n\r\n").unwrap_or(raw.len());
    let body = if content_length > 0 && header_end + 4 + content_length <= raw.len() {
        raw[header_end + 4..header_end + 4 + content_length].to_string()
    } else if header_end + 4 < raw.len() {
        raw[header_end + 4..].trim().to_string()
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
                "mcp_enabled": true,
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
        ("GET", "/pet/logs/movement/tail") => {
            let (_app, st) = ctx()?;
            let n = parse_query_usize(query, "n", 40).clamp(1, 200);
            let lines = tail_movement_logs(st.data_dir(), n)?;
            Ok((200, json!({ "ok": true, "lines": lines })))
        }
        ("GET", "/pet/interaction") => {
            let (app, st) = ctx()?;
            let state = crate::pet::interaction_state(app, st)?;
            Ok((200, json!({ "ok": true, "interaction": state })))
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
            let (app, st) = ctx()?;
            log_test_action(st, "menu.open", json!({}))?;
            crate::pet::show_pet_menu_at_cursor(app)?;
            Ok((200, json!({ "ok": true })))
        }
        ("POST", "/pet/menu/hide") => {
            let (app, st) = ctx()?;
            log_test_action(st, "menu.hide", json!({}))?;
            crate::pet::hide_pet_menu(app)?;
            crate::pet::sync_pet_interaction_state(app);
            Ok((200, json!({ "ok": true })))
        }
        ("POST", "/pet/click/left") => {
            let (app, st) = ctx()?;
            log_test_action(st, "click.left", json!({}))?;
            crate::pet::emit_pet_test_action(app, "click-left")?;
            Ok((200, json!({ "ok": true, "action": "click-left" })))
        }
        ("POST", "/pet/click/right") => {
            let (app, st) = ctx()?;
            log_test_action(st, "click.right", json!({}))?;
            let open = crate::pet::toggle_pet_menu_at_cursor(app)?;
            Ok((200, json!({ "ok": true, "menu_open": open })))
        }
        ("POST", "/pet/click/double") => {
            let (app, st) = ctx()?;
            log_test_action(st, "click.double", json!({}))?;
            crate::pet::emit_pet_test_action(app, "click-double")?;
            Ok((200, json!({ "ok": true, "action": "click-double" })))
        }
        ("POST", "/pet/main/open") => {
            let (app, st) = ctx()?;
            log_test_action(st, "main.open", json!({}))?;
            crate::pet::show_main_window(app, None)?;
            Ok((200, json!({ "ok": true })))
        }
        ("POST", "/pet/main/close") => {
            let (app, st) = ctx()?;
            log_test_action(st, "main.close", json!({}))?;
            crate::pet::hide_main_window(app)?;
            Ok((200, json!({ "ok": true })))
        }
        ("POST", "/pet/bubble/set") => {
            let input: BubbleSetBody = if body.trim().is_empty() {
                BubbleSetBody {
                    enabled: parse_query_bool(query, "enabled").ok_or_else(|| {
                        "bubble/set requires JSON body or ?enabled=true|false".to_string()
                    })?,
                }
            } else {
                parse_body(body)?
            };
            let (app, st) = ctx()?;
            log_test_action(st, "bubble.set", json!({ "enabled": input.enabled }))?;
            {
                let db = crate::db::lock_conn(&st.db)?;
                crate::pet::set_bubble_enabled(&db, input.enabled)?;
            }
            for label in [crate::pet::PET_LABEL, crate::pet::PET_MENU_LABEL, "main"] {
                let _ = app.emit_to(label, "pet-bubble-enabled-changed", input.enabled);
            }
            if !input.enabled {
                let _ = app.emit_to(crate::pet::PET_LABEL, "pet-clear-bubble", ());
            }
            crate::pet::emit_pet_status_changed(app);
            Ok((200, json!({ "ok": true, "enabled": input.enabled })))
        }
        ("POST", "/pet/interaction/sync") => {
            let (app, st) = ctx()?;
            log_test_action(st, "interaction.sync", json!({}))?;
            crate::pet::sync_pet_interaction_state(app);
            crate::pet::emit_pet_test_action(app, "sync-interaction")?;
            let state = crate::pet::interaction_state(app, st)?;
            Ok((200, json!({ "ok": true, "interaction": state })))
        }
        ("POST", "/pet/speak") => {
            let input: SpeakBody = parse_body(body)?;
            let (app, st) = ctx()?;
            crate::pet::emit_remark_agent(&app, st, &input.text, input.animation)?;
            Ok((200, json!({ "ok": true, "text": input.text.trim() })))
        }
        ("POST", "/pet/speak/random") => {
            let (app, st) = ctx()?;
            let text = crate::pet::emit_random_remark_agent(&app, st)?;
            Ok((200, json!({ "ok": true, "text": text })))
        }
        ("POST", "/pet/preview/animation") => {
            let input: PreviewAnimationBody = parse_body(body)?;
            let (app, st) = ctx()?;
            crate::pet::preview_animation(&app, st, &input.animation, input.r#loop.unwrap_or(false))?;
            Ok((200, json!({ "ok": true, "animation": input.animation })))
        }
        ("POST", "/pet/edit/enter") => {
            let (app, _st) = ctx()?;
            crate::pet::enter_pet_edit_bounds(app)?;
            Ok((200, json!({ "ok": true })))
        }
        ("GET", "/system/cursor") => {
            let pos = crate::system::win32_input::cursor_position()?;
            Ok((200, json!({ "ok": true, "cursor": pos })))
        }
        ("POST", "/system/cursor") => {
            let input: CursorSetBody = parse_body(body)?;
            crate::system::win32_input::set_cursor_position(input.x, input.y)?;
            Ok((200, json!({ "ok": true, "x": input.x, "y": input.y })))
        }
        ("POST", "/system/mouse") => {
            let input: MouseBody = parse_body(body)?;
            let (app, st) = ctx()?;
            let button = input.button.as_deref().unwrap_or("left");
            let action = input.action.as_deref().unwrap_or("click");
            log_test_action(
                st,
                "system.mouse",
                json!({ "button": button, "action": action, "x": input.x, "y": input.y }),
            )?;
            let _ = app;
            crate::system::win32_input::mouse_button_action(button, action, input.x, input.y)?;
            Ok((
                200,
                json!({ "ok": true, "button": button, "action": action, "x": input.x, "y": input.y }),
            ))
        }
        ("GET", "/system/screenshot/pet") => {
            let (app, st) = ctx()?;
            let region = pet_capture_region(app)?;
            log_test_action(st, "system.screenshot.pet", json!({ "region": region }))?;
            let shot = crate::system::win32_input::capture_region_png(
                region.x,
                region.y,
                region.width,
                region.height,
            )?;
            Ok((200, json!({ "ok": true, "screenshot": shot })))
        }
        ("GET", "/system/screenshot") => {
            let max_w = parse_query_usize(query, "max_width", 1280).clamp(320, 3840) as u32;
            let shot = crate::system::win32_input::capture_primary_screen_png(max_w)?;
            Ok((200, json!({ "ok": true, "screenshot": shot })))
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

#[derive(Deserialize)]
struct BubbleSetBody {
    enabled: bool,
}

#[derive(Deserialize)]
struct CursorSetBody {
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
struct MouseBody {
    x: i32,
    y: i32,
    button: Option<String>,
    action: Option<String>,
}

fn pet_capture_region(app: &AppHandle) -> Result<crate::system::win32_input::ScreenRegion, String> {
    let win = app
        .get_webview_window(crate::pet::PET_LABEL)
        .ok_or_else(|| "pet window missing".to_string())?;
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let size = win.outer_size().map_err(|e| e.to_string())?;
    Ok(crate::system::win32_input::ScreenRegion {
        x: pos.x,
        y: pos.y,
        width: size.width as i32,
        height: size.height as i32,
    })
}

fn log_test_action(st: &AppState, action: &str, detail: Value) -> Result<(), String> {
    let line = json!({
        "ts": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        "scope": "test-api",
        "level": "info",
        "message": action,
        "detail": detail,
    })
    .to_string();
    crate::pet::append_display_debug_logs(st.data_dir(), &[line])?;
    crate::log::info(format!("test-api action: {action}"));
    Ok(())
}

fn tail_movement_logs(data_dir: &std::path::Path, n: usize) -> Result<Vec<String>, String> {
    let path = data_dir.join("logs").join("pet-movement.jsonl");
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let start = lines.len().saturating_sub(n);
    Ok(lines[start..].to_vec())
}

#[derive(Deserialize)]
struct SpeakBody {
    text: String,
    animation: Option<String>,
}

#[derive(Deserialize)]
struct PreviewAnimationBody {
    animation: String,
    #[serde(rename = "loop")]
    r#loop: Option<bool>,
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

fn parse_query_bool(query: &str, key: &str) -> Option<bool> {
    for part in query.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            if k == key {
                return match v.trim().to_ascii_lowercase().as_str() {
                    "1" | "true" | "yes" | "on" => Some(true),
                    "0" | "false" | "no" | "off" => Some(false),
                    _ => None,
                };
            }
        }
    }
    None
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
            { "method": "GET", "path": "/pet/logs/movement/tail?n=40" },
            { "method": "GET", "path": "/pet/interaction" },
            { "method": "POST", "path": "/pet/switch/skin", "body": { "character_id": "", "skin_id": "", "timeout_ms": 30000 } },
            { "method": "POST", "path": "/pet/switch/character", "body": { "character_id": "", "timeout_ms": 30000 } },
            { "method": "POST", "path": "/pet/switch/next-skin", "body": { "timeout_ms": 30000 } },
            { "method": "POST", "path": "/pet/switch/next-character", "body": { "timeout_ms": 30000 } },
            { "method": "POST", "path": "/pet/menu/open" },
            { "method": "POST", "path": "/pet/menu/hide" },
            { "method": "POST", "path": "/pet/click/left" },
            { "method": "POST", "path": "/pet/click/right" },
            { "method": "POST", "path": "/pet/click/double" },
            { "method": "POST", "path": "/pet/main/open" },
            { "method": "POST", "path": "/pet/main/close" },
            { "method": "POST", "path": "/pet/bubble/set", "body": { "enabled": true } },
            { "method": "POST", "path": "/pet/interaction/sync" },
            { "method": "POST", "path": "/pet/speak", "body": { "text": "", "animation": null } },
            { "method": "POST", "path": "/pet/speak/random" },
            { "method": "POST", "path": "/pet/preview/animation", "body": { "animation": "", "loop": false } },
            { "method": "POST", "path": "/pet/edit/enter" },
            { "method": "GET", "path": "/system/cursor" },
            { "method": "POST", "path": "/system/cursor", "body": { "x": 0, "y": 0 } },
            { "method": "POST", "path": "/system/mouse", "body": { "x": 0, "y": 0, "button": "left", "action": "click" } },
            { "method": "GET", "path": "/system/screenshot?max_width=1280" },
            { "method": "GET", "path": "/system/screenshot/pet" },
            { "method": "POST", "path": "/app/exit" }
        ]
    })
}
