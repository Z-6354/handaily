//! 舰娘：Cubism Live2D 模型清单、同步与桌面桌宠（共用 pet 窗）

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::data_layout::{
    kanmusu_dir, kanmusu_manifest_path, kanmusu_model_dir, kanmusu_models_dir,
    resolve_repo_model_unpacked_root,
};
use crate::manifest_lock;
use crate::pet::{
    self, COMPANION_ENGINE_KANMUSU, PET_LABEL, PET_MENU_LABEL,
};
use crate::state::AppState;

pub const KANMUSU_PLAYER_LABEL: &str = "kanmusu-player";

const DEFAULT_WIDTH: f64 = 900.0;
const DEFAULT_HEIGHT: f64 = 1200.0;

#[derive(Default)]
pub struct KanmusuRuntimeState {
    pub page_load_finished: AtomicBool,
    pub pending_load: Mutex<Option<KanmusuPlayerLoadPayload>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanmusuLine {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanmusuSkin {
    pub id: String,
    pub name: String,
    pub model_dir: String,
    #[serde(default)]
    pub lines: Vec<KanmusuLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanmusuCharacter {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub skins: Vec<KanmusuSkin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanmusuManifest {
    pub version: u32,
    pub characters: Vec<KanmusuCharacter>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuCharacterBrief {
    pub id: String,
    pub name: String,
    pub description: String,
    pub skin_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuCharacterDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub skins: Vec<KanmusuSkinDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuSkinDetail {
    pub id: String,
    pub name: String,
    pub model_dir: String,
    pub model_ready: bool,
    pub model3_path: Option<String>,
    pub lines: Vec<KanmusuLine>,
    #[serde(default)]
    pub idle_animation: Option<String>,
    #[serde(default)]
    pub click_animation: Option<String>,
    #[serde(default)]
    pub animations: Vec<String>,
    #[serde(default)]
    pub touch_area_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuSyncResult {
    pub synced_slugs: Vec<String>,
    pub added_characters: usize,
    pub added_skins: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuTouchArea {
    pub id: String,
    pub zone: String,
    pub click_animation: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub bounds: KanmusuTouchBounds,
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuTouchBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for KanmusuTouchBounds {
    fn default() -> Self {
        Self {
            x: 0.2,
            y: 0.2,
            width: 0.6,
            height: 0.6,
        }
    }
}

fn touch_priority_for_zone(zone: &str) -> i32 {
    match zone {
        "special" => 2,
        "head" => 1,
        _ => 0,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuPlayerLoadPayload {
    pub skin_id: String,
    pub skin_name: String,
    pub model_dir: String,
    pub model3_path: String,
    /// AppData 模型目录绝对路径（前端 convertFileSrc）；空则回退 base64 IPC
    #[serde(default)]
    pub model_abs_dir: String,
    pub lines: Vec<KanmusuLine>,
    #[serde(default)]
    pub idle_animation: Option<String>,
    #[serde(default)]
    pub click_animation: Option<String>,
    #[serde(default)]
    pub drag_animation: Option<String>,
    #[serde(default)]
    pub boot_animation: Option<String>,
    #[serde(default)]
    pub random_animations: Vec<String>,
    #[serde(default)]
    pub random_min_sec: i64,
    #[serde(default)]
    pub random_max_sec: i64,
    #[serde(default)]
    pub animations: Vec<String>,
    #[serde(default)]
    pub touch_areas: Vec<KanmusuTouchArea>,
}

pub fn ensure_dirs(data_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(kanmusu_dir(data_dir)).map_err(|e| e.to_string())?;
    fs::create_dir_all(kanmusu_models_dir(data_dir)).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_manifest(data_dir: &Path) -> Result<KanmusuManifest, String> {
    ensure_dirs(data_dir)?;
    let path = kanmusu_manifest_path(data_dir);
    if !path.is_file() {
        return Ok(KanmusuManifest {
            version: 1,
            characters: Vec::new(),
        });
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 kanmusu manifest 失败: {e}"))
}

pub fn save_manifest(data_dir: &Path, manifest: &KanmusuManifest) -> Result<(), String> {
    ensure_dirs(data_dir)?;
    let path = kanmusu_manifest_path(data_dir);
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    manifest_lock::atomic_write(&path, &json)
}

pub fn list_brief(data_dir: &Path) -> Result<Vec<KanmusuCharacterBrief>, String> {
    // Phase 4: derive from character skin slots with kanmusu_dir (authority).
    let chars = crate::character::load_manifest(data_dir);
    let mut out = Vec::new();
    for c in &chars.characters {
        let skin_count = c
            .skins
            .iter()
            .filter(|s| {
                s.kanmusu_dir
                    .as_ref()
                    .map(|d| !d.trim().is_empty())
                    .unwrap_or(false)
            })
            .count();
        if skin_count == 0 {
            continue;
        }
        out.push(KanmusuCharacterBrief {
            id: c.id.clone(),
            name: c.name.clone(),
            description: c.description.clone(),
            skin_count,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub fn get_detail(data_dir: &Path, character_id: &str) -> Result<KanmusuCharacterDetail, String> {
    // Phase 4: character manifest first.
    let chars = crate::character::load_manifest(data_dir);
    if let Some(character) = chars.characters.iter().find(|c| c.id == character_id) {
        let skins: Vec<KanmusuSkinDetail> = character
            .skins
            .iter()
            .filter_map(|s| {
                let dir = s.kanmusu_dir.as_ref()?.trim();
                if dir.is_empty() {
                    return None;
                }
                let km = KanmusuSkin {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    model_dir: dir.to_string(),
                    lines: s
                        .lines
                        .iter()
                        .map(|l| KanmusuLine {
                            text: l.text.clone(),
                            animation: l.animation.clone(),
                        })
                        .collect(),
                };
                Some(skin_to_detail(data_dir, &km))
            })
            .collect();
        if !skins.is_empty() {
            return Ok(KanmusuCharacterDetail {
                id: character.id.clone(),
                name: character.name.clone(),
                description: character.description.clone(),
                skins,
            });
        }
    }
    // Legacy fallback: kanmusu/manifest.json
    if let Ok(manifest) = load_manifest(data_dir) {
        if let Some(character) = manifest.characters.iter().find(|c| c.id == character_id) {
            return Ok(KanmusuCharacterDetail {
                id: character.id.clone(),
                name: character.name.clone(),
                description: character.description.clone(),
                skins: character
                    .skins
                    .iter()
                    .map(|s| skin_to_detail(data_dir, s))
                    .collect(),
            });
        }
    }
    Err(format!("未找到舰娘角色: {character_id}"))
}

fn skin_to_detail(data_dir: &Path, skin: &KanmusuSkin) -> KanmusuSkinDetail {
    let model3 = find_model3_json(data_dir, &skin.model_dir);
    let meta = read_animation_meta(data_dir, &skin.model_dir);
    let touch_area_count = read_touch_area_count(data_dir, &skin.model_dir);
    KanmusuSkinDetail {
        id: skin.id.clone(),
        name: skin.name.clone(),
        model_dir: skin.model_dir.clone(),
        model_ready: model3.is_some(),
        model3_path: model3,
        lines: skin.lines.clone(),
        idle_animation: meta
            .as_ref()
            .and_then(|m| m.get("idle_animation"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        click_animation: meta
            .as_ref()
            .and_then(|m| m.get("click_animation"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        animations: meta
            .as_ref()
            .and_then(|m| m.get("animations"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        touch_area_count,
    }
}

fn read_animation_meta(data_dir: &Path, model_dir: &str) -> Option<serde_json::Value> {
    let path = kanmusu_model_dir(data_dir, model_dir).join("animations.meta.json");
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn read_touch_area_count(data_dir: &Path, model_dir: &str) -> usize {
    let path = kanmusu_model_dir(data_dir, model_dir).join("touch_areas.json");
    let Ok(raw) = fs::read_to_string(path) else {
        return 0;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return 0;
    };
    v.get("areas")
        .and_then(|a| a.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

pub fn update_character(
    data_dir: &Path,
    character_id: &str,
    name: Option<String>,
    description: Option<String>,
) -> Result<KanmusuCharacterDetail, String> {
    manifest_lock::with_lock(|| {
        let mut manifest = load_manifest(data_dir)?;
        let character = manifest
            .characters
            .iter_mut()
            .find(|c| c.id == character_id)
            .ok_or_else(|| format!("未找到舰娘角色: {character_id}"))?;
        if let Some(n) = name {
            character.name = n;
        }
        if let Some(d) = description {
            character.description = d;
        }
        save_manifest(data_dir, &manifest)?;
        get_detail(data_dir, character_id)
    })
}

pub fn update_skin(
    data_dir: &Path,
    character_id: &str,
    skin_id: &str,
    name: Option<String>,
    lines: Option<Vec<KanmusuLine>>,
) -> Result<KanmusuSkinDetail, String> {
    manifest_lock::with_lock(|| {
        let mut manifest = load_manifest(data_dir)?;
        let character = manifest
            .characters
            .iter_mut()
            .find(|c| c.id == character_id)
            .ok_or_else(|| format!("未找到舰娘角色: {character_id}"))?;
        let skin = character
            .skins
            .iter_mut()
            .find(|s| s.id == skin_id)
            .ok_or_else(|| format!("未找到皮肤: {skin_id}"))?;
        if let Some(n) = name {
            skin.name = n;
        }
        if let Some(ls) = lines {
            skin.lines = ls;
        }
        let updated = skin.clone();
        save_manifest(data_dir, &manifest)?;
        Ok(skin_to_detail(data_dir, &updated))
    })
}

pub fn ensure_seeded(data_dir: &Path) -> Result<(), String> {
    // Phase 4: seed disk from unpacked when no models; bind onto character skins.
    if !has_any_kanmusu_model_on_disk(data_dir) {
        let _ = sync_from_unpacked(data_dir)?;
    }
    let _ = crate::character::attach_kanmusu_after_sync(data_dir);
    Ok(())
}

fn has_any_kanmusu_model_on_disk(data_dir: &Path) -> bool {
    let root = kanmusu_models_dir(data_dir);
    let Ok(entries) = fs::read_dir(&root) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        p.is_dir() && has_moc3(&p)
    })
}

pub fn sync_from_unpacked(data_dir: &Path) -> Result<KanmusuSyncResult, String> {
    // Copy only — character bindings updated by attach_kanmusu_after_sync (caller / ensure_seeded).
    sync_from_unpacked_inner(data_dir)
}

fn sync_from_unpacked_inner(data_dir: &Path) -> Result<KanmusuSyncResult, String> {
    ensure_dirs(data_dir)?;
    let repo_root = resolve_repo_model_unpacked_root();
    let mut synced_slugs = Vec::new();

    let entries = fs::read_dir(&repo_root).map_err(|e| format!("读取解包目录失败: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().trim().to_string();
        if slug.is_empty() || !has_moc3(&path) {
            continue;
        }
        copy_model_dir(&path, &kanmusu_model_dir(data_dir, &slug))?;
        synced_slugs.push(slug);
    }

    synced_slugs.sort();
    let count = synced_slugs.len();
    Ok(KanmusuSyncResult {
        synced_slugs,
        added_characters: 0,
        added_skins: count,
        message: format!("已同步 {count} 个 Cubism 模型到磁盘（人物皮肤绑定由后续 attach 完成）"),
    })
}

fn has_moc3(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.path()
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("moc3"))
            .unwrap_or(false)
    })
}

fn copy_model_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.is_dir() {
        fs::remove_dir_all(dst).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    copy_dir_recursive(src, dst)
}

/// (file_count, total_bytes, max_mtime_secs) — 用于跳过无变化的整包拷贝
fn dir_fingerprint(dir: &Path) -> Result<(u64, u64, u64), String> {
    fn walk(path: &Path, count: &mut u64, bytes: &mut u64, mtime: &mut u64) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())?.flatten() {
            let p = entry.path();
            let ft = entry.file_type().map_err(|e| e.to_string())?;
            if ft.is_dir() {
                walk(&p, count, bytes, mtime)?;
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            *count += 1;
            *bytes += meta.len();
            if let Ok(modified) = meta.modified() {
                if let Ok(secs) = modified.duration_since(std::time::UNIX_EPOCH) {
                    *mtime = (*mtime).max(secs.as_secs());
                }
            }
        }
        Ok(())
    }
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut mtime = 0u64;
    walk(dir, &mut count, &mut bytes, &mut mtime)?;
    Ok((count, bytes, mtime))
}

fn copy_model_dir_if_stale(src: &Path, dst: &Path) -> Result<bool, String> {
    if dst.is_dir() && has_moc3(dst) {
        let src_fp = dir_fingerprint(src)?;
        let dst_fp = dir_fingerprint(dst)?;
        if src_fp == dst_fp {
            return Ok(false);
        }
    }
    copy_model_dir(src, dst)?;
    Ok(true)
}

/// 热切换时跳过全量 fingerprint；短时窗口内认为 AppData 已与解包一致。
static REFRESH_SKIP_UNTIL: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
const REFRESH_CACHE_TTL: Duration = Duration::from_secs(90);

fn note_refresh_fresh(slug: &str) {
    if let Ok(mut guard) = REFRESH_SKIP_UNTIL.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(slug.to_string(), Instant::now() + REFRESH_CACHE_TTL);
    }
}

fn refresh_recently_fresh(slug: &str) -> bool {
    let Ok(guard) = REFRESH_SKIP_UNTIL.lock() else {
        return false;
    };
    let Some(map) = guard.as_ref() else {
        return false;
    };
    map.get(slug).is_some_and(|until| Instant::now() < *until)
}

/// Refresh one skin's Cubism files from repo `data/model/unpacked` before preview,
/// so newly merged motions / HitAreas land in AppData without a manual full sync.
/// 未变更时跳过整目录删除重拷（否则每次预览都要几秒甚至更久）。
pub fn refresh_model_from_unpacked(data_dir: &Path, model_dir: &str) -> Result<bool, String> {
    let slug = model_dir.trim();
    if slug.is_empty() || slug.contains("..") || slug.contains('/') || slug.contains('\\') {
        return Err("非法 model_dir".into());
    }
    if refresh_recently_fresh(slug) {
        return Ok(false);
    }
    let src = resolve_repo_model_unpacked_root().join(slug);
    if !src.is_dir() || !has_moc3(&src) {
        return Ok(false);
    }
    ensure_dirs(data_dir)?;
    let changed = copy_model_dir_if_stale(&src, &kanmusu_model_dir(data_dir, slug))?;
    note_refresh_fresh(slug);
    Ok(changed)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            copy_dir_recursive(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Split `aidang_2` → (`aidang`, `皮肤 2`); used by unit tests and attach heuristics.
#[cfg_attr(not(test), allow(dead_code))]
fn split_slug(slug: &str) -> (String, String) {
    let lower = slug.to_lowercase();
    if let Some((base, suffix)) = lower.rsplit_once('_') {
        if suffix.chars().all(|c| c.is_ascii_digit()) && !base.is_empty() {
            return (base.to_string(), format!("皮肤 {suffix}"));
        }
    }
    (lower, slug.to_string())
}

pub fn find_model3_json(data_dir: &Path, model_dir: &str) -> Option<String> {
    let dir = kanmusu_model_dir(data_dir, model_dir);
    let entries = fs::read_dir(&dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name()?.to_string_lossy();
            if name.ends_with(".model3.json") {
                return Some(path.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    None
}

/// 只解析目标皮肤，避免 get_detail 扫角色下每一套皮肤目录。
pub fn lookup_skin_detail_public(
    data_dir: &Path,
    character_id: &str,
    skin_id: &str,
) -> Result<KanmusuSkinDetail, String> {
    lookup_skin_detail(data_dir, character_id, skin_id)
}

fn lookup_skin_detail(
    data_dir: &Path,
    character_id: &str,
    skin_id: &str,
) -> Result<KanmusuSkinDetail, String> {
    // Phase 4: character slot first (skin id or kanmusu_dir match).
    let chars = crate::character::load_manifest(data_dir);
    if let Some(character) = chars.characters.iter().find(|c| c.id == character_id) {
        if let Some(skin) = character.skins.iter().find(|s| {
            s.id == skin_id
                || s.kanmusu_dir
                    .as_ref()
                    .map(|d| d.trim() == skin_id)
                    .unwrap_or(false)
        }) {
            if let Some(dir) = skin
                .kanmusu_dir
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                let km = KanmusuSkin {
                    id: skin.id.clone(),
                    name: skin.name.clone(),
                    model_dir: dir.to_string(),
                    lines: skin
                        .lines
                        .iter()
                        .map(|l| KanmusuLine {
                            text: l.text.clone(),
                            animation: l.animation.clone(),
                        })
                        .collect(),
                };
                return Ok(skin_to_detail(data_dir, &km));
            }
        }
    }
    // Legacy kanmusu/manifest.json
    if let Ok(manifest) = load_manifest(data_dir) {
        if let Some(character) = manifest.characters.iter().find(|c| c.id == character_id) {
            if let Some(skin) = character.skins.iter().find(|s| s.id == skin_id) {
                return Ok(skin_to_detail(data_dir, skin));
            }
        }
    }
    Err(format!("未找到皮肤: {skin_id}"))
}

pub fn build_player_load_payload(
    data_dir: &Path,
    character_id: &str,
    skin_id: &str,
) -> Result<KanmusuPlayerLoadPayload, String> {
    let skin = lookup_skin_detail(data_dir, character_id, skin_id)?;
    build_player_load_payload_for_skin(data_dir, &skin)
}

fn build_player_load_payload_for_skin(
    data_dir: &Path,
    skin: &KanmusuSkinDetail,
) -> Result<KanmusuPlayerLoadPayload, String> {
    let model3_path = skin
        .model3_path
        .clone()
        .ok_or_else(|| format!("皮肤 {} 缺少 model3.json", skin.id))?;
    let meta = read_animation_meta(data_dir, &skin.model_dir);
    let animations = if !skin.animations.is_empty() {
        skin.animations.clone()
    } else {
        meta.as_ref()
            .and_then(|m| m.get("animations"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let idle = skin.idle_animation.clone().or_else(|| {
        meta.as_ref()
            .and_then(|m| m.get("idle_animation"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    });
    let click = skin.click_animation.clone().or_else(|| {
        meta.as_ref()
            .and_then(|m| m.get("click_animation"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    });
    let drag = meta
        .as_ref()
        .and_then(|m| m.get("drag_animation"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let boot = meta
        .as_ref()
        .and_then(|m| m.get("boot_animation"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let mut random_animations = meta
        .as_ref()
        .and_then(|m| m.get("random_animations"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if random_animations.is_empty() {
        // 无显式 random 池时，从 animations 里挑 idle/click/drag/boot 以外的动作
        let reserved = [idle.as_deref(), click.as_deref(), drag.as_deref(), boot.as_deref()];
        random_animations = animations
            .iter()
            .filter(|a| {
                let n = a.to_lowercase();
                !reserved
                    .iter()
                    .flatten()
                    .any(|r| r.eq_ignore_ascii_case(a))
                    && !n.contains("idle")
                    && !n.contains("login")
                    && !n.contains("home")
            })
            .cloned()
            .take(8)
            .collect();
    }
    let random_min_sec = meta
        .as_ref()
        .and_then(|m| m.get("random_min_sec"))
        .and_then(|v| v.as_i64())
        .unwrap_or(45);
    let random_max_sec = meta
        .as_ref()
        .and_then(|m| m.get("random_max_sec"))
        .and_then(|v| v.as_i64())
        .unwrap_or(120)
        .max(random_min_sec);
    let abs_dir = {
        let root = kanmusu_model_dir(data_dir, &skin.model_dir);
        root.canonicalize()
            .unwrap_or(root)
            .to_string_lossy()
            .replace('\\', "/")
    };
    Ok(KanmusuPlayerLoadPayload {
        skin_id: skin.id.clone(),
        skin_name: skin.name.clone(),
        model_dir: skin.model_dir.clone(),
        model3_path,
        model_abs_dir: abs_dir,
        lines: skin.lines.clone(),
        idle_animation: idle,
        click_animation: click,
        drag_animation: drag,
        boot_animation: boot,
        random_animations,
        random_min_sec,
        random_max_sec,
        animations,
        touch_areas: read_touch_areas(data_dir, &skin.model_dir),
    })
}

fn read_touch_areas(data_dir: &Path, model_dir: &str) -> Vec<KanmusuTouchArea> {
    let path = kanmusu_model_dir(data_dir, model_dir).join("touch_areas.json");
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    let Some(areas) = v.get("areas").and_then(|a| a.as_array()) else {
        return Vec::new();
    };
    areas
        .iter()
        .filter_map(|a| {
            let id = a.get("id")?.as_str()?.to_string();
            let zone = a
                .get("zone")
                .and_then(|z| z.as_str())
                .unwrap_or("body")
                .to_string();
            let click_animation = a
                .get("click_animation")
                .and_then(|c| c.as_str())
                .map(str::to_string);
            let priority = a
                .get("priority")
                .and_then(|p| p.as_i64())
                .map(|p| p as i32)
                .unwrap_or_else(|| touch_priority_for_zone(&zone));
            let attachments = a
                .get("attachments")
                .and_then(|atts| atts.as_array())
                .map(|atts| {
                    atts.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| vec![id.clone()]);
            let bounds = a
                .get("bounds")
                .map(|b| KanmusuTouchBounds {
                    x: b.get("x").and_then(|x| x.as_f64()).unwrap_or(0.2),
                    y: b.get("y").and_then(|y| y.as_f64()).unwrap_or(0.2),
                    width: b.get("width").and_then(|w| w.as_f64()).unwrap_or(0.6),
                    height: b.get("height").and_then(|h| h.as_f64()).unwrap_or(0.6),
                })
                .unwrap_or_default();
            Some(KanmusuTouchArea {
                id,
                zone,
                click_animation,
                priority,
                attachments,
                bounds,
            })
        })
        .collect()
}

fn validate_model_relpath(filename: &str) -> Result<PathBuf, String> {
    let name = filename.trim().replace('\\', "/");
    if name.is_empty() || name.contains("..") || name.starts_with('/') {
        return Err("非法文件名".into());
    }
    let parts: Vec<&str> = name.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Err("非法文件名".into());
    }
    for part in &parts {
        if part.contains(':') || *part == "." {
            return Err("非法文件名".into());
        }
    }
    Ok(parts.iter().fold(PathBuf::new(), |mut acc, p| {
        acc.push(p);
        acc
    }))
}

fn resolve_model_asset_path(data_dir: &Path, model_dir: &str, filename: &str) -> Result<PathBuf, String> {
    let rel = validate_model_relpath(filename)?;
    let root = kanmusu_model_dir(data_dir, model_dir);
    let path = root.join(&rel);
    // rel 已拒 `..`；热路径跳过 canonicalize（Windows 上每个 moc/贴图都会卡一下）
    if !path.is_file() {
        return Err(format!("文件不存在: {filename}"));
    }
    Ok(path)
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuModelAssetBundle {
    pub files: std::collections::HashMap<String, String>,
}

/// model3 原文 + 首屏必要二进制（一次 IPC，避免 model3→再 bundle 两趟）
#[derive(Debug, Clone, Serialize)]
pub struct KanmusuPrimeModelPayload {
    pub model3_json: String,
    pub files: std::collections::HashMap<String, String>,
}

pub fn read_model_asset_b64(
    data_dir: &Path,
    model_dir: &str,
    filename: &str,
) -> Result<String, String> {
    let path = resolve_model_asset_path(data_dir, model_dir, filename)?;
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(data))
}

fn encode_asset_file(data_dir: &Path, model_dir: &str, filename: &str) -> Result<(String, String), String> {
    let key = filename.trim().replace('\\', "/");
    let path = resolve_model_asset_path(data_dir, model_dir, &key)?;
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    Ok((key, base64::engine::general_purpose::STANDARD.encode(data)))
}

pub fn read_model_asset_bundle(
    data_dir: &Path,
    model_dir: &str,
    filenames: &[String],
) -> Result<KanmusuModelAssetBundle, String> {
    let mut files = std::collections::HashMap::new();
    // 小并行：贴图+moc+少数 motion；单文件时直接走
    if filenames.len() <= 1 {
        for filename in filenames {
            let (key, b64) = encode_asset_file(data_dir, model_dir, filename)?;
            files.insert(key, b64);
        }
        return Ok(KanmusuModelAssetBundle { files });
    }
    let results: Result<Vec<(String, String)>, String> = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(filenames.len());
        for filename in filenames {
            let f = filename.clone();
            handles.push(scope.spawn(move || encode_asset_file(data_dir, model_dir, &f)));
        }
        handles
            .into_iter()
            .map(|h| h.join().map_err(|_| "读取资源线程失败".to_string())?)
            .collect()
    });
    for (key, b64) in results? {
        files.insert(key, b64);
    }
    Ok(KanmusuModelAssetBundle { files })
}

const ESSENTIAL_MOTION_HINTS: &[&str] = &[
    "idle",
    "login",
    "home",
    "welcome",
];

const TOUCH_MOTION_HINTS: &[&str] = &["touch_head", "touch_body", "touch_special"];
const MAIN_MOTION_HINTS: &[&str] = &["main_1", "main_2"];

fn motion_hay_matches(hay: &str, hints: &[&str]) -> bool {
    hints.iter().any(|h| hay.contains(h))
}

fn motion_name_matches(file: &str, name: Option<&str>, hints: &[String]) -> bool {
    let hay = format!("{} {}", file, name.unwrap_or("")).to_lowercase();
    hints.iter().any(|h| {
        let t = h.trim().to_lowercase();
        !t.is_empty() && hay.contains(&t)
    }) || ESSENTIAL_MOTION_HINTS
        .iter()
        .any(|h| hay.contains(h))
}

/// 一次读齐 model3 + moc/贴图/物理/关键动作（与前端 essential-first 对齐）
/// 必须优先 idle，否则 Cubism 默认 PartOpacity 可能把多套部件全亮。
pub fn prime_model(
    data_dir: &Path,
    model_dir: &str,
    model3_filename: &str,
    priority_names: &[String],
) -> Result<KanmusuPrimeModelPayload, String> {
    let model3_path = resolve_model_asset_path(data_dir, model_dir, model3_filename)?;
    let model3_json = fs::read_to_string(&model3_path).map_err(|e| e.to_string())?;
    let v: serde_json::Value =
        serde_json::from_str(&model3_json).map_err(|e| format!("解析 model3 失败: {e}"))?;
    let refs = v
        .get("FileReferences")
        .ok_or_else(|| "model3 缺少 FileReferences".to_string())?;

    let mut want: Vec<String> = Vec::new();
    if let Some(moc) = refs.get("Moc").and_then(|x| x.as_str()) {
        want.push(moc.replace('\\', "/"));
    }
    if let Some(tex) = refs.get("Textures").and_then(|x| x.as_array()) {
        for t in tex {
            if let Some(s) = t.as_str() {
                want.push(s.replace('\\', "/"));
            }
        }
    }
    if let Some(phys) = refs.get("Physics").and_then(|x| x.as_str()) {
        want.push(phys.replace('\\', "/"));
    }

    let mut essential_motion_count = 0usize;
    const MAX_ESSENTIAL_MOTIONS: usize = 8;
    let mut seen_files: std::collections::HashSet<String> = std::collections::HashSet::new();

    let push_motion = |file: String,
                       want: &mut Vec<String>,
                       seen: &mut std::collections::HashSet<String>,
                       count: &mut usize|
     -> bool {
        if seen.contains(&file) {
            return true;
        }
        if *count >= MAX_ESSENTIAL_MOTIONS {
            return false;
        }
        seen.insert(file.clone());
        want.push(file);
        *count += 1;
        true
    };

    if let Some(motions) = refs.get("Motions").and_then(|x| x.as_object()) {
        // Pass 1: idle / boot / login — 决定部件显隐
        for (group, list) in motions {
            let Some(arr) = list.as_array() else { continue };
            for item in arr {
                let file = item
                    .get("File")
                    .and_then(|f| f.as_str())
                    .map(|s| s.trim().replace('\\', "/"))
                    .filter(|s| !s.is_empty());
                let Some(file) = file else { continue };
                let name = item.get("Name").and_then(|n| n.as_str());
                let hay = format!("{} {}", file, name.unwrap_or("")).to_lowercase();
                let prefer = group.eq_ignore_ascii_case("idle")
                    || motion_hay_matches(&hay, ESSENTIAL_MOTION_HINTS)
                    || motion_name_matches(&file, name, priority_names)
                        && (hay.contains("idle")
                            || hay.contains("login")
                            || hay.contains("home")
                            || hay.contains("welcome"));
                if prefer
                    && !push_motion(
                        file,
                        &mut want,
                        &mut seen_files,
                        &mut essential_motion_count,
                    )
                {
                    break;
                }
            }
        }

        // Pass 2: touch_*
        for (_group, list) in motions {
            let Some(arr) = list.as_array() else { continue };
            for item in arr {
                let file = item
                    .get("File")
                    .and_then(|f| f.as_str())
                    .map(|s| s.trim().replace('\\', "/"))
                    .filter(|s| !s.is_empty());
                let Some(file) = file else { continue };
                let name = item.get("Name").and_then(|n| n.as_str());
                let hay = format!("{} {}", file, name.unwrap_or("")).to_lowercase();
                let prefer = motion_hay_matches(&hay, TOUCH_MOTION_HINTS)
                    || motion_name_matches(&file, name, priority_names)
                        && hay.contains("touch_");
                if prefer
                    && !push_motion(
                        file,
                        &mut want,
                        &mut seen_files,
                        &mut essential_motion_count,
                    )
                {
                    break;
                }
            }
        }

        // Pass 3: main_*
        for (_group, list) in motions {
            let Some(arr) = list.as_array() else { continue };
            for item in arr {
                let file = item
                    .get("File")
                    .and_then(|f| f.as_str())
                    .map(|s| s.trim().replace('\\', "/"))
                    .filter(|s| !s.is_empty());
                let Some(file) = file else { continue };
                let name = item.get("Name").and_then(|n| n.as_str());
                let hay = format!("{} {}", file, name.unwrap_or("")).to_lowercase();
                if motion_hay_matches(&hay, MAIN_MOTION_HINTS)
                    && !push_motion(
                        file,
                        &mut want,
                        &mut seen_files,
                        &mut essential_motion_count,
                    )
                {
                    break;
                }
            }
        }

        // Pass 4: other priority_names
        for (_group, list) in motions {
            let Some(arr) = list.as_array() else { continue };
            for item in arr {
                let file = item
                    .get("File")
                    .and_then(|f| f.as_str())
                    .map(|s| s.trim().replace('\\', "/"))
                    .filter(|s| !s.is_empty());
                let Some(file) = file else { continue };
                let name = item.get("Name").and_then(|n| n.as_str());
                if motion_name_matches(&file, name, priority_names)
                    && !push_motion(
                        file,
                        &mut want,
                        &mut seen_files,
                        &mut essential_motion_count,
                    )
                {
                    break;
                }
            }
        }

        if essential_motion_count == 0 {
            'fallback: for (_group, list) in motions {
                let Some(arr) = list.as_array() else { continue };
                if let Some(file) = arr
                    .first()
                    .and_then(|item| item.get("File"))
                    .and_then(|f| f.as_str())
                    .map(|s| s.trim().replace('\\', "/"))
                    .filter(|s| !s.is_empty())
                {
                    if push_motion(
                        file,
                        &mut want,
                        &mut seen_files,
                        &mut essential_motion_count,
                    ) && essential_motion_count >= 2
                    {
                        break 'fallback;
                    }
                }
            }
        }
    }

    want.sort();
    want.dedup();
    let bundle = read_model_asset_bundle(data_dir, model_dir, &want)?;
    Ok(KanmusuPrimeModelPayload {
        model3_json,
        files: bundle.files,
    })
}

fn player_webview_url() -> WebviewUrl {
    WebviewUrl::App("kanmusu-player.html".into())
}

pub fn ensure_player_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(KANMUSU_PLAYER_LABEL) {
        return Ok(w);
    }
    let app_for_load = app.clone();
    WebviewWindowBuilder::new(app, KANMUSU_PLAYER_LABEL, player_webview_url())
        .title("舰娘预览")
        .inner_size(DEFAULT_WIDTH, DEFAULT_HEIGHT)
        .decorations(true)
        .resizable(true)
        .visible(false)
        .focused(false)
        .on_page_load(move |window, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            if let Some(rt) = app_for_load.try_state::<KanmusuRuntimeState>() {
                rt.page_load_finished.store(true, Ordering::Release);
                if let Ok(mut guard) = rt.pending_load.lock() {
                    if let Some(load) = guard.take() {
                        let _ = window.emit("kanmusu-player-load", &load);
                    }
                }
            }
        })
        .build()
        .map_err(|e| e.to_string())
}

pub fn consume_pending_player_load(app: &AppHandle) -> Option<KanmusuPlayerLoadPayload> {
    let rt = app.try_state::<KanmusuRuntimeState>()?;
    let mut guard = rt.pending_load.lock().ok()?;
    guard.take()
}

pub fn player_open(app: &AppHandle) -> Result<(), String> {
    let window = ensure_player_window(app)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn player_close(app: &AppHandle) -> Result<(), String> {
    // 重置 runtime，避免下次 ensure 误用旧 pending / page_ready
    if let Some(rt) = app.try_state::<KanmusuRuntimeState>() {
        rt.page_load_finished.store(false, Ordering::Release);
        if let Ok(mut guard) = rt.pending_load.lock() {
            *guard = None;
        }
    }
    if let Some(window) = app.get_webview_window(KANMUSU_PLAYER_LABEL) {
        // 单次 destroy：hide+close 双重拆 WebView，易在退出时刷 Chrome_WidgetWin_0 / Error 1412
        let _ = window.destroy();
    }
    Ok(())
}

fn queue_or_emit_kanmusu_load(
    app: &AppHandle,
    payload: KanmusuPlayerLoadPayload,
) -> Result<(), String> {
    // 始终保留 pending：页面 Finished 时/前端 bootstrap 的 consume 都可拿到。
    // 勿在 emit 前 take —— 监听尚未注册时事件会丢，透明空窗。
    if let Some(rt) = app.try_state::<KanmusuRuntimeState>() {
        *rt.pending_load.lock().map_err(|e| e.to_string())? = Some(payload.clone());
        let kanmusu_page_ready = rt.page_load_finished.load(Ordering::Acquire)
            && app
                .try_state::<pet::PetRuntimeState>()
                .and_then(|prt| {
                    prt.loaded_companion_engine
                        .lock()
                        .ok()
                        .map(|g| g.as_str() == pet::COMPANION_ENGINE_KANMUSU)
                })
                .unwrap_or(false);
        if kanmusu_page_ready {
            let _ = app.emit_to(PET_LABEL, "kanmusu-player-load", &payload);
        }
    } else {
        app.emit_to(PET_LABEL, "kanmusu-player-load", &payload)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 独立带边框预览窗（不顶替桌宠）
pub fn preview_open(
    app: &AppHandle,
    st: &Arc<AppState>,
    character_id: &str,
    skin_id: &str,
) -> Result<(), String> {
    let data_dir = st.data_dir();
    let skin = lookup_skin_detail(data_dir, character_id, skin_id)?;
    if !skin.model_ready {
        return Err("该皮肤的舰娘模型未就绪，请先同步解包资源".into());
    }
    let local_ready = has_moc3(&kanmusu_model_dir(data_dir, &skin.model_dir));
    if !local_ready {
        let _ = refresh_model_from_unpacked(data_dir, &skin.model_dir)?;
    } else {
        let data_dir_bg = data_dir.to_path_buf();
        let slug = skin.model_dir.clone();
        std::thread::spawn(move || {
            let _ = refresh_model_from_unpacked(&data_dir_bg, &slug);
        });
    }
    let payload = build_player_load_payload_for_skin(data_dir, &skin)?;

    let existed = app.get_webview_window(KANMUSU_PLAYER_LABEL).is_some();
    if let Some(rt) = app.try_state::<KanmusuRuntimeState>() {
        if !existed {
            rt.page_load_finished.store(false, Ordering::Release);
        }
        *rt.pending_load.lock().map_err(|e| e.to_string())? = Some(payload.clone());
    }

    let window = ensure_player_window(app)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;

    if existed {
        app.emit_to(KANMUSU_PLAYER_LABEL, "kanmusu-player-load", &payload)
            .map_err(|e| e.to_string())?;
    } else if let Some(rt) = app.try_state::<KanmusuRuntimeState>() {
        // 首开：页面 load 完成后由前端 consume_pending；若已 finished 则立即 emit
        if rt.page_load_finished.load(Ordering::Acquire) {
            app.emit_to(KANMUSU_PLAYER_LABEL, "kanmusu-player-load", &payload)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Put kanmusu Cubism on the shared pet desktop window (replaces Spine).
pub fn desktop_open(
    app: &AppHandle,
    st: &Arc<AppState>,
    character_id: &str,
    skin_id: &str,
) -> Result<(), String> {
    // 独立预览窗与桌宠分离；上桌时关掉预览避免双实例抢菜单
    let _ = player_close(app);
    let data_dir = st.data_dir();
    let skin = lookup_skin_detail(data_dir, character_id, skin_id)?;
    if !skin.model_ready {
        return Err("该皮肤的舰娘模型未就绪，请先同步解包资源".into());
    }
    let prev_same_skin = {
        let db = crate::db::lock_conn(&st.db)?;
        pet::get_companion_engine(&db) == COMPANION_ENGINE_KANMUSU
            && pet::get_kanmusu_active_character_id(&db).as_deref() == Some(character_id)
            && pet::get_kanmusu_active_skin_id(&db).as_deref() == Some(skin_id)
    };

    let already_kanmusu_page = app.get_webview_window(PET_LABEL).is_some()
        && app
            .try_state::<pet::PetRuntimeState>()
            .and_then(|rt| {
                rt.loaded_companion_engine
                    .lock()
                    .ok()
                    .map(|g| g.as_str() == COMPANION_ENGINE_KANMUSU)
            })
            .unwrap_or(false)
        && app
            .try_state::<KanmusuRuntimeState>()
            .map(|rt| rt.page_load_finished.load(Ordering::Acquire))
            .unwrap_or(false);

    // AppData 已有 moc3：不堵 fingerprint；缺模时才同步等待
    let local_ready = has_moc3(&kanmusu_model_dir(data_dir, &skin.model_dir));
    if !local_ready {
        let _ = refresh_model_from_unpacked(data_dir, &skin.model_dir)?;
    } else if !already_kanmusu_page {
        let data_dir_bg = data_dir.to_path_buf();
        let slug = skin.model_dir.clone();
        std::thread::spawn(move || {
            let _ = refresh_model_from_unpacked(&data_dir_bg, &slug);
        });
    }
    let payload = build_player_load_payload_for_skin(data_dir, &skin)?;

    {
        let db = crate::db::lock_conn(&st.db)?;
        pet::set_companion_engine(&db, COMPANION_ENGINE_KANMUSU)?;
        pet::set_kanmusu_active_ids(&db, character_id, skin_id)?;
        let _ = crate::db::set_setting(&db, "pet_enabled", "1");
    }

    if already_kanmusu_page {
        // 同上皮肤：跳过全量重载，只刷菜单
        if prev_same_skin {
            let _ = app.emit_to(PET_MENU_LABEL, "pet-menu-refresh-pickers", ());
            return Ok(());
        }
        // 热切换：只换模型，不销毁 pet 窗（对齐 Spine in-place switch）
        if let Some(rt) = app.try_state::<KanmusuRuntimeState>() {
            *rt.pending_load.lock().map_err(|e| e.to_string())? = Some(payload.clone());
        }
        // 已可见则不必再走完整 show_pet
        if let Some(pet) = app.get_webview_window(PET_LABEL) {
            let _ = pet.show();
        }
        queue_or_emit_kanmusu_load(app, payload)?;
        let _ = app.emit_to(PET_MENU_LABEL, "pet-menu-refresh-pickers", ());
        // 后台刷新不影响远程感（失败忽略）
        let data_dir_bg = data_dir.to_path_buf();
        let slug = skin.model_dir.clone();
        std::thread::spawn(move || {
            let _ = refresh_model_from_unpacked(&data_dir_bg, &slug);
        });
        return Ok(());
    }

    if let Some(rt) = app.try_state::<KanmusuRuntimeState>() {
        rt.page_load_finished.store(false, Ordering::Release);
        *rt.pending_load.lock().map_err(|e| e.to_string())? = Some(payload.clone());
    }

    // 冷启动 / 从 Spine 切来：重建到 kanmusu-player
    if app.get_webview_window(PET_LABEL).is_some() {
        let _ = pet::destroy_pet_window(app);
    }
    pet::apply_companion_engine(app, st, COMPANION_ENGINE_KANMUSU)?;
    pet::show_pet(app, st)?;
    queue_or_emit_kanmusu_load(app, payload)?;
    let _ = app.emit_to(PET_MENU_LABEL, "pet-menu-refresh-pickers", ());
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuMenuSkinInfo {
    pub id: String,
    pub name: String,
    pub model_id: String,
    pub model_name: String,
    pub active: bool,
    pub model_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct KanmusuMenuSkinsPayload {
    pub character_id: String,
    pub character_name: String,
    pub model_id: String,
    pub skins: Vec<KanmusuMenuSkinInfo>,
}

pub fn menu_list_brief(data_dir: &Path) -> Result<Vec<KanmusuCharacterBrief>, String> {
    ensure_seeded(data_dir)?;
    list_brief(data_dir)
}

pub fn menu_skins_for(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: Option<&str>,
) -> Result<KanmusuMenuSkinsPayload, String> {
    ensure_seeded(data_dir)?;
    let active_char = pet::get_kanmusu_active_character_id(db);
    let active_skin = pet::get_kanmusu_active_skin_id(db);
    let char_id = if let Some(id) = character_id.map(str::trim).filter(|s| !s.is_empty()) {
        id.to_string()
    } else if let Some(id) = active_char.clone() {
        id
    } else {
        let brief = list_brief(data_dir)?;
        brief
            .first()
            .map(|c| c.id.clone())
            .ok_or_else(|| "没有可切换的舰娘角色".to_string())?
    };
    let detail = get_detail(data_dir, &char_id)?;
    let skins = detail
        .skins
        .iter()
        .map(|s| KanmusuMenuSkinInfo {
            id: s.id.clone(),
            name: s.name.clone(),
            model_id: s.model_dir.clone(),
            model_name: s.name.clone(),
            active: active_skin.as_deref() == Some(s.id.as_str())
                && active_char.as_deref() == Some(char_id.as_str()),
            model_ready: s.model_ready,
        })
        .collect();
    Ok(KanmusuMenuSkinsPayload {
        character_id: detail.id,
        character_name: detail.name,
        model_id: active_skin.unwrap_or_default(),
        skins,
    })
}

pub fn menu_switch_skin(
    app: &AppHandle,
    st: &Arc<AppState>,
    character_id: &str,
    skin_id: &str,
) -> Result<(), String> {
    desktop_open(app, st, character_id, skin_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_slug_numeric_suffix() {
        let (base, name) = split_slug("aidang_2");
        assert_eq!(base, "aidang");
        assert_eq!(name, "皮肤 2");
    }

    #[test]
    fn split_slug_no_suffix() {
        let (base, name) = split_slug("chaijun");
        assert_eq!(base, "chaijun");
        assert_eq!(name, "chaijun");
    }
}
