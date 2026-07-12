//! 人物：性格（persona）+ 皮肤（skin → pet model）统一入口

pub mod avatar;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::db::character_profiles::CharacterProfileData;
use crate::persona::{self, PersonaManifest};
use crate::pet::models::{self, PetModelInfo};

const EMBEDDED_MANIFEST: &str = crate::embedded::CHARACTERS_MANIFEST;

const ACTIVE_CHARACTER_KEY: &str = "active_character_id";
const ACTIVE_SKIN_KEY: &str = "active_skin_id";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterManifest {
    pub version: u32,
    pub default_id: String,
    pub characters: Vec<CharacterMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterMeta {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub description: String,
    pub persona_id: String,
    pub skins: Vec<CharacterSkinMeta>,
    /// 非当前选用人物时，详情页展示/切换的皮肤偏好
    #[serde(default)]
    pub preferred_skin_id: Option<String>,
    #[serde(default)]
    pub faction: String,
    #[serde(default)]
    pub ship_type: String,
    #[serde(default)]
    pub rarity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSkinMeta {
    pub id: String,
    pub name: String,
    pub model_id: String,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterSkinInfo {
    pub id: String,
    pub name: String,
    pub model_id: String,
    pub model_name: String,
    pub active: bool,
    /// 对应 Spine 模型目录已存在（内置模型恒为 true）
    pub model_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterInfo {
    pub id: String,
    pub name: String,
    pub source: String,
    pub description: String,
    pub persona_id: String,
    pub active: bool,
    pub active_skin_id: String,
    pub skins: Vec<CharacterSkinInfo>,
    pub is_builtin: bool,
}

/// 桌宠右键「切换模型」：当前桌宠模型所属角色的可切换皮肤（与收藏无关）
#[derive(Debug, Clone, Serialize)]
pub struct PetMenuSkinsPayload {
    pub character_id: String,
    pub character_name: String,
    pub model_id: String,
    pub skins: Vec<CharacterSkinInfo>,
}

/// 人物列表轻量项（不含皮肤详情，用于分页/懒加载）
#[derive(Debug, Clone, Serialize)]
pub struct CharacterBrief {
    pub id: String,
    pub name: String,
    pub source: String,
    pub description: String,
    pub persona_id: String,
    pub active: bool,
    pub active_skin_id: String,
    pub active_skin_name: String,
    pub skin_count: usize,
    pub is_builtin: bool,
    pub faction: String,
    pub ship_type: String,
    pub rarity: String,
    pub trait_summary: String,
    pub avatar_path: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterListPage {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub items: Vec<CharacterBrief>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterSkinsPage {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub active_skin_id: String,
    pub items: Vec<CharacterSkinInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterDetail {
    pub id: String,
    pub name: String,
    pub source: String,
    pub description: String,
    pub persona_id: String,
    pub active: bool,
    pub active_skin_id: String,
    pub active_skin_name: String,
    pub active_model_id: String,
    pub active_model_name: String,
    pub active_model_ready: bool,
    pub skin_count: usize,
    pub is_builtin: bool,
    pub faction: String,
    pub ship_type: String,
    pub rarity: String,
    pub trait_summary: String,
    pub avatar_path: Option<String>,
    pub avatar_url: Option<String>,
    pub skill_md: String,
    pub profile_json: CharacterProfileData,
    pub has_profile: bool,
    pub profile_ai_updated: bool,
    pub profile_ai_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterWikiImportResult {
    pub message: String,
    pub lines_imported: u32,
    pub persona_id: String,
}

pub fn characters_dir(data_dir: &Path) -> PathBuf {
    crate::data_layout::characters_dir(data_dir)
}

pub fn manifest_path(data_dir: &Path) -> PathBuf {
    crate::data_layout::characters_manifest_path(data_dir)
}

pub fn seed_user_characters(data_dir: &Path) -> std::io::Result<()> {
    let dir = characters_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let manifest = manifest_path(data_dir);
    if !manifest.exists() {
        fs::write(&manifest, EMBEDDED_MANIFEST)?;
    }
    Ok(())
}

fn embedded_manifest() -> CharacterManifest {
    serde_json::from_str(EMBEDDED_MANIFEST).expect("embedded characters/manifest.json")
}

pub fn load_manifest(data_dir: &Path) -> CharacterManifest {
    let path = manifest_path(data_dir);
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(m) = serde_json::from_str(&raw) {
            return m;
        }
    }
    embedded_manifest()
}

fn write_manifest(data_dir: &Path, manifest: &CharacterManifest) -> Result<(), String> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    crate::manifest_lock::atomic_write(&manifest_path(data_dir), &json)
}

pub(crate) fn save_manifest(data_dir: &Path, manifest: &CharacterManifest) -> Result<(), String> {
    crate::manifest_lock::with_lock(|| write_manifest(data_dir, manifest))
}

pub(crate) fn mutate_character_manifest<F, T>(data_dir: &Path, f: F) -> Result<T, String>
where
    F: FnOnce(&mut CharacterManifest) -> Result<T, String>,
{
    crate::manifest_lock::with_lock(|| {
        let mut manifest = load_manifest(data_dir);
        let result = f(&mut manifest)?;
        write_manifest(data_dir, &manifest)?;
        Ok(result)
    })
}

fn is_builtin_character(id: &str) -> bool {
    embedded_manifest()
        .characters
        .iter()
        .any(|c| c.id == id)
}

fn default_skin(meta: &CharacterMeta) -> Option<&CharacterSkinMeta> {
    meta.skins
        .iter()
        .find(|s| s.default)
        .or_else(|| meta.skins.first())
}

fn resolve_preferred_skin_id(meta: &CharacterMeta) -> String {
    if let Some(ref pref) = meta.preferred_skin_id {
        if meta.skins.iter().any(|s| s.id == *pref) {
            return pref.clone();
        }
    }
    default_skin(meta)
        .map(|s| s.id.clone())
        .unwrap_or_else(|| "default".into())
}

fn resolve_character_skin_id(
    meta: &CharacterMeta,
    db: &rusqlite::Connection,
    global_active_id: &str,
) -> String {
    if meta.id == global_active_id {
        resolve_active_skin(meta, db)
    } else {
        resolve_preferred_skin_id(meta)
    }
}

/// 与皮肤分页列表一致：在 normalize 后解析应展示/高亮的皮肤 id（调用方应已 normalize skins）
fn resolve_display_skin_id(
    meta: &CharacterMeta,
    db: &rusqlite::Connection,
    global_active_id: &str,
) -> String {
    let mut skin_id = resolve_character_skin_id(meta, db, global_active_id);
    if !meta.skins.iter().any(|s| s.id == skin_id) {
        skin_id = meta
            .skins
            .iter()
            .find(|s| s.default)
            .or_else(|| meta.skins.first())
            .map(|s| s.id.clone())
            .unwrap_or(skin_id);
    }
    skin_id
}

fn set_preferred_skin_in_manifest(
    data_dir: &Path,
    character_id: &str,
    skin_id: &str,
) -> Result<(), String> {
    mutate_character_manifest(data_dir, |manifest| {
        let character = manifest
            .characters
            .iter_mut()
            .find(|c| c.id == character_id)
            .ok_or_else(|| format!("人物 {character_id} 不在 manifest 中"))?;
        if !character.skins.iter().any(|s| s.id == skin_id) {
            return Err(format!("未知皮肤: {skin_id}"));
        }
        character.preferred_skin_id = Some(skin_id.to_string());
        Ok(())
    })
}

fn model_is_ready(data_dir: &Path, model_id: &str, model_ids: Option<&HashSet<String>>) -> bool {
    if model_id.is_empty() {
        return false;
    }
    if models::is_builtin_model(model_id) {
        return true;
    }
    if let Some(ids) = model_ids {
        let canonical = models::canonical_model_id(model_id);
        if ids.contains(model_id) || ids.contains(canonical.as_str()) {
            return true;
        }
    }
    models::resolve_assets(data_dir, model_id).is_ok()
}

/// 去重并移除「已有可用模型时仍显示的默认占位皮肤」
fn normalize_character_skins(
    data_dir: &Path,
    skins: &[CharacterSkinMeta],
    model_ids: Option<&HashSet<String>>,
) -> Vec<CharacterSkinMeta> {
    let mut out: Vec<CharacterSkinMeta> = Vec::new();
    let mut seen_model_ids: HashSet<String> = HashSet::new();

    for skin in skins {
        if !seen_model_ids.insert(skin.model_id.clone()) {
            continue;
        }
        out.push(skin.clone());
    }

    let has_ready = out
        .iter()
        .any(|s| model_is_ready(data_dir, &s.model_id, model_ids));
    if has_ready {
        out.retain(|s| model_is_ready(data_dir, &s.model_id, model_ids));
    }

    if out.is_empty() {
        return skins.to_vec();
    }
    out
}

pub fn repair_character_manifest_skins(data_dir: &Path, manifest: &mut CharacterManifest) -> bool {
    let mut changed = false;
    for c in &mut manifest.characters {
        let normalized = normalize_character_skins(data_dir, &c.skins, None);
        if normalized.len() != c.skins.len()
            || normalized
                .iter()
                .zip(c.skins.iter())
                .any(|(a, b)| a.id != b.id || a.model_id != b.model_id)
        {
            c.skins = normalized;
            changed = true;
        }

        if let Some(ref pref) = c.preferred_skin_id {
            if !c.skins.iter().any(|s| s.id == *pref) {
                c.preferred_skin_id = c
                    .skins
                    .iter()
                    .find(|s| s.default)
                    .or_else(|| c.skins.first())
                    .map(|s| s.id.clone());
                changed = true;
            }
        }

        if c.skins.iter().any(|s| s.default && model_is_ready(data_dir, &s.model_id, None)) {
            continue;
        }
        if let Some(ready_idx) = c
            .skins
            .iter()
            .position(|s| model_is_ready(data_dir, &s.model_id, None))
        {
            for s in &mut c.skins {
                s.default = false;
            }
            c.skins[ready_idx].default = true;
            changed = true;
        }
    }
    changed
}

fn default_skin_model_id(character_id: &str) -> String {
    character_id.to_string()
}

fn build_skin_info(
    data_dir: &Path,
    s: &CharacterSkinMeta,
    active_skin_id: &str,
    model_cache: Option<&HashMap<String, String>>,
    model_ids: Option<&HashSet<String>>,
) -> CharacterSkinInfo {
    CharacterSkinInfo {
        id: s.id.clone(),
        name: s.name.clone(),
        model_id: models::canonical_model_id(&s.model_id),
        model_name: if let Some(cache) = model_cache {
            model_name_cached(&s.model_id, cache)
        } else {
            model_name(data_dir, &s.model_id)
        },
        active: s.id == active_skin_id,
        model_ready: model_is_ready(data_dir, &s.model_id, model_ids),
    }
}

fn model_name(data_dir: &Path, model_id: &str) -> String {
    models::model_display_name(data_dir, model_id)
}

fn model_name_cached(
    model_id: &str,
    cache: &std::collections::HashMap<String, String>,
) -> String {
    cache
        .get(model_id)
        .cloned()
        .unwrap_or_else(|| model_id.to_string())
}

fn skin_infos_with_cache(
    data_dir: &Path,
    meta: &CharacterMeta,
    active_skin_id: &str,
    model_cache: Option<&std::collections::HashMap<String, String>>,
) -> Vec<CharacterSkinInfo> {
    normalize_character_skins(data_dir, &meta.skins, None)
        .iter()
        .map(|s| build_skin_info(data_dir, s, active_skin_id, model_cache, None))
        .collect()
}

/// 合并 manifest 与仅存在于 persona 的条目（单皮肤默认模型）
/// 已有人物条目会从 persona manifest 同步 name/source/description
fn load_resolved_roster(data_dir: &Path, model_ids: &HashSet<String>) -> Vec<CharacterMeta> {
    let mut manifest = load_manifest(data_dir);
    let persona_manifest = persona::load_manifest(data_dir);
    let persona_by_id: HashMap<&str, &persona::PersonaMeta> = persona_manifest
        .personas
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();

    for c in &mut manifest.characters {
        if let Some(p) = persona_by_id.get(c.persona_id.as_str()) {
            c.name = p.name.clone();
            c.source = p.source.clone();
            c.description = p.description.clone();
        }
        c.skins = normalize_character_skins(data_dir, &c.skins, Some(model_ids));
    }

    for p in &persona_manifest.personas {
        if manifest.characters.iter().any(|c| c.persona_id == p.id) {
            continue;
        }
        let model_id = default_skin_model_id(&p.id);
        let mut meta = CharacterMeta {
            id: p.id.clone(),
            name: p.name.clone(),
            source: p.source.clone(),
            description: p.description.clone(),
            persona_id: p.id.clone(),
            skins: vec![CharacterSkinMeta {
                id: "default".into(),
                name: "默认".into(),
                model_id,
                default: true,
            }],
            preferred_skin_id: None,
            faction: String::new(),
            ship_type: String::new(),
            rarity: String::new(),
        };
        meta.skins = normalize_character_skins(data_dir, &meta.skins, Some(model_ids));
        manifest.characters.push(meta);
    }
    manifest.characters
}

struct ResolvedRosterCache {
    manifest_fp: u64,
    models_fp: u64,
    characters: Arc<Vec<CharacterMeta>>,
    model_ids: Arc<HashSet<String>>,
}

static RESOLVED_ROSTER: OnceLock<Mutex<ResolvedRosterCache>> = OnceLock::new();

fn manifest_fingerprint(data_dir: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    for path in [manifest_path(data_dir), persona::manifest_path(data_dir)] {
        if let Ok(m) = fs::metadata(&path) {
            m.len().hash(&mut h);
            if let Ok(t) = m.modified() {
                t.hash(&mut h);
            }
        }
    }
    h.finish()
}

fn models_fingerprint(data_dir: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    let models_root = models::models_dir(data_dir);
    if models_root.is_dir() {
        if let Ok(entries) = fs::read_dir(&models_root) {
            let mut names: Vec<_> = entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name())
                .collect();
            names.sort();
            for name in names {
                name.hash(&mut h);
                let sub = models_root.join(&name);
                if let Ok(m) = fs::metadata(&sub) {
                    m.len().hash(&mut h);
                    if let Ok(t) = m.modified() {
                        t.hash(&mut h);
                    }
                }
            }
        }
    }
    h.finish()
}

fn resolved_roster_arc(data_dir: &Path) -> Arc<Vec<CharacterMeta>> {
    let manifest_fp = manifest_fingerprint(data_dir);
    let models_fp = models_fingerprint(data_dir);
    let lock = RESOLVED_ROSTER.get_or_init(|| {
        Mutex::new(ResolvedRosterCache {
            manifest_fp: 0,
            models_fp: 0,
            characters: Arc::new(Vec::new()),
            model_ids: Arc::new(HashSet::new()),
        })
    });
    let mut cache = lock.lock().unwrap_or_else(|e| e.into_inner());
    if cache.manifest_fp != manifest_fp
        || cache.models_fp != models_fp
        || cache.characters.is_empty()
    {
        let model_ids = models::list_model_id_set(data_dir).unwrap_or_default();
        cache.manifest_fp = manifest_fp;
        cache.models_fp = models_fp;
        cache.model_ids = Arc::new(model_ids.clone());
        cache.characters = Arc::new(load_resolved_roster(data_dir, &model_ids));
    }
    Arc::clone(&cache.characters)
}

/// 启动后后台预热人物列表缓存，减少首次打开人物页等待。
pub fn warm_roster_cache(data_dir: &Path) {
    let _ = resolved_roster_arc(data_dir);
    let _ = avatar::cached_avatar_path_index(data_dir);
}

pub fn roster_cache_len(data_dir: &Path) -> usize {
    resolved_roster_arc(data_dir).len()
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterMemoryStats {
    pub roster_cached: usize,
    pub avatar_files: usize,
}

pub fn memory_stats(data_dir: &Path) -> CharacterMemoryStats {
    CharacterMemoryStats {
        roster_cached: roster_cache_len(data_dir),
        avatar_files: avatar::count_avatar_files(data_dir),
    }
}

fn resolve_all_characters(data_dir: &Path) -> Arc<Vec<CharacterMeta>> {
    resolved_roster_arc(data_dir)
}

fn resolved_roster_model_ids(data_dir: &Path) -> Arc<HashSet<String>> {
    let _ = resolved_roster_arc(data_dir);
    let lock = RESOLVED_ROSTER.get().expect("resolved roster init");
    let cache = lock.lock().unwrap_or_else(|e| e.into_inner());
    Arc::clone(&cache.model_ids)
}

/// 人物 id + 显示名（供头像批量同步）
pub fn roster_id_names(data_dir: &Path) -> Vec<(String, String)> {
    resolved_roster_arc(data_dir)
        .iter()
        .map(|c| (c.id.clone(), c.name.clone()))
        .collect()
}

fn sync_meta_from_persona(data_dir: &Path, mut meta: CharacterMeta) -> CharacterMeta {
    let persona_manifest = persona::load_manifest(data_dir);
    if let Some(p) = persona_manifest
        .personas
        .iter()
        .find(|p| p.id == meta.persona_id)
    {
        meta.name = p.name.clone();
        meta.source = p.source.clone();
        meta.description = p.description.clone();
    }
    meta
}

pub fn find_character_meta(data_dir: &Path, character_id: &str) -> Result<CharacterMeta, String> {
    let manifest = load_manifest(data_dir);
    if let Some(c) = manifest.characters.iter().find(|c| c.id == character_id) {
        return Ok(sync_meta_from_persona(data_dir, c.clone()));
    }
    let persona_manifest = persona::load_manifest(data_dir);
    let p = persona_manifest
        .personas
        .iter()
        .find(|p| p.id == character_id)
        .ok_or_else(|| format!("未知人物: {character_id}"))?;
    let model_id = default_skin_model_id(&p.id);
    Ok(CharacterMeta {
        id: p.id.clone(),
        name: p.name.clone(),
        source: p.source.clone(),
        description: p.description.clone(),
        persona_id: p.id.clone(),
        skins: vec![CharacterSkinMeta {
            id: "default".into(),
            name: "默认".into(),
            model_id,
            default: true,
        }],
        preferred_skin_id: None,
        faction: String::new(),
        ship_type: String::new(),
        rarity: String::new(),
    })
}

fn model_id_used_in_manifest(manifest: &CharacterManifest, model_id: &str) -> bool {
    manifest
        .characters
        .iter()
        .flat_map(|c| c.skins.iter())
        .any(|s| s.model_id == model_id)
}

/// 删除 pet 模型后，从人物 manifest 移除引用该 model_id 的皮肤（至少保留一项）
pub fn purge_model_from_manifest(data_dir: &Path, model_id: &str) -> Result<bool, String> {
    mutate_character_manifest(data_dir, |manifest| {
        let mut changed = false;
        for character in &mut manifest.characters {
            if character.skins.len() <= 1 {
                continue;
            }
            let before = character.skins.len();
            character.skins.retain(|s| s.model_id != model_id);
            if character.skins.len() == before {
                continue;
            }
            changed = true;
            if !character.skins.iter().any(|s| s.default) {
                if let Some(first) = character.skins.first_mut() {
                    first.default = true;
                }
            }
            if let Some(ref pref) = character.preferred_skin_id {
                if !character.skins.iter().any(|s| s.id == *pref) {
                    character.preferred_skin_id =
                        character.skins.first().map(|s| s.id.clone());
                }
            }
        }
        Ok(changed)
    })
}

fn character_text_match(c: &CharacterMeta, query_lower: &str) -> bool {
    [
        c.name.as_str(),
        c.source.as_str(),
        c.description.as_str(),
        c.id.as_str(),
        c.persona_id.as_str(),
        c.faction.as_str(),
        c.ship_type.as_str(),
        c.rarity.as_str(),
    ]
    .iter()
    .any(|f| !f.is_empty() && f.to_lowercase().contains(query_lower))
}

fn truncate_chars(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.chars().count() <= max {
        return t.to_string();
    }
    format!("{}…", t.chars().take(max).collect::<String>())
}

fn trait_summary_for(data_dir: &Path, persona_id: &str) -> String {
    persona::load_persona_profile(data_dir, persona_id)
        .map(|p| {
            if !p.personality.is_empty() {
                p.personality.iter().take(2).cloned().collect::<Vec<_>>().join("·")
            } else if !p.speech_style.trim().is_empty() {
                truncate_chars(&p.speech_style, 14)
            } else if !p.introduction.trim().is_empty() {
                truncate_chars(&p.introduction, 18)
            } else {
                String::new()
            }
        })
        .unwrap_or_default()
}

fn brief_trait_summary(c: &CharacterMeta) -> String {
    if !c.description.trim().is_empty() {
        return truncate_chars(&c.description, 18);
    }
    String::new()
}

fn meta_to_brief(
    data_dir: &Path,
    c: &CharacterMeta,
    active_id: &str,
    db: &rusqlite::Connection,
    include_trait: bool,
    avatar_index: Option<&HashMap<String, String>>,
) -> CharacterBrief {
    let skin_id = resolve_display_skin_id(c, db, active_id);
    let skin_meta = c
        .skins
        .iter()
        .find(|s| s.id == skin_id)
        .or_else(|| default_skin(c));
    let (faction, ship_type, rarity) = (
        c.faction.clone(),
        c.ship_type.clone(),
        c.rarity.clone(),
    );
    CharacterBrief {
        id: c.id.clone(),
        name: c.name.clone(),
        source: c.source.clone(),
        description: c.description.clone(),
        persona_id: c.persona_id.clone(),
        active: c.id == active_id,
        active_skin_id: skin_id,
        active_skin_name: skin_meta
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "默认".into()),
        skin_count: c.skins.len(),
        is_builtin: is_builtin_character(&c.id),
        faction,
        ship_type,
        rarity,
        trait_summary: if include_trait {
            trait_summary_for(data_dir, &c.persona_id)
        } else {
            brief_trait_summary(c)
        },
        avatar_path: avatar_index
            .and_then(|m| m.get(&c.id).cloned())
            .or_else(|| avatar::avatar_path_string(data_dir, &c.id)),
        avatar_url: None,
    }
}

fn roster_filter_indices(
    items: &[CharacterMeta],
    query: Option<&str>,
    favorites_only: bool,
    favorite_ids: &[String],
) -> Vec<usize> {
    let fav_set: Option<HashSet<&str>> = if favorites_only {
        Some(favorite_ids.iter().map(|s| s.as_str()).collect())
    } else {
        None
    };
    let mut indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            if let Some(ref set) = fav_set {
                if !set.contains(c.id.as_str()) {
                    return false;
                }
            }
            if let Some(q) = query {
                let q = q.trim();
                if !q.is_empty() {
                    let lower = q.to_lowercase();
                    if !character_text_match(c, &lower) {
                        return false;
                    }
                }
            }
            true
        })
        .map(|(i, _)| i)
        .collect();
    if !favorite_ids.is_empty() {
        let rank: HashMap<&str, usize> = favorite_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        indices.sort_by_key(|i| rank.get(items[*i].id.as_str()).copied().unwrap_or(usize::MAX));
    }
    indices
}

/// 将 personas/manifest 同步写入 characters/manifest（CLI 导入、启动迁移）
pub fn sync_character_manifest_from_personas(data_dir: &Path) -> Result<bool, String> {
    crate::manifest_lock::with_lock(|| {
        let persona_manifest = persona::load_manifest(data_dir);
        let mut manifest = load_manifest(data_dir);
        let mut changed = false;

        changed |= repair_default_skin_models(&mut manifest);
        changed |= repair_character_manifest_skins(data_dir, &mut manifest);

        for p in &persona_manifest.personas {
            if let Some(c) = manifest
                .characters
                .iter_mut()
                .find(|c| c.persona_id == p.id)
            {
                if c.name != p.name || c.source != p.source || c.description != p.description {
                    c.name = p.name.clone();
                    c.source = p.source.clone();
                    c.description = p.description.clone();
                    changed = true;
                }
                continue;
            }

            let model_id = default_skin_model_id(&p.id);
            manifest.characters.push(CharacterMeta {
                id: p.id.clone(),
                name: p.name.clone(),
                source: p.source.clone(),
                description: p.description.clone(),
                persona_id: p.id.clone(),
                skins: vec![CharacterSkinMeta {
                    id: "default".into(),
                    name: "默认".into(),
                    model_id,
                    default: true,
                }],
                preferred_skin_id: None,
                faction: String::new(),
                ship_type: String::new(),
                rarity: String::new(),
            });
            changed = true;
        }

        if changed {
            write_manifest(data_dir, &manifest)?;
        }
        Ok(changed)
    })
}

/// 非内置人物的默认皮肤 model_id 改回角色 id（移除错误的柴郡模型占位）
fn repair_default_skin_models(manifest: &mut CharacterManifest) -> bool {
    let mut changed = false;
    for c in &mut manifest.characters {
        if is_builtin_character(&c.id) {
            continue;
        }
        let skin_idx = c
            .skins
            .iter()
            .position(|s| s.default)
            .unwrap_or(0);
        let Some(skin) = c.skins.get_mut(skin_idx) else {
            continue;
        };
        if skin.model_id == models::BUILTIN_CHAIJUN && c.id != models::BUILTIN_CHAIJUN {
            skin.model_id = c.id.clone();
            changed = true;
        }
    }
    changed
}

fn find_meta<'a>(all: &'a [CharacterMeta], character_id: &str) -> Option<&'a CharacterMeta> {
    all.iter().find(|c| c.id == character_id)
}

fn find_by_persona<'a>(all: &'a [CharacterMeta], persona_id: &str) -> Option<&'a CharacterMeta> {
    all.iter().find(|c| c.persona_id == persona_id)
}

fn find_by_model<'a>(all: &'a [CharacterMeta], model_id: &str) -> Option<(&'a CharacterMeta, &'a CharacterSkinMeta)> {
    for c in all {
        if let Some(s) = c.skins.iter().find(|s| s.model_id == model_id) {
            return Some((c, s));
        }
    }
    None
}

/// 根据模型 ID 反查人物显示名（用于 BWIKI 台词爬取）
pub fn character_name_for_model(data_dir: &Path, model_id: &str) -> Option<String> {
    let all = resolve_all_characters(data_dir);
    find_by_model(&all, model_id)
        .map(|(meta, _)| meta.name.trim().to_string())
        .filter(|name| !name.is_empty())
}

fn strip_numeric_suffix_from_model_id(model_id: &str) -> Option<String> {
    for sep in ['-', '_'] {
        if let Some((base, tail)) = model_id.rsplit_once(sep) {
            if !base.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                return Some(base.to_string());
            }
        }
    }
    None
}

/// unique_id 碰撞产生的纯数字 id（如 `2`、`2-125`）不能用于皮肤前缀继承
fn is_weak_model_id_base(base: &str) -> bool {
    let base = base.trim();
    base.is_empty()
        || base.chars().all(|c| c.is_ascii_digit())
        || base.len() < 3
}

fn model_id_has_unreliable_inheritance(model_id: &str) -> bool {
    if model_id.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '_') {
        if let Some(base) = strip_numeric_suffix_from_model_id(model_id) {
            return is_weak_model_id_base(&base);
        }
        if model_id.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    false
}

#[derive(Debug, Deserialize)]
struct PlanWikiRow {
    #[serde(rename = "modelName")]
    model_name: String,
    #[serde(rename = "wikiTitle")]
    wiki_title: String,
}

#[derive(Debug, Deserialize)]
struct PlanWikiFile {
    plan: Vec<PlanWikiRow>,
}

fn load_plan_display_wiki_map(data_dir: &Path) -> HashMap<String, String> {
    let path = crate::data_layout::live2d_plan_path(data_dir);
    let Ok(raw) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let Ok(file) = serde_json::from_str::<PlanWikiFile>(&raw) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for row in file.plan {
        let model = row.model_name.trim();
        let wiki = row.wiki_title.trim();
        if !model.is_empty() && !wiki.is_empty() {
            map.entry(model.to_string()).or_insert_with(|| wiki.to_string());
        }
    }
    map
}

fn is_skin_suffix_label(label: &str) -> bool {
    let label = label.trim();
    label.starts_with("皮肤")
        || label.starts_with("换装")
        || label.starts_with("变体")
        || matches!(
            label,
            "泳装" | "便服" | "幼女" | "立绘" | "偶像" | "誓约" | "礼服" | "校园" | "赛车"
        )
}

fn strip_one_skin_suffix_from_display_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return String::new();
    }
    for sep in ['·', '・', '•', '．'] {
        if let Some((base, suffix)) = name.rsplit_once(sep) {
            let base = base.trim();
            let suffix = suffix.trim();
            if !base.is_empty() && is_skin_suffix_label(suffix) {
                return base.to_string();
            }
        }
    }
    name.to_string()
}

/// 从模型显示名剥离皮肤/换装后缀，得到 BWIKI 角色词条名候选
pub fn strip_skin_suffix_from_display_name(name: &str) -> String {
    let mut current = name.trim().to_string();
    loop {
        let next = strip_one_skin_suffix_from_display_name(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current.trim().to_string()
}

fn match_roster_wiki_title(candidate: &str, roster: &[String]) -> Option<String> {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return None;
    }
    for name in roster {
        if candidate == name.as_str() {
            return Some(name.clone());
        }
    }
    for name in roster {
        if candidate.starts_with(name.as_str()) {
            let rest = candidate[name.len()..].trim();
            if rest.is_empty() || is_skin_suffix_label(rest.trim_start_matches(['·', '・', '•', '．'])) {
                return Some(name.clone());
            }
        }
    }
    None
}

fn display_name_looks_like_skin_model(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    for sep in ['·', '・', '•', '．'] {
        if let Some((_, suffix)) = name.rsplit_once(sep) {
            if is_skin_suffix_label(suffix.trim()) {
                return true;
            }
        }
    }
    false
}

/// 批量 Wiki 扫描用：一次性构建 model_id → 显示名 / BWIKI 词条索引
pub struct ModelWikiTitleLookup {
    character_names: HashMap<String, String>,
    display_names: HashMap<String, String>,
    wiki_titles: HashMap<String, String>,
    plan_display_wiki: HashMap<String, String>,
    roster_names: Vec<String>,
}

impl ModelWikiTitleLookup {
    pub fn build(data_dir: &Path) -> Self {
        let display_names = models::model_names_map(data_dir);
        let wiki_titles = models::model_wiki_titles_map(data_dir);
        let plan_display_wiki = load_plan_display_wiki_map(data_dir);
        let mut character_names = HashMap::new();
        let mut roster_names = Vec::new();
        for c in resolve_all_characters(data_dir).iter() {
            let name = c.name.trim().to_string();
            if name.is_empty() {
                continue;
            }
            roster_names.push(name.clone());
            for skin in &c.skins {
                character_names.insert(skin.model_id.clone(), name.clone());
            }
            for skin in &c.skins {
                let base = skin.model_id.as_str();
                if is_weak_model_id_base(base) {
                    continue;
                }
                for model_id in display_names.keys() {
                    if model_id == base {
                        continue;
                    }
                    if model_id.starts_with(&format!("{base}-"))
                        || model_id.starts_with(&format!("{base}_"))
                    {
                        character_names
                            .entry(model_id.clone())
                            .or_insert_with(|| name.clone());
                    }
                }
            }
        }
        for title in wiki_titles.values() {
            let title = title.trim();
            if !title.is_empty() && !roster_names.iter().any(|n| n == title) {
                roster_names.push(title.to_string());
            }
        }
        roster_names.sort_by_key(|name| std::cmp::Reverse(name.chars().count()));

        for model_id in display_names.keys() {
            if character_names.contains_key(model_id) {
                continue;
            }
            if let Some(base) = strip_numeric_suffix_from_model_id(model_id) {
                if is_weak_model_id_base(&base) {
                    continue;
                }
                if let Some(name) = character_names.get(&base).cloned() {
                    character_names.insert(model_id.clone(), name);
                }
            }
        }

        Self {
            character_names,
            display_names,
            wiki_titles,
            plan_display_wiki,
            roster_names,
        }
    }

    pub fn display_name(&self, model_id: &str) -> String {
        self.display_names
            .get(model_id)
            .cloned()
            .unwrap_or_else(|| model_id.to_string())
    }

    /// BWIKI 词条名：台词属于角色，不属于皮肤；优先人物名，否则剥离皮肤后缀后的显示名
    pub fn wiki_title(&self, model_id: &str) -> Option<String> {
        if let Some(title) = self.wiki_titles.get(model_id) {
            let n = title.trim();
            if !n.is_empty() {
                return Some(n.to_string());
            }
        }
        if let Some(display) = self.display_names.get(model_id) {
            let display = display.trim();
            if let Some(title) = self.plan_display_wiki.get(display) {
                let n = title.trim();
                if !n.is_empty() {
                    return Some(n.to_string());
                }
            }
        }
        if let Some(name) = self.character_names.get(model_id) {
            if !model_id_has_unreliable_inheritance(model_id) {
                let n = name.trim();
                if !n.is_empty() {
                    return Some(n.to_string());
                }
            }
        }
        if let Some(base) = strip_numeric_suffix_from_model_id(model_id) {
            if !is_weak_model_id_base(&base) {
                if let Some(name) = self.character_names.get(&base) {
                    let n = name.trim();
                    if !n.is_empty() {
                        return Some(n.to_string());
                    }
                }
                if let Some(title) = self.wiki_titles.get(&base) {
                    let n = title.trim();
                    if !n.is_empty() {
                        return Some(n.to_string());
                    }
                }
            }
        }
        let raw = self.display_names.get(model_id)?.trim().to_string();
        if raw.is_empty() || raw == model_id {
            return None;
        }
        if display_name_looks_like_skin_model(&raw) {
            let stripped = strip_skin_suffix_from_display_name(&raw);
            if let Some(matched) = match_roster_wiki_title(&stripped, &self.roster_names) {
                return Some(matched);
            }
            if !stripped.is_empty() && stripped != raw {
                return Some(stripped);
            }
            return None;
        }
        match_roster_wiki_title(&raw, &self.roster_names).or(Some(raw))
    }
}

/// BWIKI 词条名：优先人物名，否则剥离皮肤后缀后的模型显示名
pub fn wiki_title_for_model(data_dir: &Path, model_id: &str) -> Option<String> {
    ModelWikiTitleLookup::build(data_dir).wiki_title(model_id)
}

pub fn active_character_id(db: &rusqlite::Connection, data_dir: &Path) -> String {
    let manifest = load_manifest(data_dir);
    let all = resolve_all_characters(data_dir);
    crate::db::get_setting(db, ACTIVE_CHARACTER_KEY)
        .filter(|s| !s.trim().is_empty())
        .filter(|id| all.iter().any(|c| c.id == *id))
        .unwrap_or_else(|| manifest.default_id.clone())
}

pub fn active_skin_id(db: &rusqlite::Connection) -> Option<String> {
    crate::db::get_setting(db, ACTIVE_SKIN_KEY).filter(|s| !s.trim().is_empty())
}

fn set_active_character_id(db: &rusqlite::Connection, id: &str) -> Result<(), String> {
    crate::db::set_setting(db, ACTIVE_CHARACTER_KEY, id).map_err(|e| e.to_string())
}

fn set_active_skin_id(db: &rusqlite::Connection, id: &str) -> Result<(), String> {
    crate::db::set_setting(db, ACTIVE_SKIN_KEY, id).map_err(|e| e.to_string())
}

fn resolve_active_skin(meta: &CharacterMeta, db: &rusqlite::Connection) -> String {
    if let Some(skin_id) = active_skin_id(db) {
        if meta.skins.iter().any(|s| s.id == skin_id) {
            return skin_id;
        }
    }
    default_skin(meta)
        .map(|s| s.id.clone())
        .unwrap_or_else(|| "default".into())
}

pub fn list_characters(data_dir: &Path, db: &rusqlite::Connection) -> Vec<CharacterInfo> {
    let all = resolve_all_characters(data_dir);
    let model_cache: std::collections::HashMap<String, String> = models::model_names_map(data_dir);
    let active_id = active_character_id(db, data_dir);
    if all.is_empty() {
        return vec![];
    }
    all.iter()
        .map(|c| {
            let skin = resolve_display_skin_id(c, db, &active_id);
            CharacterInfo {
                id: c.id.clone(),
                name: c.name.clone(),
                source: c.source.clone(),
                description: c.description.clone(),
                persona_id: c.persona_id.clone(),
                active: c.id == active_id,
                active_skin_id: skin.clone(),
                skins: skin_infos_with_cache(data_dir, c, &skin, Some(&model_cache)),
                is_builtin: is_builtin_character(&c.id),
            }
        })
        .collect()
}

/// 按当前选用人物返回可切换皮肤（与收藏无关）
pub fn list_pet_menu_skins(
    data_dir: &Path,
    db: &rusqlite::Connection,
) -> Result<PetMenuSkinsPayload, String> {
    let model_id = models::active_model_id(db);
    let character_id = active_character_id(db, data_dir);
    build_pet_menu_skins_payload(data_dir, &character_id, &model_id)
}

/// 按指定人物返回可切换皮肤（桌宠菜单从收藏人物进入皮肤页）
pub fn list_pet_menu_skins_for_character(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: &str,
) -> Result<PetMenuSkinsPayload, String> {
    let current_char = active_character_id(db, data_dir);
    let active_model_id = if current_char == character_id {
        models::active_model_id(db)
    } else {
        String::new()
    };
    build_pet_menu_skins_payload(data_dir, character_id, &active_model_id)
}

const FAVORITES_SETTING_KEY: &str = "character_favorites";

/// 读取收藏人物 id 列表（顺序保留）
pub fn favorite_character_ids(db: &rusqlite::Connection) -> Vec<String> {
    let Some(raw) = crate::db::get_setting(db, FAVORITES_SETTING_KEY) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) else {
        return Vec::new();
    };
    parsed
        .into_iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// 桌宠菜单：仅收藏人物（无收藏时返回空列表）
pub fn list_pet_menu_favorite_characters(
    data_dir: &Path,
    db: &rusqlite::Connection,
) -> Vec<CharacterBrief> {
    let fav_ids = favorite_character_ids(db);
    if fav_ids.is_empty() {
        return Vec::new();
    }
    let active_id = active_character_id(db, data_dir);
    let all = resolve_all_characters(data_dir);
    let mut out = Vec::new();
    for fav_id in &fav_ids {
        let Some(meta) = all.iter().find(|c| &c.id == fav_id) else {
            continue;
        };
        out.push(meta_to_brief(data_dir, meta, &active_id, db, true, None));
    }
    out
}

pub(crate) fn build_pet_menu_skins_payload(
    data_dir: &Path,
    character_id: &str,
    active_model_id: &str,
) -> Result<PetMenuSkinsPayload, String> {
    let meta = find_character_meta(data_dir, character_id)?;
    let model_ids_set = resolved_roster_model_ids(data_dir);
    let normalized =
        normalize_character_skins(data_dir, &meta.skins, Some(model_ids_set.as_ref()));
    let model_ids: Vec<String> = normalized.iter().map(|s| s.model_id.clone()).collect();
    let model_cache = models::model_names_for_ids(data_dir, &model_ids);
    let skins: Vec<CharacterSkinInfo> = normalized
        .iter()
        .map(|s| {
            let mut info = build_skin_info(
                data_dir,
                s,
                "",
                Some(&model_cache),
                Some(model_ids_set.as_ref()),
            );
            info.active = models::canonical_model_id(&s.model_id)
                == models::canonical_model_id(active_model_id);
            info
        })
        .collect();
    Ok(PetMenuSkinsPayload {
        character_id: meta.id.clone(),
        character_name: meta.name.clone(),
        model_id: active_model_id.to_string(),
        skins,
    })
}

pub fn list_characters_brief(data_dir: &Path, db: &rusqlite::Connection) -> Vec<CharacterBrief> {
    let all = resolve_all_characters(data_dir);
    let active_id = active_character_id(db, data_dir);
    all.iter()
        .map(|c| meta_to_brief(data_dir, c, &active_id, db, true, None))
        .collect()
}

pub fn list_characters_page(
    data_dir: &Path,
    db: &rusqlite::Connection,
    offset: usize,
    limit: usize,
    query: Option<&str>,
    favorites_only: bool,
    favorite_ids: &[String],
) -> CharacterListPage {
    let all = resolved_roster_arc(data_dir);
    let active_id = active_character_id(db, data_dir);
    let indices = roster_filter_indices(&all, query, favorites_only, favorite_ids);
    let total = indices.len();
    let limit = limit.clamp(1, 200);
    let avatar_index = avatar::cached_avatar_path_index(data_dir);
    let items = indices
        .iter()
        .skip(offset)
        .take(limit)
        .map(|&i| meta_to_brief(data_dir, &all[i], &active_id, db, false, Some(&avatar_index)))
        .collect();
    CharacterListPage {
        total,
        offset,
        limit,
        items,
    }
}

pub fn get_character_detail(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: &str,
) -> Result<CharacterDetail, String> {
    let meta = find_character_meta(data_dir, character_id)?;
    let active_id = active_character_id(db, data_dir);
    let skin_id = resolve_display_skin_id(&meta, db, &active_id);
    let normalized = normalize_character_skins(data_dir, &meta.skins, None);
    let active_skin = normalized
        .iter()
        .find(|s| s.id == skin_id)
        .or_else(|| default_skin(&meta));
    let (active_skin_name, active_model_id, active_model_name) =
        if let Some(s) = active_skin {
            let names = models::model_names_for_ids(data_dir, std::slice::from_ref(&s.model_id));
            (
                s.name.clone(),
                s.model_id.clone(),
                names
                    .get(&s.model_id)
                    .cloned()
                    .unwrap_or_else(|| s.model_id.clone()),
            )
        } else {
            (
                "默认".into(),
                String::new(),
                String::new(),
            )
        };
    let persona_detail = persona::get_persona_detail(data_dir, db, &meta.persona_id)?;
    let has_profile = !persona_detail.profile_json.name.trim().is_empty()
        || !persona_detail.profile_json.introduction.is_empty();
    let (faction, ship_type, rarity) = (
        meta.faction.clone(),
        meta.ship_type.clone(),
        meta.rarity.clone(),
    );
    let active_model_ready = model_is_ready(data_dir, &active_model_id, None);
    Ok(CharacterDetail {
        id: meta.id.clone(),
        name: meta.name.clone(),
        source: meta.source.clone(),
        description: meta.description.clone(),
        persona_id: meta.persona_id.clone(),
        active: meta.id == active_id,
        active_skin_id: skin_id,
        active_skin_name,
        active_model_id,
        active_model_name,
        active_model_ready,
        skin_count: meta.skins.len(),
        is_builtin: is_builtin_character(&meta.id),
        faction,
        ship_type,
        rarity,
        trait_summary: trait_summary_for(data_dir, &meta.persona_id),
        avatar_path: avatar::avatar_path_string(data_dir, &meta.id),
        avatar_url: avatar::avatar_url_for_name(&meta.name),
        skill_md: persona_detail.skill_md,
        profile_json: persona_detail.profile_json,
        has_profile,
        profile_ai_updated: persona_detail.profile_ai_updated,
        profile_ai_updated_at: persona_detail.profile_ai_updated_at,
    })
}

pub fn list_character_skins_page(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: &str,
    offset: usize,
    limit: usize,
) -> Result<CharacterSkinsPage, String> {
    let meta = find_character_meta(data_dir, character_id)?;
    let active_id = active_character_id(db, data_dir);
    let normalized = normalize_character_skins(data_dir, &meta.skins, None);
    let active_skin_id = resolve_display_skin_id(&meta, db, &active_id);
    let total = normalized.len();
    let limit = limit.clamp(1, 100);
    let slice: Vec<_> = normalized.iter().skip(offset).take(limit).collect();
    let model_ids: Vec<String> = slice.iter().map(|s| s.model_id.clone()).collect();
    let model_cache = models::model_names_for_ids(data_dir, &model_ids);
    let items = slice
        .iter()
        .map(|s| build_skin_info(data_dir, s, &active_skin_id, Some(&model_cache), None))
        .collect();
    Ok(CharacterSkinsPage {
        total,
        offset,
        limit,
        active_skin_id,
        items,
    })
}

pub fn set_active_character(
    data_dir: &Path,
    db: &rusqlite::Connection,
    persona_manifest: &PersonaManifest,
    character_id: &str,
) -> Result<(), String> {
    let meta = find_character_meta(data_dir, character_id)?;
    let normalized = normalize_character_skins(data_dir, &meta.skins, None);
    let pref_id = resolve_preferred_skin_id(&meta);
    let skin = normalized
        .iter()
        .find(|s| s.id == pref_id)
        .or_else(|| normalized.iter().find(|s| s.default))
        .or_else(|| normalized.first())
        .ok_or_else(|| format!("人物 {character_id} 无可用皮肤"))?;
    if !model_is_ready(data_dir, &skin.model_id, None) {
        return Err("该人物皮肤模型尚未完成，请先导入模型".into());
    }
    set_active_character_id(db, character_id)?;
    set_active_skin_id(db, &skin.id)?;
    persona::set_active_persona_id(db, persona_manifest, &meta.persona_id)?;
    models::set_active_model_id(db, &skin.model_id)?;
    Ok(())
}

/// 选择人物皮肤：写入偏好，并切换为当前选用人物与桌宠模型。
pub fn select_character_skin(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: &str,
    skin_id: &str,
) -> Result<String, String> {
    let meta = find_character_meta(data_dir, character_id)?;
    let normalized = normalize_character_skins(data_dir, &meta.skins, None);
    let skin = normalized
        .iter()
        .find(|s| s.id == skin_id)
        .ok_or_else(|| format!("未知皮肤: {skin_id}"))?;
    if !model_is_ready(data_dir, &skin.model_id, None) {
        return Err("该皮肤模型尚未完成，请先导入模型".into());
    }
    set_preferred_skin_in_manifest(data_dir, character_id, skin_id)?;

    let manifest = persona::load_manifest(data_dir);
    set_active_character_id(db, character_id)?;
    set_active_skin_id(db, skin_id)?;
    persona::set_active_persona_id(db, &manifest, &meta.persona_id)?;
    models::set_active_model_id(db, &skin.model_id)?;
    Ok(skin.model_id.clone())
}

pub fn set_active_skin(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: &str,
    skin_id: &str,
) -> Result<(), String> {
    select_character_skin(data_dir, db, character_id, skin_id).map(|_| ())
}

pub fn sync_from_persona(data_dir: &Path, db: &rusqlite::Connection, persona_id: &str) {
    let all = resolve_all_characters(data_dir);
    let Some(meta) = find_by_persona(&all, persona_id) else {
        return;
    };
    let _ = set_active_character_id(db, &meta.id);
    let current_char = crate::db::get_setting(db, ACTIVE_CHARACTER_KEY).unwrap_or_default();
    let skin_id = if current_char == meta.id {
        resolve_active_skin(meta, db)
    } else if let Some(s) = default_skin(meta) {
        s.id.clone()
    } else {
        return;
    };
    if let Some(s) = meta.skins.iter().find(|s| s.id == skin_id) {
        let _ = set_active_skin_id(db, &skin_id);
        let _ = models::set_active_model_id(db, &s.model_id);
    }
}

pub fn sync_from_model(data_dir: &Path, db: &rusqlite::Connection, model_id: &str) {
    let all = resolve_all_characters(data_dir);
    let Some((meta, skin)) = find_by_model(&all, model_id) else {
        return;
    };
    let persona_manifest = persona::load_manifest(data_dir);
    let _ = set_active_character_id(db, &meta.id);
    let _ = set_active_skin_id(db, &skin.id);
    let _ = persona::set_active_persona_id(db, &persona_manifest, &meta.persona_id);
}

/// 删除人设时级联：移除 characters/manifest 条目、未引用的用户模型
pub fn purge_character_for_persona(
    data_dir: &Path,
    db: &rusqlite::Connection,
    persona_id: &str,
) -> Result<(), String> {
    let pre = load_manifest(data_dir);
    let Some(character) = pre
        .characters
        .iter()
        .find(|c| c.persona_id == persona_id)
        .cloned()
    else {
        return Ok(());
    };
    if is_builtin_character(&character.id) {
        return Ok(());
    }

    let was_active = active_character_id(db, data_dir) == character.id;
    let model_ids: Vec<String> = character.skins.iter().map(|s| s.model_id.clone()).collect();

    mutate_character_manifest(data_dir, |manifest| {
        manifest.characters.retain(|c| c.persona_id != persona_id);
        Ok(())
    })?;

    let manifest = load_manifest(data_dir);
    for model_id in model_ids {
        if !models::is_builtin_model(&model_id) && !model_id_used_in_manifest(&manifest, &model_id) {
            let _ = models::delete_model(data_dir, db, &model_id);
        }
    }

    if was_active {
        let fallback_id = manifest.default_id.clone();
        set_active_character_id(db, &fallback_id)?;
        if let Some(fallback) = manifest.characters.iter().find(|c| c.id == fallback_id) {
            if let Some(skin) = default_skin(fallback) {
                set_active_skin_id(db, &skin.id)?;
                models::set_active_model_id(db, &skin.model_id)?;
            }
        }
    }
    Ok(())
}

/// 移除人物皮肤；可选删除磁盘模型（若无其它皮肤引用）
pub fn remove_character_skin(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: &str,
    skin_id: &str,
    delete_model_files: bool,
) -> Result<Option<String>, String> {
    let was_active_char = active_character_id(db, data_dir) == character_id;
    let was_active_skin = active_skin_id(db).as_deref() == Some(skin_id);

    struct SkinRemoval {
        removed: CharacterSkinMeta,
        fallback_skin: CharacterSkinMeta,
    }

    let removal = mutate_character_manifest(data_dir, |manifest| {
        let character = manifest
            .characters
            .iter_mut()
            .find(|c| c.id == character_id)
            .ok_or_else(|| format!("未知人物: {character_id}"))?;

        if character.skins.len() <= 1 {
            return Err("至少保留一个皮肤".into());
        }
        let skin_idx = character
            .skins
            .iter()
            .position(|s| s.id == skin_id)
            .ok_or_else(|| format!("未知皮肤: {skin_id}"))?;
        let removed = character.skins[skin_idx].clone();
        if models::is_builtin_model(&removed.model_id) {
            return Err("内置模型皮肤不可删除".into());
        }

        let removed_default = removed.default;
        character.skins.remove(skin_idx);
        if removed_default {
            if let Some(first) = character.skins.first_mut() {
                first.default = true;
            }
        }

        let fallback_skin = default_skin(character)
            .cloned()
            .ok_or_else(|| "皮肤列表为空".to_string())?;
        Ok(SkinRemoval {
            removed,
            fallback_skin,
        })
    })?;

    if was_active_char && was_active_skin {
        set_active_skin_id(db, &removal.fallback_skin.id)?;
        models::set_active_model_id(db, &removal.fallback_skin.model_id)?;
    }

    if delete_model_files && !models::is_builtin_model(&removal.removed.model_id) {
        let manifest = load_manifest(data_dir);
        if !model_id_used_in_manifest(&manifest, &removal.removed.model_id) {
            models::delete_model(data_dir, db, &removal.removed.model_id)?;
        }
    }

    if was_active_char && was_active_skin {
        Ok(Some(removal.fallback_skin.model_id))
    } else {
        Ok(None)
    }
}

/// 用户导入模型后，为当前人物追加皮肤（若 model 尚未绑定）
pub fn attach_imported_model_as_skin(
    data_dir: &Path,
    db: &rusqlite::Connection,
    model: &PetModelInfo,
) -> Result<(), String> {
    let active_id = active_character_id(db, data_dir);
    attach_model_to_character(data_dir, db, &active_id, model, &model.name, true)
}

/// 在已加载的 manifest 上追加皮肤（不写盘）
pub fn attach_model_in_manifest(
    data_dir: &Path,
    manifest: &mut CharacterManifest,
    character_id: &str,
    model: &PetModelInfo,
    skin_name: &str,
) -> Result<(), String> {
    if !manifest.characters.iter().any(|c| c.id == character_id) {
        let all = resolve_all_characters(data_dir);
        if let Some(meta) = find_meta(&all, character_id) {
            manifest.characters.push(meta.clone());
        }
    }

    let character = manifest
        .characters
        .iter_mut()
        .find(|c| c.id == character_id)
        .ok_or_else(|| format!("人物 {character_id} 不在 manifest 中"))?;

    if character.skins.iter().any(|s| s.model_id == model.id) {
        return Ok(());
    }

    let skin_id = format!("skin-{}", model.id);
    let name = skin_name.trim();
    let skin_display = if name.is_empty() {
        model.name.clone()
    } else {
        name.to_string()
    };
    character.skins.push(CharacterSkinMeta {
        id: skin_id.clone(),
        name: skin_display,
        model_id: model.id.clone(),
        default: false,
    });
    character.preferred_skin_id = Some(skin_id);
    Ok(())
}

/// 为指定人物追加皮肤（批量 Live2D 导入等场景）
pub fn attach_model_to_character(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: &str,
    model: &PetModelInfo,
    skin_name: &str,
    set_active: bool,
) -> Result<(), String> {
    mutate_character_manifest(data_dir, |manifest| {
        attach_model_in_manifest(data_dir, manifest, character_id, model, skin_name)?;
        let _ = repair_character_manifest_skins(data_dir, manifest);
        Ok(())
    })?;

    if set_active {
        let skin_id = format!("skin-{}", model.id);
        set_active_character_id(db, character_id)?;
        set_active_skin_id(db, &skin_id)?;
        models::set_active_model_id(db, &model.id)?;
    }
    Ok(())
}

pub fn migrate_on_startup(data_dir: &Path, db: &rusqlite::Connection) -> Result<(), String> {
    let embedded = embedded_manifest();
    mutate_character_manifest(data_dir, |manifest| {
        let mut changed = false;
        for builtin in &embedded.characters {
            if !manifest.characters.iter().any(|c| c.id == builtin.id) {
                manifest.characters.push(builtin.clone());
                changed = true;
            }
        }
        if !manifest.characters.iter().any(|c| c.id == manifest.default_id) {
            manifest.default_id = embedded.default_id.clone();
            changed = true;
        }
        Ok(changed)
    })?;

    let _ = sync_character_manifest_from_personas(data_dir)?;

    if crate::db::get_setting(db, ACTIVE_CHARACTER_KEY)
        .filter(|s| !s.trim().is_empty())
        .is_some()
    {
        return Ok(());
    }

    let persona_manifest = persona::load_manifest(data_dir);
    let persona_id = persona::active_persona_id(db, &persona_manifest);
    let model_id = models::active_model_id(db);
    let all = resolve_all_characters(data_dir);

    if let Some((meta, skin)) = find_by_model(&all, &model_id) {
        set_active_character_id(db, &meta.id)?;
        set_active_skin_id(db, &skin.id)?;
        return Ok(());
    }

    if let Some(meta) = find_by_persona(&all, &persona_id) {
        set_active_character(data_dir, db, &persona_manifest, &meta.id)?;
        return Ok(());
    }

    set_active_character(
        data_dir,
        db,
        &persona_manifest,
        &load_manifest(data_dir).default_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn embedded_manifest_parses() {
        let m = embedded_manifest();
        assert_eq!(m.default_id, "cheshire");
        assert!(m.characters.iter().any(|c| c.id == "cheshire"));
        let cheshire = m.characters.iter().find(|c| c.id == "cheshire").unwrap();
        assert_eq!(cheshire.skins[0].model_id, "chaijun");
    }

    #[test]
    fn resolve_display_skin_id_falls_back_after_normalize() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        seed_user_characters(base).unwrap();
        let db = db::open_and_migrate(&base.join("test.db")).unwrap();
        let mut manifest = load_manifest(base);
        let character = manifest
            .characters
            .iter_mut()
            .find(|c| c.id == "cheshire")
            .unwrap();
        character.preferred_skin_id = Some("default".into());
        character.skins = vec![
            CharacterSkinMeta {
                id: "default".into(),
                name: "默认·待导入".into(),
                model_id: "missing-model".into(),
                default: true,
            },
            CharacterSkinMeta {
                id: "ready-skin".into(),
                name: "已导入".into(),
                model_id: "chaijun".into(),
                default: false,
            },
        ];
        save_manifest(base, &manifest).unwrap();
        let mut meta = find_character_meta(base, "cheshire").unwrap();
        meta.skins = normalize_character_skins(base, &meta.skins, None);
        let skin_id = resolve_display_skin_id(&meta, &db, "other-character");
        assert_eq!(skin_id, "ready-skin");
    }

    #[test]
    fn migrate_from_persona_and_model() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        persona::seed_user_personas(base).unwrap();
        seed_user_characters(base).unwrap();
        let db = db::open_and_migrate(&base.join("test.db")).unwrap();
        persona::set_active_persona_id(
            &db,
            &persona::load_manifest(base),
            "cheshire",
        )
        .unwrap();
        models::set_active_model_id(&db, "chaijun").unwrap();
        migrate_on_startup(base, &db).unwrap();
        assert_eq!(
            crate::db::get_setting(&db, ACTIVE_CHARACTER_KEY).as_deref(),
            Some("cheshire")
        );
    }

    #[test]
    fn model_wiki_title_lookup_builtin_model() {
        let dir = tempfile::tempdir().unwrap();
        let lookup = ModelWikiTitleLookup::build(dir.path());
        assert_eq!(lookup.wiki_title("chaijun"), Some("柴郡".to_string()));
        assert_eq!(lookup.display_name("chaijun"), "柴郡");
    }

    #[test]
    fn strip_skin_suffix_from_display_name_variants() {
        assert_eq!(
            strip_skin_suffix_from_display_name("柴郡·皮肤2"),
            "柴郡"
        );
        assert_eq!(
            strip_skin_suffix_from_display_name("阿尔弗雷多·奥里亚尼·皮肤2"),
            "阿尔弗雷多·奥里亚尼"
        );
        assert_eq!(strip_skin_suffix_from_display_name("爱宕·便服"), "爱宕");
        assert_eq!(strip_skin_suffix_from_display_name("柴郡"), "柴郡");
    }

    #[test]
    fn model_wiki_title_lookup_skin_display_name() {
        let dir = tempfile::tempdir().unwrap();
        let model_dir = dir.path().join("pet-models").join("skin-test");
        fs::create_dir_all(&model_dir).unwrap();
        fs::write(model_dir.join("display_name.txt"), "柴郡·皮肤2").unwrap();
        let lookup = ModelWikiTitleLookup::build(dir.path());
        assert_eq!(lookup.wiki_title("skin-test"), Some("柴郡".to_string()));
    }

    #[test]
    fn model_wiki_title_lookup_persisted_wiki_title() {
        let dir = tempfile::tempdir().unwrap();
        let model_dir = dir.path().join("pet-models").join("skin-test");
        fs::create_dir_all(&model_dir).unwrap();
        fs::write(model_dir.join("display_name.txt"), "柴郡·皮肤2").unwrap();
        fs::write(model_dir.join("wiki_title.txt"), "柴郡").unwrap();
        let lookup = ModelWikiTitleLookup::build(dir.path());
        assert_eq!(lookup.wiki_title("skin-test"), Some("柴郡".to_string()));
    }

    #[test]
    fn model_wiki_title_avoids_numeric_id_collision() {
        let dir = tempfile::tempdir().unwrap();
        let model_dir = dir.path().join("pet-models").join("2-125");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("display_name.txt"), "胡德·皮肤2").unwrap();
        let base_dir = dir.path().join("pet-models").join("2");
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::write(base_dir.join("display_name.txt"), "阿贝克隆比·皮肤2").unwrap();
        let lookup = ModelWikiTitleLookup::build(dir.path());
        assert_eq!(lookup.wiki_title("2-125"), Some("胡德".to_string()));
    }

    #[test]
    fn model_wiki_title_lookup_complex_character_skin() {
        let dir = tempfile::tempdir().unwrap();
        let model_dir = dir.path().join("pet-models").join("alfredo-skin");
        fs::create_dir_all(&model_dir).unwrap();
        fs::write(
            model_dir.join("display_name.txt"),
            "阿尔弗雷多·奥里亚尼·皮肤2",
        )
        .unwrap();
        let lookup = ModelWikiTitleLookup::build(dir.path());
        assert_eq!(
            lookup.wiki_title("alfredo-skin"),
            Some("阿尔弗雷多·奥里亚尼".to_string())
        );
    }
}
