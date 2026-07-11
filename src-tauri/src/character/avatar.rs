//! BWIKI 图鉴头像：按需 SQL 查询 + 下载到应用 data 目录（不预加载整表）

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::persona::import_reference::resolve_blhx_db_path;
use crate::state::AppState;

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("xiaohan-daily/0.1")
            .timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(4)
            .build()
            .expect("http client")
    })
}

#[derive(Debug, Clone, Default)]
pub struct CatalogMeta {
    pub avatar_url: Option<String>,
    pub faction: Option<String>,
    pub ship_type: Option<String>,
    pub rarity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvatarImportResult {
    pub ok: usize,
    pub skipped: usize,
    pub failed: usize,
    pub processed: usize,
    pub remaining: usize,
    pub tags_updated: usize,
    pub message: String,
}

static BLHX_CONN: OnceLock<Mutex<Option<(PathBuf, Connection)>>> = OnceLock::new();

pub fn avatars_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("characters").join("avatars")
}

pub fn resolve_avatar_path(data_dir: &Path, character_id: &str) -> Option<PathBuf> {
    let dir = avatars_dir(data_dir);
    for ext in ["webp", "jpg", "jpeg", "png"] {
        let path = dir.join(format!("{character_id}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn avatar_path_string(data_dir: &Path, character_id: &str) -> Option<String> {
    resolve_avatar_path(data_dir, character_id)
        .map(|p| p.to_string_lossy().into_owned())
}

/// 读取本地头像文件为 base64（供前端 blob 展示，绕过 asset 协议 scope 问题）
pub fn read_avatar_base64(data_dir: &Path, character_id: &str) -> Result<Option<String>, String> {
    let Some(path) = resolve_avatar_path(data_dir, character_id) else {
        return Ok(None);
    };
    let bytes = fs::read(&path).map_err(|e| format!("读取头像失败: {e}"))?;
    if bytes.is_empty() {
        return Ok(None);
    }
    Ok(Some(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        bytes,
    )))
}

/// 一次读取 avatars 目录，供列表页批量解析路径（避免每人 4 次 stat）
pub fn build_avatar_path_index(data_dir: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let dir = avatars_dir(data_dir);
    if !dir.is_dir() {
        return map;
    }
    if let Ok(rd) = fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if !stem.is_empty() {
                map.insert(stem.to_string(), path.to_string_lossy().into_owned());
            }
        }
    }
    map
}

pub fn count_avatar_files(data_dir: &Path) -> usize {
    let dir = avatars_dir(data_dir);
    if !dir.is_dir() {
        return 0;
    }
    fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).count())
        .unwrap_or(0)
}

fn with_blhx_conn<F, T>(f: F) -> Option<T>
where
    F: FnOnce(&Connection) -> Option<T>,
{
    let blhx = resolve_blhx_db_path().ok()?;
    let lock = BLHX_CONN.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().ok()?;
    let need_open = guard
        .as_ref()
        .is_none_or(|(path, _)| path != &blhx);
    if need_open {
        *guard = Connection::open(&blhx).ok().map(|c| (blhx, c));
    }
    guard.as_ref().and_then(|(_, conn)| f(conn))
}

pub fn lookup_meta(name: &str) -> Option<CatalogMeta> {
    with_blhx_conn(|conn| {
        conn.query_row(
            "SELECT avatar_url, faction, ship_type, rarity FROM catalog
             WHERE display_name = ?1 OR wiki_title = ?1 LIMIT 1",
            [name],
            |row| {
                Ok(CatalogMeta {
                    avatar_url: row.get(0)?,
                    faction: row.get(1)?,
                    ship_type: row.get(2)?,
                    rarity: row.get(3)?,
                })
            },
        )
        .ok()
    })
}

pub fn avatar_url_for_name(name: &str) -> Option<String> {
    lookup_meta(name).and_then(|m| {
        m.avatar_url
            .filter(|u| !u.trim().is_empty())
            .map(|u| u.trim().to_string())
    })
}

fn ext_from_url(url: &str) -> &'static str {
    let lower = url.to_ascii_lowercase();
    if lower.contains(".png") {
        "png"
    } else if lower.contains(".webp") {
        "webp"
    } else {
        "jpg"
    }
}

pub async fn download_avatar(url: &str, dest: &Path) -> Result<(), String> {
    let resp = http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载头像失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("头像 HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取头像数据失败: {e}"))?;
    if bytes.is_empty() {
        return Err("头像内容为空".into());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(dest, &bytes).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn ensure_avatar_cached(
    data_dir: &Path,
    character_id: &str,
    name: &str,
) -> Result<Option<String>, String> {
    if let Some(path) = resolve_avatar_path(data_dir, character_id) {
        return Ok(Some(path.to_string_lossy().into_owned()));
    }
    let Some(url) = avatar_url_for_name(name) else {
        return Ok(None);
    };
    let ext = ext_from_url(&url);
    let dest = avatars_dir(data_dir).join(format!("{character_id}.{ext}"));
    download_avatar(&url, &dest).await?;
    Ok(Some(dest.to_string_lossy().into_owned()))
}

/// 批量下载（已知 id + 显示名，跳过 find_character_meta）
pub async fn ensure_avatars_cached_batch_pairs(
    data_dir: &Path,
    pairs: &[(String, String)],
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    const CONCURRENCY: usize = 4;
    for chunk in pairs.chunks(CONCURRENCY) {
        let mut tasks = Vec::new();
        for (id, name) in chunk {
            let id = id.clone();
            let name = name.clone();
            let data_dir = data_dir.to_path_buf();
            tasks.push(tokio::spawn(async move {
                let path = ensure_avatar_cached(&data_dir, &id, &name).await?;
                Ok::<_, String>((id, path))
            }));
        }
        for task in tasks {
            if let Ok(Ok((id, Some(path)))) = task.await {
                out.insert(id, path);
            }
        }
    }
    out
}

/// 批量下载头像到本地（有限并发）
pub async fn ensure_avatars_cached_batch(
    data_dir: &Path,
    character_ids: &[String],
) -> HashMap<String, String> {
    let mut pairs = Vec::with_capacity(character_ids.len());
    for id in character_ids {
        if resolve_avatar_path(data_dir, id).is_some() {
            continue;
        }
        if let Ok(meta) = crate::character::find_character_meta(data_dir, id) {
            pairs.push((id.clone(), meta.name));
        }
    }
    ensure_avatars_cached_batch_pairs(data_dir, &pairs).await
}

/// 尚未缓存且 BWIKI 图鉴有 URL 的人物
pub fn collect_pending_avatars(data_dir: &Path, skip_existing: bool) -> Vec<(String, String)> {
    let mut pending = Vec::new();
    for (id, name) in crate::character::roster_id_names(data_dir) {
        if skip_existing && resolve_avatar_path(data_dir, &id).is_some() {
            continue;
        }
        if avatar_url_for_name(&name).is_some() {
            pending.push((id, name));
        }
    }
    pending
}

const AVATAR_STARTUP_DELAY_SECS: u64 = 1;
const AVATAR_BATCH_SIZE: usize = 48;
const AVATAR_BATCH_PAUSE_SECS: u64 = 2;

/// 应用启动后后台分批下载缺失头像，落盘到 data/characters/avatars
pub fn spawn_sync_on_startup(app: AppHandle, st: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(AVATAR_STARTUP_DELAY_SECS)).await;

        if resolve_blhx_db_path().is_err() {
            crate::log::warn("avatar sync skipped: blhx sqlite not found");
            return;
        }

        loop {
            if st.stop_flag.load(Ordering::Relaxed) {
                break;
            }

            let data_dir = st.data_dir().to_path_buf();
            let pending = collect_pending_avatars(&data_dir, true);
            if pending.is_empty() {
                crate::log::info("avatar sync: all roster avatars cached");
                break;
            }

            let batch: Vec<(String, String)> = pending
                .into_iter()
                .take(AVATAR_BATCH_SIZE)
                .collect();
            let paths = ensure_avatars_cached_batch_pairs(&data_dir, &batch).await;
            if !paths.is_empty() {
                let _ = app.emit("avatars-cached", paths);
            }

            tokio::time::sleep(Duration::from_secs(AVATAR_BATCH_PAUSE_SECS)).await;
        }
    });
}

pub async fn run_avatar_import_default(
    data_dir: &Path,
    limit: usize,
    skip_existing: bool,
    _sync_tags: bool,
) -> Result<AvatarImportResult, String> {
    let mut pending = collect_pending_avatars(data_dir, skip_existing);
    let remaining_total = pending.len();
    let take = limit.max(1).min(pending.len());
    let batch: Vec<_> = pending.drain(..take).collect();
    let mut ok = 0usize;
    let mut failed = 0usize;
    const CONCURRENCY: usize = 4;
    for chunk in batch.chunks(CONCURRENCY) {
        let mut tasks = Vec::new();
        for (id, name) in chunk {
            let id = id.clone();
            let name = name.clone();
            let data_dir = data_dir.to_path_buf();
            tasks.push(tokio::spawn(async move {
                match ensure_avatar_cached(&data_dir, &id, &name).await {
                    Ok(Some(_)) => Ok(true),
                    Ok(None) => Ok(false),
                    Err(e) => Err(e),
                }
            }));
        }
        for task in tasks {
            match task.await {
                Ok(Ok(true)) => ok += 1,
                Ok(Ok(false)) => {}
                Ok(Err(_)) | Err(_) => failed += 1,
            }
        }
    }
    let remaining = remaining_total.saturating_sub(batch.len());
    Ok(AvatarImportResult {
        ok,
        skipped: 0,
        failed,
        processed: batch.len(),
        remaining,
        tags_updated: 0,
        message: format!(
            "本批处理 {} 条：头像缓存 {ok}，失败 {failed}，剩余约 {remaining} 条",
            batch.len()
        ),
    })
}
