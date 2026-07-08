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

const EMBEDDED_MANIFEST: &str = include_str!("../../../characters/manifest.json");

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
    data_dir.join("characters")
}

pub fn manifest_path(data_dir: &Path) -> PathBuf {
    characters_dir(data_dir).join("manifest.json")
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

/// 与皮肤分页列表一致：在 normalize 后解析应展示/高亮的皮肤 id
fn resolve_display_skin_id(
    data_dir: &Path,
    meta: &CharacterMeta,
    db: &rusqlite::Connection,
    global_active_id: &str,
) -> String {
    let normalized = normalize_character_skins(data_dir, &meta.skins);
    let mut skin_id = resolve_character_skin_id(meta, db, global_active_id);
    if !normalized.iter().any(|s| s.id == skin_id) {
        skin_id = normalized
            .iter()
            .find(|s| s.default)
            .or_else(|| normalized.first())
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

fn model_is_ready(data_dir: &Path, model_id: &str) -> bool {
    if model_id.is_empty() {
        return false;
    }
    if models::is_builtin_model(model_id) {
        return true;
    }
    models::resolve_assets(data_dir, model_id).is_ok()
}

/// 去重并移除「已有可用模型时仍显示的默认占位皮肤」
fn normalize_character_skins(data_dir: &Path, skins: &[CharacterSkinMeta]) -> Vec<CharacterSkinMeta> {
    let mut out: Vec<CharacterSkinMeta> = Vec::new();
    let mut seen_model_ids: HashSet<String> = HashSet::new();

    for skin in skins {
        if !seen_model_ids.insert(skin.model_id.clone()) {
            continue;
        }
        out.push(skin.clone());
    }

    let has_ready = out.iter().any(|s| model_is_ready(data_dir, &s.model_id));
    if has_ready {
        out.retain(|s| model_is_ready(data_dir, &s.model_id));
    }

    if out.is_empty() {
        return skins.to_vec();
    }
    out
}

pub fn repair_character_manifest_skins(data_dir: &Path, manifest: &mut CharacterManifest) -> bool {
    let mut changed = false;
    for c in &mut manifest.characters {
        let normalized = normalize_character_skins(data_dir, &c.skins);
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

        if c.skins.iter().any(|s| s.default && model_is_ready(data_dir, &s.model_id)) {
            continue;
        }
        if let Some(ready_idx) = c
            .skins
            .iter()
            .position(|s| model_is_ready(data_dir, &s.model_id))
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
) -> CharacterSkinInfo {
    CharacterSkinInfo {
        id: s.id.clone(),
        name: s.name.clone(),
        model_id: s.model_id.clone(),
        model_name: if let Some(cache) = model_cache {
            model_name_cached(&s.model_id, cache)
        } else {
            model_name(data_dir, &s.model_id)
        },
        active: s.id == active_skin_id,
        model_ready: model_is_ready(data_dir, &s.model_id),
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
    meta.skins
        .iter()
        .map(|s| build_skin_info(data_dir, s, active_skin_id, model_cache))
        .collect()
}

/// 合并 manifest 与仅存在于 persona 的条目（单皮肤默认模型）
/// 已有人物条目会从 persona manifest 同步 name/source/description
/// 合并 manifest 与 persona（单次读盘）
fn load_resolved_roster(data_dir: &Path, _model_ids: &HashSet<String>) -> CharacterManifest {
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
    }

    for p in &persona_manifest.personas {
        if manifest.characters.iter().any(|c| c.persona_id == p.id) {
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
    }
    manifest
}

struct ResolvedRosterCache {
    fingerprint: u64,
    characters: Arc<Vec<CharacterMeta>>,
    model_ids: Arc<HashSet<String>>,
}

static RESOLVED_ROSTER: OnceLock<Mutex<ResolvedRosterCache>> = OnceLock::new();

fn roster_fingerprint(data_dir: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    for path in [
        manifest_path(data_dir),
        persona::manifest_path(data_dir),
    ] {
        if let Ok(m) = fs::metadata(&path) {
            m.len().hash(&mut h);
            if let Ok(t) = m.modified() {
                t.hash(&mut h);
            }
        }
    }
    let models_root = models::models_dir(data_dir);
    if models_root.is_dir() {
        if let Ok(entries) = fs::read_dir(&models_root) {
            let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
            dirs.sort_by_key(|e| e.file_name());
            for entry in dirs {
                entry.file_name().hash(&mut h);
                if let Ok(m) = entry.metadata() {
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
    let fp = roster_fingerprint(data_dir);
    let lock = RESOLVED_ROSTER.get_or_init(|| {
        Mutex::new(ResolvedRosterCache {
            fingerprint: 0,
            characters: Arc::new(Vec::new()),
            model_ids: Arc::new(HashSet::new()),
        })
    });
    let mut cache = lock.lock().unwrap_or_else(|e| e.into_inner());
    if cache.fingerprint != fp || cache.characters.is_empty() {
        let model_ids = models::list_model_id_set(data_dir).unwrap_or_default();
        cache.fingerprint = fp;
        cache.model_ids = Arc::new(model_ids);
        cache.characters = Arc::new(
            load_resolved_roster(data_dir, cache.model_ids.as_ref())
                .characters,
        );
    }
    Arc::clone(&cache.characters)
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

fn resolve_all_characters(data_dir: &Path) -> Vec<CharacterMeta> {
    resolved_roster_arc(data_dir).as_ref().clone()
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
    let normalized = normalize_character_skins(data_dir, &c.skins);
    let skin_id = resolve_display_skin_id(data_dir, c, db, active_id);
    let skin_meta = normalized
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
        skin_count: normalized.len(),
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
    let mut indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            if favorites_only {
                let set: HashSet<&str> = favorite_ids.iter().map(|s| s.as_str()).collect();
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
            let skin = resolve_display_skin_id(data_dir, c, db, &active_id);
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
    let limit = limit.max(1).min(200);
    let avatar_index = avatar::build_avatar_path_index(data_dir);
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
    let skin_id = resolve_display_skin_id(data_dir, &meta, db, &active_id);
    let normalized = normalize_character_skins(data_dir, &meta.skins);
    let active_skin = normalized
        .iter()
        .find(|s| s.id == skin_id)
        .or_else(|| default_skin(&meta));
    let (active_skin_name, active_model_id, active_model_name) =
        if let Some(s) = active_skin {
            let names = models::model_names_for_ids(data_dir, &[s.model_id.clone()]);
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
    let has_profile = persona_detail.profile_json.name.trim().len() > 0
        || !persona_detail.profile_json.introduction.is_empty();
    let (faction, ship_type, rarity) = (
        meta.faction.clone(),
        meta.ship_type.clone(),
        meta.rarity.clone(),
    );
    let active_model_ready = model_is_ready(data_dir, &active_model_id);
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
    let normalized = normalize_character_skins(data_dir, &meta.skins);
    let active_skin_id = resolve_display_skin_id(data_dir, &meta, db, &active_id);
    let total = normalized.len();
    let limit = limit.max(1).min(100);
    let slice: Vec<_> = normalized.iter().skip(offset).take(limit).collect();
    let model_ids: Vec<String> = slice.iter().map(|s| s.model_id.clone()).collect();
    let model_cache = models::model_names_for_ids(data_dir, &model_ids);
    let items = slice
        .iter()
        .map(|s| build_skin_info(data_dir, s, &active_skin_id, Some(&model_cache)))
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
    let normalized = normalize_character_skins(data_dir, &meta.skins);
    let pref_id = resolve_preferred_skin_id(&meta);
    let skin = normalized
        .iter()
        .find(|s| s.id == pref_id)
        .or_else(|| normalized.iter().find(|s| s.default))
        .or_else(|| normalized.first())
        .ok_or_else(|| format!("人物 {character_id} 无可用皮肤"))?;
    if !model_is_ready(data_dir, &skin.model_id) {
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
    let skin = meta
        .skins
        .iter()
        .find(|s| s.id == skin_id)
        .ok_or_else(|| format!("未知皮肤: {skin_id}"))?;
    if !model_is_ready(data_dir, &skin.model_id) {
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
    seed_user_characters(data_dir).map_err(|e| e.to_string())?;

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
        let meta = find_character_meta(base, "cheshire").unwrap();
        let skin_id = resolve_display_skin_id(base, &meta, &db, "other-character");
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
}
