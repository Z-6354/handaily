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
    /// Wiki / 游戏导出对照的英文名
    #[serde(default)]
    pub english_name: String,
    /// BWIKI 标题
    #[serde(default)]
    pub wiki_title: String,
    /// 配音演员
    #[serde(default)]
    pub cv: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterSkinLine {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wiki_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_relpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSkinMeta {
    pub id: String,
    pub name: String,
    /// Spine pet-models id；仅舰娘时可为空串
    pub model_id: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin_index: Option<i32>,
    /// Cubism 目录名（如 aidang_2）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kanmusu_dir: Option<String>,
    /// 皮肤英文名
    #[serde(default)]
    pub english_name: String,
    #[serde(default)]
    pub lines: Vec<CharacterSkinLine>,
}

impl CharacterSkinMeta {
    pub fn spine(id: impl Into<String>, name: impl Into<String>, model_id: impl Into<String>, is_default: bool) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            model_id: model_id.into(),
            default: is_default,
            skin_index: None,
            kanmusu_dir: None,
            english_name: String::new(),
            lines: Vec::new(),
        }
    }
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
    #[serde(default)]
    pub skin_index: Option<i32>,
    #[serde(default)]
    pub kanmusu_dir: Option<String>,
    #[serde(default)]
    pub kanmusu_ready: bool,
    #[serde(default)]
    pub lines_count: usize,
    #[serde(default)]
    pub english_name: String,
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
    #[serde(default)]
    pub english_name: String,
    #[serde(default)]
    pub wiki_title: String,
    #[serde(default)]
    pub cv: String,
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
    #[serde(default)]
    pub english_name: String,
    #[serde(default)]
    pub wiki_title: String,
    #[serde(default)]
    pub cv: String,
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

/// BWIKI 导入产生的 `p` + 8 位 hex 人设/角色 id
fn is_hash_persona_id(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == 9
        && (b[0] == b'p' || b[0] == b'P')
        && b[1..].iter().all(|c| c.is_ascii_hexdigit())
}

fn merge_skin_meta(dst: &mut CharacterSkinMeta, src: &CharacterSkinMeta) {
    if dst.model_id.trim().is_empty() && !src.model_id.trim().is_empty() {
        dst.model_id = src.model_id.clone();
    }
    let dst_km_empty = dst
        .kanmusu_dir
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if dst_km_empty {
        if let Some(k) = src
            .kanmusu_dir
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            dst.kanmusu_dir = Some(k.clone());
        }
    }
    if dst.skin_index.is_none() {
        dst.skin_index = src.skin_index;
    }
    if dst.english_name.trim().is_empty() && !src.english_name.trim().is_empty() {
        dst.english_name = src.english_name.clone();
    }
    if dst.lines.is_empty() && !src.lines.is_empty() {
        dst.lines = src.lines.clone();
    }
    if !dst.default && src.default {
        dst.default = true;
    }
}

fn merge_skins_from_donor(canon: &mut CharacterMeta, donor: &CharacterMeta) {
    let mut by_id: HashMap<String, CharacterSkinMeta> = HashMap::new();
    for s in &canon.skins {
        if !s.id.is_empty() {
            by_id.insert(s.id.clone(), s.clone());
        }
    }
    for s in &donor.skins {
        if s.id.is_empty() {
            continue;
        }
        if let Some(cur) = by_id.get_mut(&s.id) {
            merge_skin_meta(cur, s);
            continue;
        }
        if let Some(idx) = s.skin_index {
            if let Some(cur) = by_id
                .values_mut()
                .find(|e| e.skin_index == Some(idx))
            {
                merge_skin_meta(cur, s);
                continue;
            }
        }
        by_id.insert(s.id.clone(), s.clone());
    }
    canon.skins = by_id.into_values().collect();
    if canon.english_name.trim().is_empty() && !donor.english_name.trim().is_empty() {
        canon.english_name = donor.english_name.clone();
    }
    if canon.wiki_title.trim().is_empty() && !donor.wiki_title.trim().is_empty() {
        canon.wiki_title = donor.wiki_title.clone();
    }
    if canon.description.trim().is_empty() && !donor.description.trim().is_empty() {
        canon.description = donor.description.clone();
    }
    if canon.source.trim().is_empty() && !donor.source.trim().is_empty() {
        canon.source = donor.source.clone();
    }
    if canon.faction.trim().is_empty() && !donor.faction.trim().is_empty() {
        canon.faction = donor.faction.clone();
    }
    if canon.ship_type.trim().is_empty() && !donor.ship_type.trim().is_empty() {
        canon.ship_type = donor.ship_type.clone();
    }
    if canon.rarity.trim().is_empty() && !donor.rarity.trim().is_empty() {
        canon.rarity = donor.rarity.clone();
    }
    if canon.cv.trim().is_empty() && !donor.cv.trim().is_empty() {
        canon.cv = donor.cv.clone();
    }
    // 规范角色 id 与 persona_id 对齐为拼音条目
    canon.persona_id = canon.id.clone();
}

/// 同中文名：hash 角色并入拼音角色；返回 old_id → canonical_id
fn dedupe_characters_by_name(
    characters: &mut Vec<CharacterMeta>,
) -> (bool, HashMap<String, String>) {
    let mut remap = HashMap::new();
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    let mut orphans: Vec<usize> = Vec::new();
    for (i, c) in characters.iter().enumerate() {
        let name = c.name.trim().to_string();
        if name.is_empty() {
            orphans.push(i);
            continue;
        }
        by_name.entry(name).or_default().push(i);
    }

    let mut keep = vec![false; characters.len()];
    for i in &orphans {
        keep[*i] = true;
    }

    let mut changed = false;
    for indices in by_name.values() {
        if indices.len() == 1 {
            keep[indices[0]] = true;
            continue;
        }
        let non_hash: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| !is_hash_persona_id(&characters[i].id))
            .collect();
        let canon_idx = if !non_hash.is_empty() {
            // 优先已有 kanmusu 皮肤的拼音条目
            non_hash
                .iter()
                .copied()
                .find(|&i| {
                    characters[i].skins.iter().any(|s| {
                        s.kanmusu_dir
                            .as_ref()
                            .map(|d| !d.trim().is_empty())
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(non_hash[0])
        } else {
            indices[0]
        };
        let mut canon = characters[canon_idx].clone();
        for &i in indices {
            if i == canon_idx {
                continue;
            }
            let donor = &characters[i];
            if donor.id != canon.id {
                remap.insert(donor.id.clone(), canon.id.clone());
                merge_skins_from_donor(&mut canon, donor);
                changed = true;
            }
        }
        let (skins_changed, _) = coalesce_character_skins(&mut canon);
        changed |= skins_changed;
        keep[canon_idx] = true;
        // 用合并后的 canon 替换原位（稍后重建列表）
        characters[canon_idx] = canon;
    }

    if !changed && remap.is_empty() {
        // 仍可能有同名双条目但 id 相同的异常；仅在有 donor 被丢弃时重建
        let drop_any = keep.iter().any(|k| !*k);
        if !drop_any {
            return (false, remap);
        }
        changed = true;
    }

    let mut out = Vec::with_capacity(characters.len());
    for (i, c) in characters.drain(..).enumerate() {
        if keep[i] {
            out.push(c);
        } else {
            changed = true;
        }
    }
    *characters = out;
    (changed, remap)
}

/// Wiki 数字皮肤包目录（如 `2-14`、`5`）不是角色皮肤序号
fn is_numeric_pack_id(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    let mut parts = s.split('-');
    let Some(first) = parts.next() else {
        return false;
    };
    if first.is_empty() || !first.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match parts.next() {
        None => true,
        Some(second) => {
            !second.is_empty()
                && second.chars().all(|c| c.is_ascii_digit())
                && parts.next().is_none()
        }
    }
}

fn trailing_skin_index(s: &str) -> Option<i32> {
    let s = s.trim();
    // slug_N 或 slug-N
    let (base, idx_str) = if let Some((b, i)) = s.rsplit_once('_') {
        (b, i)
    } else if let Some((b, i)) = s.rsplit_once('-') {
        (b, i)
    } else {
        return None;
    };
    if base.is_empty() || idx_str.is_empty() || !idx_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // 排除纯数字包 2-14
    if base.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    idx_str.parse().ok()
}

fn hash_model_skin_index(s: &str) -> Option<i32> {
    // m######## or m########-N
    let b = s.as_bytes();
    if b.len() < 9 || (b[0] != b'm' && b[0] != b'M') {
        return None;
    }
    if !b[1..9].iter().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    if b.len() == 9 {
        return Some(0);
    }
    if b.len() > 10 && b[9] == b'-' {
        let rest = &s[10..];
        if rest.chars().all(|c| c.is_ascii_digit()) {
            return rest.parse().ok();
        }
    }
    None
}

/// 从显示名取序号：`皮肤2` / `爱宕·皮肤2` / `爱宕换装2`
fn skin_index_from_label(name: &str) -> Option<i32> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    // 优先「换装N」，再「皮肤N」
    for key in ["换装", "皮肤"] {
        if let Some(pos) = name.rfind(key) {
            let after = name[pos + key.len()..].trim_start();
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return digits.parse().ok();
            }
            // 仅「换装」无数字时，与 wiki 启发式一致：视为皮肤序号 2
            if key == "换装" && after.chars().next().map(|c| !c.is_ascii_digit()).unwrap_or(true) {
                let before_ok = pos == 0
                    || name[..pos]
                        .chars()
                        .last()
                        .map(|c| c.is_whitespace() || "·・_-".contains(c))
                        .unwrap_or(true);
                if before_ok && after.is_empty() {
                    return Some(2);
                }
            }
        }
    }
    None
}

/// 从 skin_index / 显示名 / kanmusu_dir / id / model_id 推断逻辑皮肤序号
fn infer_skin_index(skin: &CharacterSkinMeta) -> Option<i32> {
    if let Some(idx) = skin.skin_index {
        return Some(idx);
    }
    // 显示名「皮肤2」「换装2」优于数字包 model_id（如 2-9），避免与 aidang_2 拆成两行
    if let Some(idx) = skin_index_from_label(&skin.name) {
        return Some(idx);
    }
    if let Some(kd) = skin
        .kanmusu_dir
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if let Some(idx) = trailing_skin_index(kd) {
            return Some(idx);
        }
        // 无后缀的 kanmusu 基目录视为默认皮
        if !is_numeric_pack_id(kd) && !is_hash_persona_id(kd) {
            return Some(0);
        }
    }
    let sid = skin.id.trim();
    if sid == "default" {
        return Some(0);
    }
    let body = sid.strip_prefix("skin-").unwrap_or(sid);
    if is_numeric_pack_id(body) {
        return None;
    }
    if let Some(idx) = hash_model_skin_index(body) {
        return Some(idx);
    }
    if let Some(idx) = trailing_skin_index(body) {
        return Some(idx);
    }
    let mid = skin.model_id.trim();
    if !mid.is_empty() {
        if is_numeric_pack_id(mid) {
            return None;
        }
        if let Some(idx) = hash_model_skin_index(mid) {
            return Some(idx);
        }
        if is_hash_persona_id(mid) {
            return Some(0);
        }
        if let Some(idx) = trailing_skin_index(mid) {
            return Some(idx);
        }
    }
    None
}

/// 有序号的皮肤：优先用 kanmusu_dir / `{canon}_{idx}` 作为稳定 id
fn normalize_indexed_skin_id(skin: &mut CharacterSkinMeta, idx: i32, canon_id: &str) {
    if idx == 0 {
        skin.id = "default".into();
        return;
    }
    if let Some(kd) = skin
        .kanmusu_dir
        .as_ref()
        .map(|d| d.trim())
        .filter(|d| !d.is_empty())
    {
        skin.id = kd.to_string();
        return;
    }
    let id = skin.id.trim();
    if id.is_empty() || id.starts_with("skin-") || hash_model_skin_index(id).is_some() {
        skin.id = format!("{canon_id}_{idx}");
    }
}

/// 同序号皮肤合并为一行（Spine model_id + Cubism kanmusu_dir）
fn coalesce_skins_by_index(
    skins: &[CharacterSkinMeta],
    canon_id: &str,
) -> (Vec<CharacterSkinMeta>, bool) {
    let mut by_idx: HashMap<i32, CharacterSkinMeta> = HashMap::new();
    let mut leftovers: Vec<CharacterSkinMeta> = Vec::new();
    let mut changed = false;

    for s in skins {
        let Some(idx) = infer_skin_index(s) else {
            leftovers.push(s.clone());
            continue;
        };
        if let Some(cur) = by_idx.get_mut(&idx) {
            let before = cur.clone();
            merge_skin_meta(cur, s);
            if cur.skin_index.is_none() {
                cur.skin_index = Some(idx);
            }
            // 名称：换装 优于 皮肤N 占位；否则保留更具体的非「默认」
            if s.name.contains("换装") && cur.name.contains("皮肤") {
                cur.name = s.name.clone();
            } else if (cur.name == "默认" || cur.name.trim().is_empty())
                && !s.name.trim().is_empty()
                && s.name != "默认"
            {
                cur.name = s.name.clone();
            }
            normalize_indexed_skin_id(cur, idx, canon_id);
            if before.id != cur.id
                || before.model_id != cur.model_id
                || before.kanmusu_dir != cur.kanmusu_dir
                || before.name != cur.name
            {
                changed = true;
            }
        } else {
            let mut row = s.clone();
            row.skin_index = Some(idx);
            let old_id = row.id.clone();
            normalize_indexed_skin_id(&mut row, idx, canon_id);
            if row.id != old_id || row.skin_index != s.skin_index {
                changed = true;
            }
            by_idx.insert(idx, row);
        }
    }

    let mut merged: Vec<CharacterSkinMeta> = by_idx.into_values().collect();
    merged.sort_by(|a, b| {
        let ia = a.skin_index.unwrap_or(10_000);
        let ib = b.skin_index.unwrap_or(10_000);
        ia.cmp(&ib).then_with(|| a.id.cmp(&b.id))
    });

    let mut seen_models: HashSet<String> = merged
        .iter()
        .map(|s| s.model_id.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for mut s in leftovers {
        let mid = s.model_id.trim().to_string();
        if !mid.is_empty() && !seen_models.insert(mid.clone()) {
            changed = true;
            continue;
        }
        if !mid.is_empty() && !s.id.starts_with("skin-") {
            s.id = format!("skin-{mid}");
            changed = true;
        } else if s.id.trim().is_empty() {
            s.id = format!("skin-{}", if mid.is_empty() { "extra" } else { &mid });
            changed = true;
        }
        merged.push(s);
    }

    if merged.len() != skins.len() {
        changed = true;
    }
    // 确保只有一个 default 标记
    if let Some(def_pos) = merged.iter().position(|s| s.default) {
        for (i, s) in merged.iter_mut().enumerate() {
            s.default = i == def_pos;
        }
    } else if let Some(first) = merged.first_mut() {
        first.default = true;
        changed = true;
    }

    (merged, changed)
}

/// 根据合并前后皮肤列表，推断旧 skin id → 新 skin id
fn build_skin_id_remap(
    old: &[CharacterSkinMeta],
    new: &[CharacterSkinMeta],
) -> HashMap<String, String> {
    let mut by_idx: HashMap<i32, String> = HashMap::new();
    let mut by_model: HashMap<String, String> = HashMap::new();
    let mut by_km: HashMap<String, String> = HashMap::new();
    let new_ids: HashSet<&str> = new.iter().map(|s| s.id.as_str()).collect();
    for s in new {
        if let Some(idx) = s.skin_index.or_else(|| infer_skin_index(s)) {
            by_idx.entry(idx).or_insert_with(|| s.id.clone());
        }
        let mid = s.model_id.trim();
        if !mid.is_empty() {
            by_model.entry(mid.to_string()).or_insert_with(|| s.id.clone());
        }
        if let Some(kd) = s
            .kanmusu_dir
            .as_ref()
            .map(|d| d.trim())
            .filter(|d| !d.is_empty())
        {
            by_km.entry(kd.to_string()).or_insert_with(|| s.id.clone());
        }
    }
    let mut remap = HashMap::new();
    for s in old {
        if new_ids.contains(s.id.as_str()) {
            continue;
        }
        if let Some(idx) = infer_skin_index(s) {
            if let Some(nid) = by_idx.get(&idx) {
                remap.insert(s.id.clone(), nid.clone());
                continue;
            }
        }
        let mid = s.model_id.trim();
        if !mid.is_empty() {
            if let Some(nid) = by_model.get(mid) {
                remap.insert(s.id.clone(), nid.clone());
                continue;
            }
        }
        if let Some(kd) = s
            .kanmusu_dir
            .as_ref()
            .map(|d| d.trim())
            .filter(|d| !d.is_empty())
        {
            if let Some(nid) = by_km.get(kd) {
                remap.insert(s.id.clone(), nid.clone());
            }
        }
    }
    remap
}

fn coalesce_character_skins(character: &mut CharacterMeta) -> (bool, HashMap<String, String>) {
    let old_skins = character.skins.clone();
    let (merged, changed) = coalesce_skins_by_index(&character.skins, &character.id);
    let remap = build_skin_id_remap(&old_skins, &merged);
    if !changed
        && remap.is_empty()
        && merged
            .iter()
            .zip(character.skins.iter())
            .all(|(a, b)| a.id == b.id && a.model_id == b.model_id && a.kanmusu_dir == b.kanmusu_dir)
    {
        return (false, remap);
    }
    let old_pref = character.preferred_skin_id.clone();
    character.skins = merged;
    if let Some(ref pref) = old_pref {
        if let Some(nid) = remap.get(pref) {
            character.preferred_skin_id = Some(nid.clone());
        } else if !character.skins.iter().any(|s| s.id == *pref) {
            character.preferred_skin_id = character
                .skins
                .iter()
                .find(|s| s.default)
                .or_else(|| character.skins.first())
                .map(|s| s.id.clone());
        }
    }
    (true, remap)
}

/// `aidang_2` → (`aidang`, 2, "皮肤 2")；无后缀 → index 0
fn split_kanmusu_folder(folder: &str) -> Option<(String, i32, String)> {
    let slug = folder.trim().to_lowercase();
    if slug.is_empty() || is_numeric_pack_id(&slug) || is_hash_persona_id(&slug) {
        return None;
    }
    if let Some(idx) = trailing_skin_index(&slug) {
        let base = slug
            .rsplit_once('_')
            .map(|(b, _)| b)
            .or_else(|| slug.rsplit_once('-').map(|(b, _)| b))?;
        if base.is_empty() {
            return None;
        }
        return Some((base.to_string(), idx, format!("皮肤 {idx}")));
    }
    Some((slug, 0, "默认".into()))
}

/// 将 AppData `kanmusu-models/*` 挂到已有人物皮肤（不新建角色）
fn attach_kanmusu_dirs_to_manifest(data_dir: &Path, manifest: &mut CharacterManifest) -> bool {
    let root = crate::data_layout::kanmusu_models_dir(data_dir);
    if !root.is_dir() {
        return false;
    }
    let Ok(entries) = fs::read_dir(&root) else {
        return false;
    };
    let mut changed = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().trim().to_string();
        let Some((char_id, idx, skin_name)) = split_kanmusu_folder(&slug) else {
            continue;
        };
        if !kanmusu_model_ready(data_dir, &slug) {
            continue;
        }
        let Some(character) = manifest.characters.iter_mut().find(|c| c.id == char_id) else {
            continue;
        };
        if character.skins.iter().any(|s| {
            s.kanmusu_dir
                .as_ref()
                .map(|d| d.trim() == slug)
                .unwrap_or(false)
        }) {
            continue;
        }
        if let Some(skin) = character
            .skins
            .iter_mut()
            .find(|s| infer_skin_index(s) == Some(idx))
        {
            let empty_km = skin
                .kanmusu_dir
                .as_ref()
                .map(|d| d.trim().is_empty())
                .unwrap_or(true);
            if empty_km {
                skin.kanmusu_dir = Some(slug);
                if skin.skin_index.is_none() {
                    skin.skin_index = Some(idx);
                }
                if idx != 0 && (skin.name.trim().is_empty() || skin.name == "默认") {
                    skin.name = skin_name;
                }
                changed = true;
            }
            continue;
        }
        let mut skin = CharacterSkinMeta::spine(
            if idx == 0 {
                "default".to_string()
            } else {
                slug.clone()
            },
            if idx == 0 {
                "默认".to_string()
            } else {
                skin_name
            },
            String::new(),
            character.skins.is_empty(),
        );
        skin.skin_index = Some(idx);
        skin.kanmusu_dir = Some(slug);
        character.skins.push(skin);
        changed = true;
    }
    changed
}

/// kanmusu 目录名 → 若 pet-models 下存在同名/skin- 前缀则回填 Spine model_id
fn probe_model_id_for_folder(
    data_dir: &Path,
    folder: &str,
    model_ids: Option<&HashSet<String>>,
) -> Option<String> {
    let folder = folder.trim();
    if folder.is_empty() {
        return None;
    }
    let mut dir_hit: Option<String> = None;
    for candidate in [folder.to_string(), format!("skin-{folder}")] {
        if model_is_ready(data_dir, &candidate, model_ids) {
            return Some(candidate);
        }
        if dir_hit.is_none() && models::models_dir(data_dir).join(&candidate).is_dir() {
            dir_hit = Some(candidate);
        }
    }
    // 与 Python 导入一致：目录在即可挂上 model_id，就绪态再由 UI/校验判断
    dir_hit
}

fn backfill_skin_model_ids(
    data_dir: &Path,
    skins: &mut [CharacterSkinMeta],
    model_ids: Option<&HashSet<String>>,
) -> bool {
    let mut changed = false;
    for skin in skins.iter_mut() {
        if !skin.model_id.trim().is_empty() {
            continue;
        }
        let Some(kd) = skin
            .kanmusu_dir
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        if let Some(mid) = probe_model_id_for_folder(data_dir, kd, model_ids) {
            skin.model_id = mid;
            changed = true;
        }
    }
    changed
}

/// 去重并移除「已有可用 Spine 时仍显示的默认占位皮肤」；保留舰娘-only 皮（空 model_id）。
fn normalize_character_skins(
    data_dir: &Path,
    skins: &[CharacterSkinMeta],
    model_ids: Option<&HashSet<String>>,
) -> Vec<CharacterSkinMeta> {
    let mut out: Vec<CharacterSkinMeta> = Vec::new();
    let mut seen_skin_ids: HashSet<String> = HashSet::new();
    let mut seen_model_ids: HashSet<String> = HashSet::new();

    for skin in skins {
        if !seen_skin_ids.insert(skin.id.clone()) {
            continue;
        }
        let mid = skin.model_id.trim();
        if !mid.is_empty() && !seen_model_ids.insert(mid.to_string()) {
            continue;
        }
        out.push(skin.clone());
    }

    let _ = backfill_skin_model_ids(data_dir, &mut out, model_ids);

    let has_ready_spine = out
        .iter()
        .any(|s| model_is_ready(data_dir, &s.model_id, model_ids));
    if has_ready_spine {
        out.retain(|s| {
            model_is_ready(data_dir, &s.model_id, model_ids)
                || s.kanmusu_dir
                    .as_ref()
                    .map(|d| !d.trim().is_empty())
                    .unwrap_or(false)
        });
    }

    if out.is_empty() {
        return skins.to_vec();
    }
    out
}

/// 回填/挂载/合并皮肤；第二项为旧 skin id → 新 skin id
pub fn repair_character_manifest_skins(
    data_dir: &Path,
    manifest: &mut CharacterManifest,
) -> bool {
    repair_character_manifest_skins_ex(data_dir, manifest).0
}

fn repair_character_manifest_skins_ex(
    data_dir: &Path,
    manifest: &mut CharacterManifest,
) -> (bool, HashMap<String, String>) {
    let mut changed = false;
    let mut skin_remap = HashMap::new();

    changed |= attach_kanmusu_dirs_to_manifest(data_dir, manifest);

    for c in &mut manifest.characters {
        changed |= backfill_skin_model_ids(data_dir, &mut c.skins, None);
        let (coalesced, local_remap) = coalesce_character_skins(c);
        changed |= coalesced;
        skin_remap.extend(local_remap.iter().map(|(k, v)| (k.clone(), v.clone())));

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
            if let Some(nid) = local_remap.get(pref) {
                c.preferred_skin_id = Some(nid.clone());
                changed = true;
            } else if !c.skins.iter().any(|s| s.id == *pref) {
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
    (changed, skin_remap)
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
    let kanmusu_dir = s.kanmusu_dir.clone().filter(|d| !d.trim().is_empty());
    let kanmusu_ready = kanmusu_dir
        .as_ref()
        .map(|d| kanmusu_model_ready(data_dir, d))
        .unwrap_or(false);
    let spine_id = s.model_id.trim();
    let model_ready = !spine_id.is_empty() && model_is_ready(data_dir, spine_id, model_ids);
    CharacterSkinInfo {
        id: s.id.clone(),
        name: s.name.clone(),
        model_id: if spine_id.is_empty() {
            String::new()
        } else {
            models::canonical_model_id(spine_id)
        },
        model_name: if spine_id.is_empty() {
            String::new()
        } else if let Some(cache) = model_cache {
            model_name_cached(spine_id, cache)
        } else {
            model_name(data_dir, spine_id)
        },
        active: s.id == active_skin_id,
        model_ready,
        skin_index: s.skin_index,
        kanmusu_dir,
        kanmusu_ready,
        lines_count: s.lines.len(),
        english_name: s.english_name.clone(),
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
        if manifest
            .characters
            .iter()
            .any(|c| c.persona_id == p.id || c.id == p.id)
        {
            continue;
        }
        // 同中文名已有人物时不再生出 hash 双胞胎
        let pname = p.name.trim();
        if !pname.is_empty()
            && manifest
                .characters
                .iter()
                .any(|c| c.name.trim() == pname)
        {
            continue;
        }
        let model_id = default_skin_model_id(&p.id);
        let mut meta = CharacterMeta {
            id: p.id.clone(),
            name: p.name.clone(),
            source: p.source.clone(),
            description: p.description.clone(),
            persona_id: p.id.clone(),
            skins: vec![CharacterSkinMeta::spine("default", "默认", model_id, true)],
            preferred_skin_id: None,
            faction: String::new(),
            ship_type: String::new(),
            rarity: String::new(),
            english_name: String::new(),
            wiki_title: String::new(),
            cv: String::new(),
        };
        meta.skins = normalize_character_skins(data_dir, &meta.skins, Some(model_ids));
        manifest.characters.push(meta);
    }
    let _ = dedupe_characters_by_name(&mut manifest.characters);
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
        skins: vec![CharacterSkinMeta::spine("default", "默认", model_id, true)],
        preferred_skin_id: None,
        faction: String::new(),
        ship_type: String::new(),
        rarity: String::new(),
        english_name: String::new(),
        wiki_title: String::new(),
        cv: String::new(),
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
        c.english_name.as_str(),
        c.wiki_title.as_str(),
        c.cv.as_str(),
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
        english_name: c.english_name.clone(),
        wiki_title: c.wiki_title.clone(),
        cv: c.cv.clone(),
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
        indices.sort_by(|a, b| {
            let ra = rank
                .get(items[*a].id.as_str())
                .copied()
                .unwrap_or(usize::MAX);
            let rb = rank
                .get(items[*b].id.as_str())
                .copied()
                .unwrap_or(usize::MAX);
            ra.cmp(&rb).then_with(|| a.cmp(b))
        });
    }
    indices
}

/// 将 personas/manifest 同步写入 characters/manifest（CLI 导入、启动迁移）
/// 返回 (是否写盘, 角色 id 重映射, 皮肤 id 重映射)
pub fn sync_character_manifest_from_personas(
    data_dir: &Path,
) -> Result<(bool, HashMap<String, String>, HashMap<String, String>), String> {
    crate::manifest_lock::with_lock(|| {
        let persona_manifest = persona::load_manifest(data_dir);
        let mut manifest = load_manifest(data_dir);
        let mut changed = false;
        let mut skin_remap = HashMap::new();

        changed |= repair_default_skin_models(&mut manifest);
        let (repaired, r1) = repair_character_manifest_skins_ex(data_dir, &mut manifest);
        changed |= repaired;
        skin_remap.extend(r1);

        for p in &persona_manifest.personas {
            if let Some(c) = manifest
                .characters
                .iter_mut()
                .find(|c| c.persona_id == p.id || c.id == p.id)
            {
                if c.name != p.name || c.source != p.source || c.description != p.description {
                    c.name = p.name.clone();
                    c.source = p.source.clone();
                    c.description = p.description.clone();
                    changed = true;
                }
                continue;
            }

            let pname = p.name.trim();
            if !pname.is_empty()
                && manifest
                    .characters
                    .iter()
                    .any(|c| c.name.trim() == pname)
            {
                // 已有同名拼音角色：挂到已有条目的元数据即可，不新增 hash 双胞胎
                if let Some(c) = manifest
                    .characters
                    .iter_mut()
                    .find(|c| c.name.trim() == pname)
                {
                    if c.source.trim().is_empty() && !p.source.trim().is_empty() {
                        c.source = p.source.clone();
                        changed = true;
                    }
                    if c.description.trim().is_empty() && !p.description.trim().is_empty() {
                        c.description = p.description.clone();
                        changed = true;
                    }
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
                skins: vec![CharacterSkinMeta::spine("default", "默认", model_id, true)],
                preferred_skin_id: None,
                faction: String::new(),
                ship_type: String::new(),
                rarity: String::new(),
                english_name: String::new(),
                wiki_title: String::new(),
                cv: String::new(),
            });
            changed = true;
        }

        let (deduped, remap) = dedupe_characters_by_name(&mut manifest.characters);
        changed |= deduped;
        // 去重后再回填/挂载一次
        let (repaired2, r2) = repair_character_manifest_skins_ex(data_dir, &mut manifest);
        changed |= repaired2;
        skin_remap.extend(r2);

        if changed {
            write_manifest(data_dir, &manifest)?;
        }
        Ok((changed, remap, skin_remap))
    })
}

/// 同步 Cubism 目录后挂到人物皮肤（与 sync_from_unpacked 配套，在锁外调用）
pub fn attach_kanmusu_after_sync(data_dir: &Path) -> Result<bool, String> {
    mutate_character_manifest(data_dir, |manifest| {
        let (changed, _) = repair_character_manifest_skins_ex(data_dir, manifest);
        Ok(changed)
    })
}

fn remap_favorite_character_ids(
    db: &rusqlite::Connection,
    remap: &HashMap<String, String>,
) -> Result<(), String> {
    if remap.is_empty() {
        return Ok(());
    }
    let prev = favorite_character_ids(db);
    if prev.is_empty() {
        return Ok(());
    }
    let mut seen = HashSet::new();
    let mut next = Vec::with_capacity(prev.len());
    for id in &prev {
        let mapped = remap.get(id).cloned().unwrap_or_else(|| id.clone());
        if seen.insert(mapped.clone()) {
            next.push(mapped);
        }
    }
    if next == prev {
        return Ok(());
    }
    let json = serde_json::to_string(&next).map_err(|e| e.to_string())?;
    crate::db::set_setting(db, FAVORITES_SETTING_KEY, &json).map_err(|e| e.to_string())
}

fn remap_setting_id(
    db: &rusqlite::Connection,
    key: &str,
    remap: &HashMap<String, String>,
) -> Result<(), String> {
    if remap.is_empty() {
        return Ok(());
    }
    let Some(cur) = crate::db::get_setting(db, key) else {
        return Ok(());
    };
    let trimmed = cur.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let Some(mapped) = remap.get(trimmed) else {
        return Ok(());
    };
    if mapped == trimmed {
        return Ok(());
    }
    crate::db::set_setting(db, key, mapped).map_err(|e| e.to_string())
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
    let active_skin = active_skin_id(db);
    build_pet_menu_skins_payload(data_dir, &character_id, &model_id, active_skin.as_deref())
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
    let active_skin = if current_char == character_id {
        active_skin_id(db)
    } else {
        None
    };
    build_pet_menu_skins_payload(
        data_dir,
        character_id,
        &active_model_id,
        active_skin.as_deref(),
    )
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
    active_skin_id: Option<&str>,
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
                active_skin_id.unwrap_or(""),
                Some(&model_cache),
                Some(model_ids_set.as_ref()),
            );
            let by_skin = active_skin_id == Some(s.id.as_str());
            let by_model = !s.model_id.trim().is_empty()
                && models::canonical_model_id(&s.model_id)
                    == models::canonical_model_id(active_model_id);
            info.active = by_skin || by_model;
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

/// 有舰娘 Cubism 的人物（菜单 kanmusu 模式）
pub fn list_characters_with_kanmusu(
    data_dir: &Path,
    db: &rusqlite::Connection,
) -> Vec<CharacterBrief> {
    let all = resolve_all_characters(data_dir);
    let active_id = active_character_id(db, data_dir);
    all.iter()
        .filter(|c| {
            c.skins.iter().any(|s| {
                s.kanmusu_dir
                    .as_ref()
                    .map(|d| !d.trim().is_empty())
                    .unwrap_or(false)
            })
        })
        .map(|c| meta_to_brief(data_dir, c, &active_id, db, true, None))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct CharacterSkinDetail {
    pub id: String,
    pub name: String,
    pub model_id: String,
    pub model_ready: bool,
    pub skin_index: Option<i32>,
    pub kanmusu_dir: Option<String>,
    pub kanmusu_ready: bool,
    #[serde(default)]
    pub english_name: String,
    pub lines: Vec<CharacterSkinLine>,
}

pub fn get_skin_detail(
    data_dir: &Path,
    character_id: &str,
    skin_id: &str,
) -> Result<CharacterSkinDetail, String> {
    let meta = find_character_meta(data_dir, character_id)?;
    let skin = meta
        .skins
        .iter()
        .find(|s| s.id == skin_id)
        .ok_or_else(|| format!("未知皮肤: {skin_id}"))?;
    let info = build_skin_info(data_dir, skin, skin_id, None, None);
    Ok(CharacterSkinDetail {
        id: skin.id.clone(),
        name: skin.name.clone(),
        model_id: info.model_id,
        model_ready: info.model_ready,
        skin_index: skin.skin_index,
        kanmusu_dir: info.kanmusu_dir,
        kanmusu_ready: info.kanmusu_ready,
        english_name: skin.english_name.clone(),
        lines: skin.lines.clone(),
    })
}

pub fn update_skin_lines(
    data_dir: &Path,
    character_id: &str,
    skin_id: &str,
    lines: Vec<CharacterSkinLine>,
) -> Result<CharacterSkinDetail, String> {
    mutate_character_manifest(data_dir, |manifest| {
        let character = manifest
            .characters
            .iter_mut()
            .find(|c| c.id == character_id)
            .ok_or_else(|| format!("未知人物: {character_id}"))?;
        let skin = character
            .skins
            .iter_mut()
            .find(|s| s.id == skin_id)
            .ok_or_else(|| format!("未知皮肤: {skin_id}"))?;
        skin.lines = lines
            .into_iter()
            .filter(|l| !l.text.trim().is_empty())
            .collect();
        Ok(())
    })?;
    get_skin_detail(data_dir, character_id, skin_id)
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
    persona::ensure_persona_stub(
        data_dir,
        &meta.persona_id,
        &meta.name,
        &meta.source,
        &meta.description,
    )?;
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
        english_name: meta.english_name.clone(),
        wiki_title: meta.wiki_title.clone(),
        cv: meta.cv.clone(),
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
    _persona_manifest: &PersonaManifest,
    character_id: &str,
) -> Result<(), String> {
    let meta = find_character_meta(data_dir, character_id)?;
    let normalized = normalize_character_skins(data_dir, &meta.skins, None);
    let pref_id = resolve_preferred_skin_id(&meta);
    let skin = normalized
        .iter()
        .find(|s| s.id == pref_id)
        .or_else(|| normalized.iter().find(|s| s.default))
        .or_else(|| {
            normalized.iter().find(|s| {
                model_is_ready(data_dir, &s.model_id, None)
                    || s.kanmusu_dir
                        .as_ref()
                        .map(|d| kanmusu_model_ready(data_dir, d))
                        .unwrap_or(false)
            })
        })
        .or_else(|| normalized.first())
        .ok_or_else(|| format!("人物 {character_id} 无可用皮肤"))?;
    let spine_ok = model_is_ready(data_dir, &skin.model_id, None);
    let kanmusu_ok = skin
        .kanmusu_dir
        .as_ref()
        .map(|d| kanmusu_model_ready(data_dir, d))
        .unwrap_or(false);
    if !spine_ok && !kanmusu_ok {
        return Err("该人物皮肤模型尚未完成，请先导入桌宠或舰娘模型".into());
    }
    persona::ensure_persona_stub(
        data_dir,
        &meta.persona_id,
        &meta.name,
        &meta.source,
        &meta.description,
    )?;
    let persona_manifest = persona::load_manifest(data_dir);
    set_active_character_id(db, character_id)?;
    set_active_skin_id(db, &skin.id)?;
    persona::set_active_persona_id(db, &persona_manifest, &meta.persona_id)?;
    if spine_ok {
        models::set_active_model_id(db, &skin.model_id)?;
    }
    Ok(())
}

/// 选择人物皮肤后返回桌宠 / 舰娘目标。
#[derive(Debug, Clone, Serialize)]
pub struct SelectSkinResult {
    pub model_id: String,
    pub kanmusu_dir: Option<String>,
    pub spine_ready: bool,
    pub kanmusu_ready: bool,
}

fn kanmusu_model_ready(data_dir: &Path, slug: &str) -> bool {
    crate::kanmusu::find_model3_json(data_dir, slug).is_some()
}

/// 选择人物皮肤：写入偏好，并切换为当前选用人物；有 Spine 则更新 pet_model_id。
pub fn select_character_skin(
    data_dir: &Path,
    db: &rusqlite::Connection,
    character_id: &str,
    skin_id: &str,
) -> Result<SelectSkinResult, String> {
    let meta = find_character_meta(data_dir, character_id)?;
    let normalized = normalize_character_skins(data_dir, &meta.skins, None);
    let skin = normalized
        .iter()
        .find(|s| s.id == skin_id)
        .ok_or_else(|| format!("未知皮肤: {skin_id}"))?;
    let spine_ready = model_is_ready(data_dir, &skin.model_id, None);
    let kanmusu_dir = skin.kanmusu_dir.clone().filter(|d| !d.trim().is_empty());
    let kanmusu_ready = kanmusu_dir
        .as_ref()
        .map(|d| kanmusu_model_ready(data_dir, d))
        .unwrap_or(false);
    if !spine_ready && !kanmusu_ready {
        return Err("该皮肤模型尚未完成，请先导入桌宠或舰娘模型".into());
    }
    set_preferred_skin_in_manifest(data_dir, character_id, skin_id)?;

    persona::ensure_persona_stub(
        data_dir,
        &meta.persona_id,
        &meta.name,
        &meta.source,
        &meta.description,
    )?;
    let manifest = persona::load_manifest(data_dir);
    set_active_character_id(db, character_id)?;
    set_active_skin_id(db, skin_id)?;
    persona::set_active_persona_id(db, &manifest, &meta.persona_id)?;
    if spine_ready {
        models::set_active_model_id(db, &skin.model_id)?;
    }
    Ok(SelectSkinResult {
        model_id: skin.model_id.clone(),
        kanmusu_dir,
        spine_ready,
        kanmusu_ready,
    })
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
        if model_is_ready(data_dir, &s.model_id, None) {
            let _ = models::set_active_model_id(db, &s.model_id);
        }
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
    character.skins.push(CharacterSkinMeta::spine(
        skin_id.clone(),
        skin_display,
        model.id.clone(),
        false,
    ));
    character.preferred_skin_id.replace(skin_id);
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
    let resolved_skin_id = mutate_character_manifest(data_dir, |manifest| {
        attach_model_in_manifest(data_dir, manifest, character_id, model, skin_name)?;
        let _ = repair_character_manifest_skins(data_dir, manifest);
        let skin_id = manifest
            .characters
            .iter()
            .find(|c| c.id == character_id)
            .and_then(|c| {
                c.skins.iter().find(|s| {
                    s.model_id == model.id
                        || s.model_id == models::canonical_model_id(&model.id)
                })
            })
            .map(|s| s.id.clone())
            .unwrap_or_else(|| format!("skin-{}", model.id));
        if let Some(c) = manifest.characters.iter_mut().find(|c| c.id == character_id) {
            c.preferred_skin_id = Some(skin_id.clone());
        }
        Ok(skin_id)
    })?;

    if set_active {
        set_active_character_id(db, character_id)?;
        set_active_skin_id(db, &resolved_skin_id)?;
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

    let (_, remap, skin_remap) = sync_character_manifest_from_personas(data_dir)?;
    let _ = remap_favorite_character_ids(db, &remap);
    let _ = persona::absorb_merged_persona_ids(data_dir, db, &remap);

    // 当前激活角色 / 舰娘角色若已并入拼音 id，一并改写
    if let Some(active) = crate::db::get_setting(db, ACTIVE_CHARACTER_KEY) {
        if let Some(canon) = remap.get(&active) {
            set_active_character_id(db, canon)?;
        }
    }
    if let Some(km) = crate::db::get_setting(db, "kanmusu_active_character_id") {
        if let Some(canon) = remap.get(km.trim()) {
            let skin = crate::db::get_setting(db, "kanmusu_active_skin_id")
                .unwrap_or_default();
            let mapped_skin = skin_remap
                .get(skin.trim())
                .cloned()
                .unwrap_or(skin);
            let _ = crate::pet::set_kanmusu_active_ids(db, canon, &mapped_skin);
        }
    }
    let _ = remap_setting_id(db, ACTIVE_SKIN_KEY, &skin_remap);
    let _ = remap_setting_id(db, "kanmusu_active_skin_id", &skin_remap);

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
    fn dedupe_merges_hash_into_pinyin_by_name() {
        let mut chars = vec![
            CharacterMeta {
                id: "aijier".into(),
                name: "埃吉尔".into(),
                source: String::new(),
                description: String::new(),
                persona_id: "aijier".into(),
                skins: vec![{
                    let mut s = CharacterSkinMeta::spine("default", "默认", "", true);
                    s.kanmusu_dir = Some("aijier".into());
                    s.skin_index = Some(0);
                    s
                }],
                preferred_skin_id: None,
                faction: String::new(),
                ship_type: String::new(),
                rarity: String::new(),
                english_name: String::new(),
                wiki_title: String::new(),
                cv: String::new(),
            },
            CharacterMeta {
                id: "p92564837".into(),
                name: "埃吉尔".into(),
                source: "wiki".into(),
                description: "desc".into(),
                persona_id: "p92564837".into(),
                skins: vec![{
                    let mut s = CharacterSkinMeta::spine("skin-2", "皮2", "aijier_2", false);
                    s.skin_index = Some(2);
                    s
                }],
                preferred_skin_id: None,
                faction: String::new(),
                ship_type: String::new(),
                rarity: String::new(),
                english_name: "Ägir".into(),
                wiki_title: "埃吉尔".into(),
                cv: String::new(),
            },
        ];
        let (changed, remap) = dedupe_characters_by_name(&mut chars);
        assert!(changed);
        assert_eq!(remap.get("p92564837").map(String::as_str), Some("aijier"));
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].id, "aijier");
        assert_eq!(chars[0].english_name, "Ägir");
        assert!(chars[0].skins.iter().any(|s| {
            s.skin_index == Some(2) || s.model_id == "aijier_2" || s.id.contains('2')
        }));
        assert!(chars[0]
            .skins
            .iter()
            .any(|s| s.kanmusu_dir.as_deref() == Some("aijier")));
    }

    #[test]
    fn backfill_model_id_from_pet_models_dir() {
        let dir = tempfile::tempdir().unwrap();
        let model_dir = dir.path().join("pet-models").join("aidang_2");
        fs::create_dir_all(&model_dir).unwrap();
        let mut skins = vec![{
            let mut s = CharacterSkinMeta::spine("s2", "皮2", "", false);
            s.kanmusu_dir = Some("aidang_2".into());
            s
        }];
        assert!(backfill_skin_model_ids(dir.path(), &mut skins, None));
        assert_eq!(skins[0].model_id, "aidang_2");
    }

    #[test]
    fn attach_kanmusu_dir_onto_existing_character_skin() {
        let dir = tempfile::tempdir().unwrap();
        let km = dir.path().join("kanmusu-models").join("aidang_2");
        fs::create_dir_all(&km).unwrap();
        fs::write(km.join("aidang_2.model3.json"), "{}").unwrap();
        let mut manifest = CharacterManifest {
            version: 1,
            default_id: "aidang".into(),
            characters: vec![CharacterMeta {
                id: "aidang".into(),
                name: "爱宕".into(),
                source: String::new(),
                description: String::new(),
                persona_id: "aidang".into(),
                skins: vec![{
                    let mut s = CharacterSkinMeta::spine("skin-x", "皮2", "m1-2", false);
                    s.skin_index = Some(2);
                    s
                }],
                preferred_skin_id: Some("skin-x".into()),
                faction: String::new(),
                ship_type: String::new(),
                rarity: String::new(),
                english_name: String::new(),
                wiki_title: String::new(),
                cv: String::new(),
            }],
        };
        assert!(attach_kanmusu_dirs_to_manifest(dir.path(), &mut manifest));
        let skin = &manifest.characters[0].skins[0];
        assert_eq!(skin.kanmusu_dir.as_deref(), Some("aidang_2"));
    }

    #[test]
    fn skin_id_remap_after_coalesce() {
        let mut c = CharacterMeta {
            id: "aijier".into(),
            name: "埃吉尔".into(),
            source: String::new(),
            description: String::new(),
            persona_id: "aijier".into(),
            skins: vec![
                {
                    let mut s = CharacterSkinMeta::spine("skin-mxx-2", "皮2", "m92564837-2", false);
                    s.skin_index = Some(2);
                    s
                },
                {
                    let mut s = CharacterSkinMeta::spine("tmp", "皮肤2", "", false);
                    s.kanmusu_dir = Some("aijier_2".into());
                    s
                },
            ],
            preferred_skin_id: Some("skin-mxx-2".into()),
            faction: String::new(),
            ship_type: String::new(),
            rarity: String::new(),
            english_name: String::new(),
            wiki_title: String::new(),
            cv: String::new(),
        };
        let (changed, remap) = coalesce_character_skins(&mut c);
        assert!(changed);
        assert_eq!(remap.get("skin-mxx-2").map(String::as_str), Some("aijier_2"));
        assert_eq!(c.preferred_skin_id.as_deref(), Some("aijier_2"));
        assert_eq!(c.skins.len(), 1);
    }

    #[test]
    fn coalesce_merges_label_skin2_with_kanmusu_huanzhuang2() {
        let skins = vec![
            {
                let mut s = CharacterSkinMeta::spine("aidang_2", "爱宕换装2", "", false);
                s.kanmusu_dir = Some("aidang_2".into());
                s.skin_index = Some(2);
                s
            },
            CharacterSkinMeta::spine("skin-2-9", "皮肤2", "2-9", false),
        ];
        let (merged, changed) = coalesce_skins_by_index(&skins, "aidang");
        assert!(changed);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].model_id, "2-9");
        assert_eq!(merged[0].kanmusu_dir.as_deref(), Some("aidang_2"));
        assert!(merged[0].name.contains("换装"));
        assert_eq!(merged[0].skin_index, Some(2));
    }

    #[test]
    fn skin_index_from_label_variants() {
        assert_eq!(skin_index_from_label("皮肤2"), Some(2));
        assert_eq!(skin_index_from_label("爱宕·皮肤2"), Some(2));
        assert_eq!(skin_index_from_label("爱宕换装2"), Some(2));
        assert_eq!(skin_index_from_label("换装"), Some(2));
    }

    #[test]
    fn coalesce_merges_same_index_spine_and_kanmusu() {
        let skins = vec![
            {
                let mut s = CharacterSkinMeta::spine("default", "默认", "aijier", true);
                s.skin_index = Some(0);
                s
            },
            {
                let mut s = CharacterSkinMeta::spine("skin-aijier", "舰娘默认", "", false);
                s.kanmusu_dir = Some("aijier".into());
                s
            },
            {
                let mut s = CharacterSkinMeta::spine("skin-mxx-2", "皮2", "m92564837-2", false);
                s.skin_index = Some(2);
                s
            },
            {
                let mut s = CharacterSkinMeta::spine("aijier_2", "皮肤2", "", false);
                s.kanmusu_dir = Some("aijier_2".into());
                s
            },
        ];
        let (merged, changed) = coalesce_skins_by_index(&skins, "aijier");
        assert!(changed);
        assert_eq!(merged.len(), 2);
        let def = merged.iter().find(|s| s.skin_index == Some(0)).unwrap();
        assert_eq!(def.model_id, "aijier");
        assert_eq!(def.kanmusu_dir.as_deref(), Some("aijier"));
        let s2 = merged.iter().find(|s| s.skin_index == Some(2)).unwrap();
        assert_eq!(s2.model_id, "m92564837-2");
        assert_eq!(s2.kanmusu_dir.as_deref(), Some("aijier_2"));
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
            CharacterSkinMeta::spine("default", "默认·待导入", "missing-model", true),
            CharacterSkinMeta::spine("ready-skin", "已导入", "chaijun", false),
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
