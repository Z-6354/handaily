//! 人设：仓库 `personas/*.md` + manifest，运行时优先读用户数据目录

pub mod import_reference;

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::character_profiles::CharacterProfileData;

const EMBEDDED_MANIFEST: &str = include_str!("../../../personas/manifest.json");

const EMBEDDED_PERSONAS: &[(&str, &str)] = &[
    ("cheshire", include_str!("../../../personas/cheshire.md")),
    ("edu", include_str!("../../../personas/edu.md")),
    ("wushiling", include_str!("../../../personas/wushiling.md")),
    ("qiye", include_str!("../../../personas/qiye.md")),
    ("tashigan", include_str!("../../../personas/tashigan.md")),
];

const EMBEDDED_PROFILES: &[(&str, &str)] = &[
    ("cheshire", include_str!("../../../personas/cheshire.json")),
    ("edu", include_str!("../../../personas/edu.json")),
    ("wushiling", include_str!("../../../personas/wushiling.json")),
    ("qiye", include_str!("../../../personas/qiye.json")),
    ("tashigan", include_str!("../../../personas/tashigan.json")),
];

/// 旧版 Wiki 导入哈希 ID → 内置 slug（含人设 p 前缀与桌宠 m 前缀）
const LEGACY_BUILTIN_PERSONA_IDS: &[(&str, &str)] = &[
    ("p951a05aa", "edu"),
    ("m951a05aa", "edu"),
    ("pe2795090", "wushiling"),
    ("ma19bdb1b", "wushiling"),
    ("pc5623cfa", "qiye"),
    ("mc5623cfa", "qiye"),
    ("pea9d211a", "tashigan"),
    ("mea9d211a", "tashigan"),
];

/// 已移除的内置人设（启动迁移时从 manifest 清理）
const REMOVED_BUILTIN_PERSONAS: &[&str] = &["default", "phoebe", "sora"];

const ACTIVE_PERSONA_KEY: &str = "active_persona_id";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaManifest {
    pub version: u32,
    pub default_id: String,
    pub personas: Vec<PersonaMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaMeta {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaInfo {
    pub id: String,
    pub name: String,
    pub source: String,
    pub description: String,
    pub active: bool,
    pub has_profile: bool,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaDetail {
    pub id: String,
    pub name: String,
    pub source: String,
    pub description: String,
    pub active: bool,
    pub skill_md: String,
    pub profile_json: CharacterProfileData,
    pub is_builtin: bool,
    pub profile_ai_updated: bool,
    pub profile_ai_updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PersonaImportFile {
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PersonaImportResult {
    pub imported_ids: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaImportProgressEvent {
    pub step: String,
    pub message: String,
    pub step_index: u32,
    pub step_total: u32,
}

#[derive(Debug, Deserialize)]
pub struct PersonaUpdateInput {
    pub name: String,
    pub source: String,
    pub description: String,
    pub skill_md: String,
    pub profile_json: CharacterProfileData,
}

pub fn personas_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("personas")
}

pub fn manifest_path(data_dir: &Path) -> PathBuf {
    personas_dir(data_dir).join("manifest.json")
}

pub fn seed_user_personas(data_dir: &Path) -> std::io::Result<()> {
    let dir = personas_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let manifest = manifest_path(data_dir);
    if !manifest.exists() {
        fs::write(&manifest, EMBEDDED_MANIFEST)?;
    }
    for (id, content) in EMBEDDED_PERSONAS {
        let path = dir.join(format!("{id}.md"));
        if !path.exists() {
            fs::write(&path, content)?;
        }
    }
    for (id, content) in EMBEDDED_PROFILES {
        let path = dir.join(format!("{id}.json"));
        if !path.exists() {
            fs::write(&path, content)?;
        }
    }
    Ok(())
}

/// 将内置人设 Skill / JSON 同步到用户目录（slug 文件为唯一来源）
fn sync_embedded_builtin_files(data_dir: &Path) -> std::io::Result<()> {
    let dir = personas_dir(data_dir);
    fs::create_dir_all(&dir)?;
    for (id, content) in EMBEDDED_PERSONAS {
        let path = dir.join(format!("{id}.md"));
        if !path.exists() {
            fs::write(&path, content)?;
        }
    }
    for (id, content) in EMBEDDED_PROFILES {
        let path = dir.join(format!("{id}.json"));
        if !path.exists() {
            fs::write(&path, content)?;
        }
    }
    Ok(())
}

fn remove_legacy_persona_files(data_dir: &Path) {
    let dir = personas_dir(data_dir);
    for (legacy, _) in LEGACY_BUILTIN_PERSONA_IDS {
        for ext in ["md", "json"] {
            let path = dir.join(format!("{legacy}.{ext}"));
            let _ = fs::remove_file(path);
        }
    }
}

/// 启动迁移：补齐内置五人设、清理已删预设、修正 legacy ID
pub fn migrate_legacy_personas(data_dir: &Path, db: &rusqlite::Connection) -> Result<(), String> {
    let embedded: PersonaManifest =
        serde_json::from_str(EMBEDDED_MANIFEST).expect("embedded personas/manifest.json");

    let manifest: PersonaManifest = crate::manifest_lock::with_lock(|| -> Result<PersonaManifest, String> {
        let mut manifest = load_manifest(data_dir);
        let mut changed = false;

        let before = manifest.personas.len();
        manifest
            .personas
            .retain(|p| !REMOVED_BUILTIN_PERSONAS.contains(&p.id.as_str()));
        if manifest.personas.len() != before {
            changed = true;
        }

        for bp in &embedded.personas {
            if !manifest.personas.iter().any(|p| p.id == bp.id) {
                manifest.personas.push(bp.clone());
                changed = true;
            }
        }

        for (legacy, slug) in LEGACY_BUILTIN_PERSONA_IDS {
            if let Some(pos) = manifest.personas.iter().position(|p| p.id == *legacy) {
                if manifest.personas.iter().any(|p| p.id == *slug) {
                    manifest.personas.remove(pos);
                } else {
                    manifest.personas[pos].id = (*slug).to_string();
                }
                changed = true;
            }
        }

        if !manifest.personas.iter().any(|p| p.id == manifest.default_id) {
            manifest.default_id = embedded.default_id.clone();
            changed = true;
        }

        if changed {
            write_persona_manifest(data_dir, &manifest)?;
        }
        Ok(manifest)
    })?;

    let active = active_persona_id(db, &manifest);
    let resolved_active = LEGACY_BUILTIN_PERSONA_IDS
        .iter()
        .find(|(legacy, _)| *legacy == active.as_str())
        .map(|(_, slug)| *slug)
        .unwrap_or(active.as_str());
    if resolved_active != active
        || REMOVED_BUILTIN_PERSONAS.contains(&active.as_str())
        || !manifest.personas.iter().any(|p| p.id == active)
    {
        let fallback = if manifest.personas.iter().any(|p| p.id == manifest.default_id) {
            manifest.default_id.as_str()
        } else {
            "cheshire"
        };
        set_active_persona_id(db, &manifest, fallback)?;
    }

    sync_embedded_builtin_files(data_dir).map_err(|e| e.to_string())?;
    remove_legacy_persona_files(data_dir);

    Ok(())
}

pub fn load_manifest(data_dir: &Path) -> PersonaManifest {
    let path = manifest_path(data_dir);
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(m) = serde_json::from_str(&raw) {
            return m;
        }
    }
    serde_json::from_str(EMBEDDED_MANIFEST).expect("embedded personas/manifest.json")
}

fn write_persona_manifest(data_dir: &Path, manifest: &PersonaManifest) -> Result<(), String> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    crate::manifest_lock::atomic_write(&manifest_path(data_dir), &json)
}

fn mutate_persona_manifest<F, T>(data_dir: &Path, f: F) -> Result<T, String>
where
    F: FnOnce(&mut PersonaManifest) -> Result<T, String>,
{
    crate::manifest_lock::with_lock(|| {
        let mut manifest = load_manifest(data_dir);
        let result = f(&mut manifest)?;
        write_persona_manifest(data_dir, &manifest)?;
        Ok(result)
    })
}

pub fn is_builtin_persona(id: &str) -> bool {
    EMBEDDED_PERSONAS.iter().any(|(pid, _)| *pid == id)
}

pub fn list_personas(data_dir: &Path, db: &rusqlite::Connection) -> Vec<PersonaInfo> {
    let manifest = load_manifest(data_dir);
    let active = active_persona_id(db, &manifest);
    manifest
        .personas
        .iter()
        .map(|p| PersonaInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            source: p.source.clone(),
            description: p.description.clone(),
            active: p.id == active,
            has_profile: load_persona_profile(data_dir, &p.id).is_some(),
            is_builtin: is_builtin_persona(&p.id),
        })
        .collect()
}

pub fn get_persona_detail(
    data_dir: &Path,
    db: &rusqlite::Connection,
    id: &str,
) -> Result<PersonaDetail, String> {
    let manifest = load_manifest(data_dir);
    let meta = manifest
        .personas
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("未知人设: {id}"))?;
    let active = active_persona_id(db, &manifest) == id;
    let char_row = crate::db::character_profiles::find_by_persona_id(db, id).ok().flatten();

    let mut skill_md = load_persona_body(data_dir, id).unwrap_or_default();
    if skill_md.is_empty() {
        if let Some(ref row) = char_row {
            skill_md = row.skill_md.clone();
        }
    }

    let is_builtin = is_builtin_persona(id);

    let file_profile = load_persona_profile(data_dir, id);
    let profile_json = match (file_profile, char_row.as_ref()) {
        (Some(file), Some(row))
            if profile_has_rich_content(&file) || !profile_has_rich_content(&row.profile_json) =>
        {
            file
        }
        (Some(file), None) => file,
        (None, Some(row)) if profile_has_rich_content(&row.profile_json) => {
            row.profile_json.clone()
        }
        (None, Some(row))
            if !row.profile_json.name.is_empty() || !row.profile_json.introduction.is_empty() =>
        {
            row.profile_json.clone()
        }
        _ => profile_from_meta(meta),
    };

    let (profile_ai_updated, profile_ai_updated_at) = if is_builtin {
        (false, None)
    } else {
        crate::persona_builder::resolve_profile_ai_update_meta(data_dir, id, &profile_json)
    };

    Ok(PersonaDetail {
        id: meta.id.clone(),
        name: meta.name.clone(),
        source: meta.source.clone(),
        description: meta.description.clone(),
        active,
        skill_md,
        profile_json,
        is_builtin,
        profile_ai_updated,
        profile_ai_updated_at,
    })
}

fn profile_from_meta(meta: &PersonaMeta) -> CharacterProfileData {
    CharacterProfileData {
        name: meta.name.clone(),
        source: meta.source.clone(),
        introduction: meta.description.clone(),
        ..Default::default()
    }
}

fn profile_has_rich_content(p: &CharacterProfileData) -> bool {
    !p.introduction.trim().is_empty()
        || !p.personality.is_empty()
        || !p.speech_style.trim().is_empty()
        || !p.sample_lines.is_empty()
        || !p.relationships.trim().is_empty()
        || !p.taboos.is_empty()
        || !p.extra.is_empty()
}

pub fn active_persona_id(db: &rusqlite::Connection, manifest: &PersonaManifest) -> String {
    db::get_setting(db, ACTIVE_PERSONA_KEY).unwrap_or_else(|| manifest.default_id.clone())
}

pub fn set_active_persona_id(
    db: &rusqlite::Connection,
    manifest: &PersonaManifest,
    id: &str,
) -> Result<(), String> {
    if !manifest.personas.iter().any(|p| p.id == id) {
        return Err(format!("未知人设: {id}"));
    }
    db::set_setting(db, ACTIVE_PERSONA_KEY, id).map_err(|e| e.to_string())
}

pub fn save_persona_file(data_dir: &Path, id: &str, body: &str) -> Result<(), String> {
    let dir = personas_dir(data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{id}.md"));
    std::fs::write(&path, body.trim()).map_err(|e| e.to_string())
}

pub fn upsert_manifest_entry(data_dir: &Path, meta: &PersonaMeta) -> Result<(), String> {
    mutate_persona_manifest(data_dir, |manifest| {
        if let Some(p) = manifest.personas.iter_mut().find(|p| p.id == meta.id) {
            p.name = meta.name.clone();
            p.source = meta.source.clone();
            p.description = meta.description.clone();
        } else {
            manifest.personas.push(meta.clone());
        }
        Ok(())
    })
}

pub fn update_persona(
    data_dir: &Path,
    id: &str,
    input: &PersonaUpdateInput,
) -> Result<(), String> {
    let manifest = load_manifest(data_dir);
    if !manifest.personas.iter().any(|p| p.id == id) {
        return Err(format!("未知人设: {id}"));
    }
    let name = input.name.trim();
    if name.is_empty() {
        return Err("显示名称不能为空".into());
    }
    save_persona_file(data_dir, id, &input.skill_md)?;
    save_persona_profile(data_dir, id, &input.profile_json)?;
    let meta = PersonaMeta {
        id: id.to_string(),
        name: name.to_string(),
        source: input.source.trim().to_string(),
        description: input.description.trim().to_string(),
    };
    upsert_manifest_entry(data_dir, &meta)
}

/// 删除用户自定义人设（内置柴郡等不可删）
pub fn delete_persona(
    data_dir: &Path,
    db: &rusqlite::Connection,
    id: &str,
) -> Result<(), String> {
    if is_builtin_persona(id) {
        return Err("内置人设不可删除".into());
    }

    let pre = load_manifest(data_dir);
    if !pre.personas.iter().any(|p| p.id == id) {
        return Err(format!("未知人设: {id}"));
    }
    if pre.personas.len() <= 1 {
        return Err("至少需要保留一个人设".into());
    }
    let was_active = active_persona_id(db, &pre) == id;

    mutate_persona_manifest(data_dir, |manifest| {
        manifest.personas.retain(|p| p.id != id);
        Ok(())
    })?;

    let manifest = load_manifest(data_dir);
    let dir = personas_dir(data_dir);
    let _ = fs::remove_file(dir.join(format!("{id}.md")));
    let _ = fs::remove_file(dir.join(format!("{id}.json")));

    if was_active {
        let fallback = if manifest.personas.iter().any(|p| p.id == manifest.default_id) {
            manifest.default_id.clone()
        } else {
            manifest.personas[0].id.clone()
        };
        set_active_persona_id(db, &manifest, &fallback)?;
    }

    crate::character::purge_character_for_persona(data_dir, db, id)?;

    Ok(())
}

/// 加载当前人设全文，作为 system prompt
pub fn system_prompt(data_dir: &Path, db: &rusqlite::Connection) -> String {
    let manifest = load_manifest(data_dir);
    let id = active_persona_id(db, &manifest);
    load_persona_body(data_dir, &id).unwrap_or_else(|| {
        load_persona_body(data_dir, &manifest.default_id).unwrap_or_default()
    })
}

fn load_persona_body(data_dir: &Path, id: &str) -> Option<String> {
    let user_path = personas_dir(data_dir).join(format!("{id}.md"));
    if let Ok(s) = fs::read_to_string(&user_path) {
        let t = s.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    EMBEDDED_PERSONAS
        .iter()
        .find(|(pid, _)| *pid == id)
        .map(|(_, c)| c.trim().to_string())
}

pub fn load_persona_profile(data_dir: &Path, id: &str) -> Option<CharacterProfileData> {
    let user_path = personas_dir(data_dir).join(format!("{id}.json"));
    if let Ok(raw) = fs::read_to_string(&user_path) {
        if let Ok(p) = serde_json::from_str(&raw) {
            return Some(p);
        }
    }
    EMBEDDED_PROFILES
        .iter()
        .find(|(pid, _)| *pid == id)
        .and_then(|(_, raw)| serde_json::from_str(raw).ok())
}

pub fn save_persona_profile(data_dir: &Path, id: &str, profile: &CharacterProfileData) -> Result<(), String> {
    let dir = personas_dir(data_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{id}.json")), json).map_err(|e| e.to_string())
}

fn slugify_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("无效人设 ID".into());
    }
    let mut id: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == '-' || c == '_' {
                c
            } else if c.is_whitespace() || c == '.' {
                '-'
            } else {
                '_'
            }
        })
        .collect();
    id = id.trim_matches('-').trim_matches('_').to_string();
    if id.is_empty() {
        id = fallback_persona_id(trimmed);
    } else if id.len() > 64 {
        id = id.chars().take(64).collect();
    }
    if id.is_empty() {
        return Err(format!("无效人设 ID: {raw}"));
    }
    Ok(id)
}

fn fallback_persona_id(raw: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("p{:08x}", hasher.finish() as u32)
}

/// 根据显示名生成唯一人设 ID（Wiki 导入等场景）
pub fn suggest_persona_id(data_dir: &Path, name: &str) -> Result<String, String> {
    let base = slugify_id(name)?;
    let manifest = load_manifest(data_dir);
    if !manifest.personas.iter().any(|p| p.id == base) {
        return Ok(base);
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !manifest.personas.iter().any(|p| p.id == candidate) {
            return Ok(candidate);
        }
        n += 1;
        if n > 99 {
            return Err(format!("无法为人设「{name}」生成唯一 ID"));
        }
    }
}

fn id_from_filename(filename: &str) -> Result<String, String> {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("无效文件名: {filename}"))?;
    slugify_id(stem)
}

#[derive(Debug, Clone)]
pub struct PersonaReferenceImportArgs {
    pub id: String,
    pub name_hint: Option<String>,
    pub source_hint: Option<String>,
    pub is_update: bool,
}

pub fn resolve_reference_import(
    data_dir: &Path,
    persona_id: Option<&str>,
    new_id: Option<&str>,
    name: Option<&str>,
    source: Option<&str>,
) -> Result<PersonaReferenceImportArgs, String> {
    let manifest = load_manifest(data_dir);
    if let Some(pid) = persona_id {
        if !manifest.personas.iter().any(|p| p.id == pid) {
            return Err(format!("未知人设: {pid}"));
        }
        let existing = manifest.personas.iter().find(|p| p.id == pid);
        Ok(PersonaReferenceImportArgs {
            id: pid.to_string(),
            name_hint: name
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .or_else(|| existing.map(|p| p.name.clone())),
            source_hint: source
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| existing.map(|p| p.source.clone())),
            is_update: true,
        })
    } else {
        let raw = new_id.ok_or("请填写人设 ID")?.trim();
        if raw.is_empty() {
            return Err("请填写人设 ID".into());
        }
        let id = slugify_id(raw)?;
        if manifest.personas.iter().any(|p| p.id == id) {
            return Err(format!("人设 ID「{id}」已存在"));
        }
        Ok(PersonaReferenceImportArgs {
            id,
            name_hint: name
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string),
            source_hint: source
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            is_update: false,
        })
    }
}

pub fn parse_import_files(files: Vec<PersonaImportFile>) -> Result<Vec<(String, String)>, String> {
    if files.is_empty() {
        return Err("请选择至少一个 .txt 或 .md 文本文件".into());
    }

    use std::collections::HashMap;

    let mut pending: HashMap<String, String> = HashMap::new();

    for file in files {
        let filename = file.filename.trim();
        if filename.is_empty() {
            continue;
        }
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let id = id_from_filename(filename)?;
        match ext.as_str() {
            "md" | "txt" => {
                let body = file.content.trim();
                if body.is_empty() {
                    return Err(format!("{filename} 内容为空"));
                }
                pending.insert(id, body.to_string());
            }
            _ => {
                return Err(format!(
                    "不支持的文件类型: {filename}（仅 .txt / .md 参考文本）"
                ));
            }
        }
    }

    if pending.is_empty() {
        return Err("没有可导入的参考文本".into());
    }

    let mut items: Vec<(String, String)> = pending.into_iter().collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn manifest_parses() {
        let m: PersonaManifest = serde_json::from_str(EMBEDDED_MANIFEST).unwrap();
        assert_eq!(m.personas.len(), 5);
        assert!(m.personas.iter().any(|p| p.id == "cheshire"));
        assert!(m.personas.iter().any(|p| p.id == "edu"));
    }

    #[test]
    fn all_builtin_personas_embedded() {
        assert_eq!(EMBEDDED_PERSONAS.len(), 5);
        assert_eq!(EMBEDDED_PROFILES.len(), 5);
        for id in ["cheshire", "edu", "wushiling", "qiye", "tashigan"] {
            assert!(is_builtin_persona(id));
        }
    }

    #[test]
    fn seed_and_load() {
        let base = env::temp_dir().join(format!("xiaohan-persona-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        seed_user_personas(&base).unwrap();
        assert!(manifest_path(&base).exists());
        let body = load_persona_body(&base, "cheshire").unwrap();
        assert!(body.contains("柴郡"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn parse_import_files_groups_by_stem() {
        let items = parse_import_files(vec![
            PersonaImportFile {
                filename: "custom.txt".into(),
                content: "测试角色\n\n性格温柔。".into(),
            },
            PersonaImportFile {
                filename: "other.md".into(),
                content: "# 其它\n\n内容".into(),
            },
        ])
        .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, "custom");
        assert_eq!(items[1].0, "other");
    }

    #[test]
    fn resolve_reference_import_create_and_update() {
        let base = env::temp_dir().join(format!("xiaohan-persona-resolve-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        seed_user_personas(&base).unwrap();

        let created = resolve_reference_import(&base, None, Some("nova"), Some("新星"), None).unwrap();
        assert_eq!(created.id, "nova");
        assert!(!created.is_update);
        assert_eq!(created.name_hint.as_deref(), Some("新星"));

        upsert_manifest_entry(
            &base,
            &PersonaMeta {
                id: created.id.clone(),
                name: created.name_hint.clone().unwrap_or_default(),
                source: String::new(),
                description: String::new(),
            },
        )
        .unwrap();

        let err = resolve_reference_import(&base, None, Some("nova"), None, None).unwrap_err();
        assert!(err.contains("已存在"));

        let updated = resolve_reference_import(&base, Some("cheshire"), None, None, None).unwrap();
        assert!(updated.is_update);
        assert_eq!(updated.id, "cheshire");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn update_persona_meta_and_files() {
        let base = env::temp_dir().join(format!("xiaohan-persona-update-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        seed_user_personas(&base).unwrap();

        let input = PersonaUpdateInput {
            name: "柴郡改".into(),
            source: "测试".into(),
            description: "新简介".into(),
            skill_md: "# 柴郡改\n\n新 skill".into(),
            profile_json: CharacterProfileData {
                name: "柴郡改".into(),
                introduction: "新介绍".into(),
                ..Default::default()
            },
        };
        update_persona(&base, "cheshire", &input).unwrap();
        let manifest = load_manifest(&base);
        let meta = manifest.personas.iter().find(|p| p.id == "cheshire").unwrap();
        assert_eq!(meta.name, "柴郡改");
        assert!(load_persona_body(&base, "cheshire").unwrap().contains("新 skill"));
        let _ = fs::remove_dir_all(&base);
    }
}
