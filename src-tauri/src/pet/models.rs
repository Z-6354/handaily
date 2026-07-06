//! 桌宠模型：内置 + 用户导入（Spine skel/atlas/png）

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};

pub const BUILTIN_CHAIJUN: &str = "chaijun";

struct BuiltinModelDef {
    id: &'static str,
    name: &'static str,
    skel: &'static str,
    atlas: &'static str,
    png: &'static str,
    meta_json: &'static str,
}

const BUILTIN_MODELS: &[BuiltinModelDef] = &[
    BuiltinModelDef {
        id: "chaijun",
        name: "柴郡",
        skel: "chaijun.skel",
        atlas: "chaijun.atlas",
        png: "chaijun.png",
        meta_json: include_str!("../../../public/assets/pet/chaijun/animations.meta.json"),
    },
    BuiltinModelDef {
        id: "edu",
        name: "恶毒",
        skel: "edu_3.skel",
        atlas: "edu_3.atlas",
        png: "edu_3.png",
        meta_json: include_str!("../../../public/assets/pet/edu/animations.meta.json"),
    },
    BuiltinModelDef {
        id: "wushiling",
        name: "五十铃",
        skel: "wushiling.skel",
        atlas: "wushiling.atlas",
        png: "wushiling.png",
        meta_json: include_str!("../../../public/assets/pet/wushiling/animations.meta.json"),
    },
    BuiltinModelDef {
        id: "qiye",
        name: "企业",
        skel: "qiye.skel",
        atlas: "qiye.atlas",
        png: "qiye.png",
        meta_json: include_str!("../../../public/assets/pet/qiye/animations.meta.json"),
    },
    BuiltinModelDef {
        id: "tashigan",
        name: "塔什干",
        skel: "tashigan.skel",
        atlas: "tashigan.atlas",
        png: "tashigan.png",
        meta_json: include_str!("../../../public/assets/pet/tashigan/animations.meta.json"),
    },
];

/// 旧版 Wiki 导入哈希 ID → 内置 slug
const LEGACY_BUILTIN_IDS: &[(&str, &str)] = &[
    ("m951a05aa", "edu"),
    ("ma19bdb1b", "wushiling"),
    ("mc5623cfa", "qiye"),
    ("mea9d211a", "tashigan"),
];

fn find_builtin(id: &str) -> Option<&'static BuiltinModelDef> {
    BUILTIN_MODELS.iter().find(|m| m.id == id)
}

fn is_builtin_id(id: &str) -> bool {
    find_builtin(id).is_some()
}

fn legacy_builtin_slug(id: &str) -> Option<&'static str> {
    LEGACY_BUILTIN_IDS
        .iter()
        .find(|(legacy, _)| *legacy == id)
        .map(|(_, slug)| *slug)
}

fn resolve_builtin_id(id: &str) -> &str {
    legacy_builtin_slug(id).unwrap_or(id)
}

fn is_legacy_or_builtin_id(id: &str) -> bool {
    is_builtin_id(id) || legacy_builtin_slug(id).is_some()
}

#[derive(Debug, Clone, Serialize)]
pub struct PetModelInfo {
    pub id: String,
    pub name: String,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PetModelAssets {
    pub model_id: String,
    pub model_name: String,
    /// 内置：`/assets/pet/{id}/`；用户：`绝对路径目录`（前端 convertFileSrc）
    pub asset_base: String,
    /// Live2DViewerEX Spine 配置（.config.json）；有则前端优先解析
    pub config_file: Option<String>,
    pub skel_file: String,
    pub atlas_file: String,
    pub png_file: String,
    pub use_file_src: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetRemarkLine {
    pub text: String,
    #[serde(default)]
    pub animation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetAnimationMeta {
    #[serde(default)]
    pub animations: Vec<String>,
    #[serde(default)]
    pub idle_animation: Option<String>,
    #[serde(default)]
    pub click_animation: Option<String>,
    #[serde(default)]
    pub boot_animation: Option<String>,
   #[serde(default)]
   pub return_idle_animation: Option<String>,
   #[serde(default)]
   pub drag_animation: Option<String>,
   #[serde(default)]
   pub random_animations: Vec<String>,
    #[serde(default = "default_random_min_sec")]
    pub random_min_sec: i64,
    #[serde(default = "default_random_max_sec")]
    pub random_max_sec: i64,
    #[serde(default)]
    pub lines: Vec<PetRemarkLine>,
    #[serde(default)]
    pub power_mode: Option<String>,
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub remark_interval_sec: Option<i64>,
}

impl Default for PetAnimationMeta {
    fn default() -> Self {
        Self {
            animations: Vec::new(),
            idle_animation: None,
            click_animation: None,
            boot_animation: None,
       return_idle_animation: None,
       drag_animation: None,
       random_animations: Vec::new(),
            random_min_sec: default_random_min_sec(),
            random_max_sec: default_random_max_sec(),
            lines: Vec::new(),
            power_mode: None,
            scale: None,
            remark_interval_sec: None,
        }
    }
}

fn default_random_min_sec() -> i64 {
    30
}

fn default_random_max_sec() -> i64 {
    120
}

#[derive(Debug, Deserialize)]
pub struct PetSyncAnimationsPayload {
    pub model_id: String,
    pub animations: Vec<String>,
    pub idle_animation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PetImportFilesPayload {
    pub name: String,
    pub skel_b64: String,
    pub atlas_b64: String,
    pub png_b64: String,
}

#[derive(Debug, Deserialize)]
pub struct PetStageFilesPayload {
    pub skel_b64: String,
    pub atlas_b64: String,
    pub png_b64: String,
    /// 原始文件名（保留 atlas 内纹理引用一致）
    pub skel_name: Option<String>,
    pub atlas_name: Option<String>,
    pub png_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetImportStagingPreview {
    pub source: String,
    pub folder_path: Option<String>,
    pub skel_file: String,
    pub atlas_file: String,
    pub png_file: String,
    pub config_file: Option<String>,
    pub config_generated: bool,
}

fn staging_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("pet-import-staging")
}

pub fn clear_import_staging(data_dir: &Path) -> Result<(), String> {
    let dir = staging_dir(data_dir);
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn get_import_staging(data_dir: &Path) -> Result<Option<PetImportStagingPreview>, String> {
    let dir = staging_dir(data_dir);
    if !dir.is_dir() {
        return Ok(None);
    }
    Ok(Some(read_staging_preview(&dir)?))
}

fn read_staging_preview(dir: &Path) -> Result<PetImportStagingPreview, String> {
    let manifest_path = dir.join("manifest.json");
    if manifest_path.is_file() {
        let raw = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
        return serde_json::from_str(&raw).map_err(|e| e.to_string());
    }
    let triple = resolve_user_dir(dir)?;
    let config_file = find_config_file(dir);
    Ok(PetImportStagingPreview {
        source: "unknown".into(),
        folder_path: None,
        skel_file: triple.0,
        atlas_file: triple.1,
        png_file: triple.2,
        config_file,
        config_generated: false,
    })
}

fn write_staging_manifest(dir: &Path, preview: &PetImportStagingPreview) -> Result<(), String> {
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(preview).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn stage_from_folder(data_dir: &Path, folder: &Path) -> Result<PetImportStagingPreview, String> {
    if !folder.is_dir() {
        return Err(format!("文件夹不存在: {}", folder.display()));
    }
    clear_import_staging(data_dir)?;
    let triple = inspect_spine_folder(folder)?;
    let config_generated = find_config_file(folder).is_none();

    let dest = staging_dir(data_dir);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    copy_model_files(folder, &dest, &triple)?;

    let config_file = find_config_file(&dest);
    let preview = PetImportStagingPreview {
        source: "folder".into(),
        folder_path: Some(folder.to_string_lossy().to_string()),
        skel_file: triple.0,
        atlas_file: triple.1,
        png_file: triple.2,
        config_file,
        config_generated,
    };
    write_staging_manifest(&dest, &preview)?;
    Ok(preview)
}

pub fn stage_from_files(data_dir: &Path, payload: &PetStageFilesPayload) -> Result<PetImportStagingPreview, String> {
    clear_import_staging(data_dir)?;
    let dest = staging_dir(data_dir);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    let skel = sanitize_spine_filename(
        payload.skel_name.as_deref().unwrap_or("model.skel"),
        "skel",
    );
    let atlas = sanitize_spine_filename(
        payload.atlas_name.as_deref().unwrap_or("model.atlas"),
        "atlas",
    );
    let png = sanitize_spine_filename(payload.png_name.as_deref().unwrap_or("model.png"), "png");

    let skel = decode_and_write(&dest, &skel, &payload.skel_b64)?;
    let atlas = decode_and_write(&dest, &atlas, &payload.atlas_b64)?;
    let png = decode_and_write(&dest, &png, &payload.png_b64)?;
    let triple = SpineTriple(skel.clone(), atlas.clone(), png.clone());
    sync_model_package(&dest, &triple)?;

    let preview = PetImportStagingPreview {
        source: "files".into(),
        folder_path: None,
        skel_file: skel,
        atlas_file: atlas,
        png_file: png,
        config_file: Some("config.json".into()),
        config_generated: true,
    };
    write_staging_manifest(&dest, &preview)?;
    Ok(preview)
}

pub fn commit_staged_import(data_dir: &Path, name: &str) -> Result<PetModelInfo, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请填写模型名称".into());
    }
    let staging = staging_dir(data_dir);
    if !staging.is_dir() {
        return Err("请先选择文件夹或文件并完成缓存".into());
    }
    let triple = resolve_user_dir(&staging)?;
    let id = unique_id(data_dir, &slugify_id(name)?);
    let dest = models_dir(data_dir).join(&id);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    copy_model_files(&staging, &dest, &triple)?;
    write_model_name(&dest, name)?;
    clear_import_staging(data_dir)?;
    Ok(PetModelInfo {
        id,
        name: name.to_string(),
        builtin: false,
    })
}

fn validate_model_filename(filename: &str) -> Result<&str, String> {
    let name = filename.trim();
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err("非法文件名".into());
    }
    Ok(name)
}

/// 读取用户模型资源文件（base64），供桌宠前端加载。
pub fn read_model_asset_b64(data_dir: &Path, model_id: &str, filename: &str) -> Result<String, String> {
    let name = validate_model_filename(filename)?;
    let assets = resolve_assets(data_dir, model_id)?;
    if !assets.use_file_src {
        return Err("内置模型请使用静态资源路径".into());
    }
    let path = models_dir(data_dir).join(model_id).join(name);
    if !path.is_file() {
        return Err(format!("文件不存在: {name}"));
    }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(data))
}

#[derive(Debug, Clone, Serialize)]
pub struct PetModelAssetBundle {
    pub files: std::collections::HashMap<String, String>,
}

/// 一次读取多个模型文件，减少 IPC 往返。
pub fn read_model_asset_bundle(
    data_dir: &Path,
    model_id: &str,
    filenames: &[String],
) -> Result<PetModelAssetBundle, String> {
    let assets = resolve_assets(data_dir, model_id)?;
    if !assets.use_file_src {
        return Err("内置模型请使用静态资源路径".into());
    }
    let dir = models_dir(data_dir).join(model_id);
    let mut files = std::collections::HashMap::new();
    for filename in filenames {
        let name = validate_model_filename(filename)?;
        let path = dir.join(name);
        if !path.is_file() {
            return Err(format!("文件不存在: {name}"));
        }
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
        files.insert(name.to_string(), b64);
    }
    Ok(PetModelAssetBundle { files })
}

pub fn models_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("pet-models")
}

pub fn active_model_id(db: &rusqlite::Connection) -> String {
    let id = crate::db::get_setting(db, "pet_model_id")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| BUILTIN_CHAIJUN.to_string());
    resolve_builtin_id(&id).to_string()
}

pub fn set_active_model_id(db: &rusqlite::Connection, id: &str) -> Result<(), String> {
    crate::db::set_setting(db, "pet_model_id", id).map_err(|e| e.to_string())
}

pub fn list_models(data_dir: &Path) -> Result<Vec<PetModelInfo>, String> {
    let mut out: Vec<PetModelInfo> = BUILTIN_MODELS
        .iter()
        .map(|m| PetModelInfo {
            id: m.id.into(),
            name: m.name.into(),
            builtin: true,
        })
        .collect();

    let dir = models_dir(data_dir);
    if dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let id = entry.file_name().to_string_lossy().to_string();
            if is_legacy_or_builtin_id(&id) {
                continue;
            }
            if resolve_user_dir(&entry.path()).is_ok() {
                let name = read_model_name(&entry.path()).unwrap_or_else(|| id.clone());
                out.push(PetModelInfo {
                    id,
                    name,
                    builtin: false,
                });
            }
        }
    }
    Ok(out)
}

pub fn resolve_assets(data_dir: &Path, model_id: &str) -> Result<PetModelAssets, String> {
    let resolved = resolve_builtin_id(model_id);
    if let Some(m) = find_builtin(resolved) {
        return Ok(PetModelAssets {
            model_id: m.id.into(),
            model_name: m.name.into(),
            asset_base: format!("/assets/pet/{}/", m.id),
            config_file: Some("config.json".into()),
            skel_file: m.skel.into(),
            atlas_file: m.atlas.into(),
            png_file: m.png.into(),
            use_file_src: false,
        });
    }

    let dir = models_dir(data_dir).join(model_id);
    let triple = resolve_user_dir(&dir)?;
    let config_file = find_config_file(&dir);
    let name = read_model_name(&dir).unwrap_or_else(|| model_id.to_string());
    Ok(PetModelAssets {
        model_id: model_id.to_string(),
        model_name: name,
        asset_base: dir.to_string_lossy().replace('\\', "/"),
        config_file,
        skel_file: triple.0,
        atlas_file: triple.1,
        png_file: triple.2,
        use_file_src: true,
    })
}

struct SpineTriple(String, String, String);

fn try_triple_from_config(dir: &Path) -> Option<SpineTriple> {
    let cfg_name = find_config_file(dir)?;
    let raw = fs::read_to_string(dir.join(&cfg_name)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    if !is_viewer_ex_spine_config(&v) {
        return None;
    }
    let skel = v.get("skeleton")?.as_str()?.trim();
    let atlas_entry = v.get("atlases")?.as_array()?.first()?;
    let atlas = atlas_entry.get("atlas")?.as_str()?.trim();
    let png = atlas_entry
        .get("textures")?
        .as_array()?
        .first()?
        .as_str()?
        .trim();
    if dir.join(skel).is_file() && dir.join(atlas).is_file() && dir.join(png).is_file() {
        Some(SpineTriple(
            skel.to_string(),
            atlas.to_string(),
            png.to_string(),
        ))
    } else {
        None
    }
}

fn inspect_spine_folder(dir: &Path) -> Result<SpineTriple, String> {
    if !dir.is_dir() {
        return Err(format!("文件夹不存在: {}", dir.display()));
    }
    if let Some(triple) = try_triple_from_config(dir) {
        return Ok(triple);
    }
    let skel = find_file_with_ext_optional(dir, "skel");
    let atlas = find_file_with_ext_optional(dir, "atlas");
    let png = find_file_with_ext_optional(dir, "png");
    if skel.is_none() || atlas.is_none() || png.is_none() {
        let mut missing = Vec::new();
        if skel.is_none() {
            missing.push(".skel");
        }
        if atlas.is_none() {
            missing.push(".atlas");
        }
        if png.is_none() {
            missing.push(".png");
        }
        return Err(format!(
            "该文件夹不是有效的 Spine 模型：缺少 {}（需要 .skel、.atlas、.png 三件套齐全）",
            missing.join("、")
        ));
    }
    Ok(SpineTriple(skel.unwrap(), atlas.unwrap(), png.unwrap()))
}

fn resolve_user_dir(dir: &Path) -> Result<SpineTriple, String> {
    inspect_spine_folder(dir)
}

fn find_file_with_ext_optional(dir: &Path, ext: &str) -> Option<String> {
    fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        if !path.is_file() {
            return None;
        }
        path.extension()
            .and_then(|e| e.to_str())
            .filter(|e| e.eq_ignore_ascii_case(ext))?;
        Some(path.file_name()?.to_str()?.to_string())
    })
}

fn file_stem(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("texture")
        .to_string()
}

fn generate_viewer_ex_config(skel: &str, atlas: &str, png: &str) -> serde_json::Value {
    let tex_name = file_stem(png);
    serde_json::json!({
        "conf_ver": 1,
        "type": 9,
        "options": {
            "tex_type": 0,
            "edge_padding": false
        },
        "skeleton": skel,
        "atlases": [{
            "atlas": atlas,
            "tex_names": [tex_name],
            "textures": [png]
        }]
    })
}

fn sanitize_spine_filename(name: &str, ext: &str) -> String {
    let path = Path::new(name.trim());
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("model");
    let mut safe = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            safe.push(c);
        } else if c.is_whitespace() {
            safe.push('_');
        }
    }
    if safe.is_empty() {
        safe = "model".into();
    }
    format!("{safe}.{ext}")
}

fn read_atlas_texture_page(atlas_path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(atlas_path).map_err(|e| e.to_string())?;
    for line in text.lines() {
        let line = line.trim();
        if !line.is_empty() && !line.contains(':') {
            return Ok(line.to_string());
        }
    }
    Err(format!("atlas 缺少纹理页名称: {}", atlas_path.display()))
}

fn rewrite_atlas_texture_page(atlas_path: &Path, png_name: &str) -> Result<(), String> {
    let text = fs::read_to_string(atlas_path).map_err(|e| e.to_string())?;
    let mut replaced = false;
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if !replaced {
            let t = line.trim();
            if !t.is_empty() && !t.contains(':') {
                lines.push(png_name.to_string());
                replaced = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }
    if !replaced {
        return Err(format!("无法修正 atlas 纹理页: {}", atlas_path.display()));
    }
    fs::write(atlas_path, lines.join("\n")).map_err(|e| e.to_string())
}

fn config_matches_triple(dir: &Path, triple: &SpineTriple) -> bool {
    let Some(cfg_name) = find_config_file(dir) else {
        return false;
    };
    let Ok(raw) = fs::read_to_string(dir.join(cfg_name)) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    if !is_viewer_ex_spine_config(&v) {
        return false;
    }
    let skel = v.get("skeleton").and_then(|v| v.as_str()).unwrap_or("");
    let atlas0 = v
        .get("atlases")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first());
    let atlas = atlas0
        .and_then(|a| a.get("atlas"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let png = atlas0
        .and_then(|a| a.get("textures"))
        .and_then(|t| t.as_array())
        .and_then(|t| t.first())
        .and_then(|v| v.as_str())
        .unwrap_or("");
    skel == triple.0 && atlas == triple.1 && png == triple.2
}

fn sync_model_package(dir: &Path, triple: &SpineTriple) -> Result<String, String> {
    let atlas_path = dir.join(&triple.1);
    if atlas_path.is_file() {
        if let Ok(page) = read_atlas_texture_page(&atlas_path) {
            if page != triple.2 {
                rewrite_atlas_texture_page(&atlas_path, &triple.2)?;
            }
        }
    }

    if !config_matches_triple(dir, triple) {
        for name in ["config.json", ".config.json"] {
            let p = dir.join(name);
            if p.is_file() {
                let _ = fs::remove_file(p);
            }
        }
    }

    let cfg = "config.json";
    if !dir.join(cfg).is_file() {
        let json = generate_viewer_ex_config(&triple.0, &triple.1, &triple.2);
        fs::write(
            dir.join(cfg),
            serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(cfg.to_string())
}

fn meta_db_key(model_id: &str) -> String {
    format!("pet_anim_meta_{model_id}")
}

/// 启动时将旧版导入哈希 ID 迁移为内置 slug（pet_model_id 与台词/动作 meta）
pub fn migrate_legacy_builtin_models(db: &rusqlite::Connection) -> Result<(), String> {
    if let Some(active) = crate::db::get_setting(db, "pet_model_id") {
        if let Some(slug) = legacy_builtin_slug(&active) {
            set_active_model_id(db, slug)?;
        }
    }
    for (legacy, slug) in LEGACY_BUILTIN_IDS {
        let legacy_key = meta_db_key(legacy);
        if let Some(raw) = crate::db::get_setting(db, &legacy_key) {
            if !raw.trim().is_empty() {
                let new_key = meta_db_key(slug);
                if crate::db::get_setting(db, &new_key).is_none() {
                    crate::db::set_setting(db, &new_key, &raw).map_err(|e| e.to_string())?;
                }
                crate::db::set_setting(db, &legacy_key, "").map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

const ANIM_META_FILENAME: &str = "animations.meta.json";

/// 导入新模型时使用的默认动作/台词模板（与 config/pet-action-template.json 同步）
const IMPORT_ACTION_TEMPLATE: &str = include_str!("../../../config/pet-action-template.json");

fn read_import_action_template() -> PetAnimationMeta {
    serde_json::from_str(IMPORT_ACTION_TEMPLATE).unwrap_or_default()
}

fn match_animation_name(preferred: &str, names: &[String]) -> Option<String> {
    let pref = preferred.trim();
    if pref.is_empty() {
        return None;
    }
    if names.is_empty() {
        return Some(pref.to_string());
    }
    if names.iter().any(|n| n == pref) {
        return Some(pref.to_string());
    }
    names
        .iter()
        .find(|n| n.eq_ignore_ascii_case(pref))
        .cloned()
}

fn match_animation_keyword(keywords: &[&str], names: &[String]) -> Option<String> {
    for name in names {
        let lower = name.to_ascii_lowercase();
        if keywords.iter().any(|key| lower.contains(key)) {
            return Some(name.clone());
        }
    }
    None
}

fn resolve_role_animation(
    template_val: &Option<String>,
    names: &[String],
    keywords: &[&str],
    detect: fn(&[String]) -> Option<String>,
) -> Option<String> {
    if let Some(pref) = template_val.as_ref().filter(|s| !s.trim().is_empty()) {
        if let Some(matched) = match_animation_name(pref, names) {
            return Some(matched);
        }
        let lower = pref.to_ascii_lowercase();
        let mut keys: Vec<&str> = keywords.to_vec();
        if !keys.iter().any(|k| *k == lower.as_str()) {
            keys.push(lower.as_str());
        }
        if let Some(matched) = match_animation_keyword(&keys, names) {
            return Some(matched);
        }
    }
    detect(names)
}

fn apply_template_to_meta(meta: &mut PetAnimationMeta, template: &PetAnimationMeta) {
    let names = meta.animations.clone();
    meta.idle_animation = resolve_role_animation(
        &template.idle_animation,
        &names,
        &["normal", "idle", "stand", "standby", "default"],
        detect_idle_animation,
    );
    normalize_idle_animation(meta);
    meta.click_animation = resolve_role_animation(
        &template.click_animation,
        &names,
        &["touch", "tap", "click", "hit"],
        detect_click_animation,
    );
    meta.boot_animation = resolve_role_animation(
        &template.boot_animation,
        &names,
        &["normal", "idle", "stand", "start", "boot"],
        detect_idle_animation,
    );
    meta.return_idle_animation = resolve_role_animation(
        &template.return_idle_animation,
        &names,
        &["normal", "idle", "stand", "standby", "default"],
        detect_idle_animation,
    );
    meta.drag_animation = resolve_role_animation(
        &template.drag_animation,
        &names,
        &["tuozhuai", "drag"],
        detect_drag_animation,
    );

    let idle = meta.idle_animation.as_deref().unwrap_or("");
    let mut random: Vec<String> = template
        .random_animations
        .iter()
        .filter_map(|name| {
            match_animation_name(name, &names).or_else(|| {
                match_animation_keyword(&[name.as_str()], &names)
            })
        })
        .filter(|n| n.as_str() != idle)
        .collect();
    random.sort();
    random.dedup();
    if random.is_empty() && !names.is_empty() {
        random = names
            .iter()
            .filter(|n| {
                n.as_str() != idle
                    && meta.click_animation.as_deref() != Some(n.as_str())
                    && meta.boot_animation.as_deref() != Some(n.as_str())
                    && meta.return_idle_animation.as_deref() != Some(n.as_str())
                    && meta.drag_animation.as_deref() != Some(n.as_str())
                    && !is_likely_idle_name(n)
            })
            .cloned()
            .collect();
    }
    meta.random_animations = random;

    meta.random_min_sec = template.random_min_sec;
    meta.random_max_sec = template.random_max_sec;
    meta.lines = template
        .lines
        .iter()
        .map(|line| PetRemarkLine {
            text: line.text.clone(),
            animation: line
                .animation
                .as_ref()
                .and_then(|anim| match_animation_name(anim, &names)),
        })
        .filter(|line| !line.text.trim().is_empty())
        .collect();
    normalize_idle_animation(meta);
}

fn is_likely_idle_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if ["normal", "stand", "idle", "standby", "default"]
        .iter()
        .any(|k| lower.as_str() == *k)
    {
        return true;
    }
    ["idle", "stand", "normal"]
        .iter()
        .any(|k| lower.contains(k))
}

/// 模型导入后立即写入默认动作模板（动作名在首次 sync 时按模型实际列表解析）
pub fn apply_import_action_template(
    data_dir: &Path,
    db: &rusqlite::Connection,
    model_id: &str,
) -> Result<(), String> {
    if is_builtin_id(resolve_builtin_id(model_id)) {
        return Ok(());
    }
    let mut meta = read_import_action_template();
    meta.animations = Vec::new();
    save_animation_meta(data_dir, db, model_id, &meta)
}

fn meta_persist_dir(data_dir: &Path, model_id: &str) -> PathBuf {
    data_dir.join("pet-meta").join(model_id)
}

fn read_bundled_animation_meta(model_id: &str) -> Option<PetAnimationMeta> {
    let resolved = resolve_builtin_id(model_id);
    let m = find_builtin(resolved)?;
    serde_json::from_str(m.meta_json).ok()
}

fn merge_meta_defaults(meta: &mut PetAnimationMeta, defaults: &PetAnimationMeta) {
    if meta.idle_animation.is_none() {
        meta.idle_animation = defaults.idle_animation.clone();
    }
    if meta.click_animation.is_none() {
        meta.click_animation = defaults.click_animation.clone();
    }
    if meta.boot_animation.is_none() {
        meta.boot_animation = defaults.boot_animation.clone();
    }
   if meta.return_idle_animation.is_none() {
       meta.return_idle_animation = defaults.return_idle_animation.clone();
   }
   if meta.drag_animation.is_none() {
       meta.drag_animation = defaults.drag_animation.clone();
   }
   if meta.random_animations.is_empty() && !defaults.random_animations.is_empty() {
        meta.random_animations = defaults.random_animations.clone();
    }
    if meta.lines.is_empty() && !defaults.lines.is_empty() {
        meta.lines = defaults.lines.clone();
    }
    if meta.random_min_sec == default_random_min_sec()
        && defaults.random_min_sec != default_random_min_sec()
    {
        meta.random_min_sec = defaults.random_min_sec;
    }
    if meta.random_max_sec == default_random_max_sec()
        && defaults.random_max_sec != default_random_max_sec()
    {
        meta.random_max_sec = defaults.random_max_sec;
    }
}

fn normalize_idle_animation(meta: &mut PetAnimationMeta) {
    if meta.idle_animation.is_none() {
        meta.idle_animation = detect_idle_animation(&meta.animations);
    }
    let idle = meta.idle_animation.clone();
    if meta.return_idle_animation.is_none() {
        meta.return_idle_animation = idle.clone();
    }
    if meta.boot_animation.is_none() {
        meta.boot_animation = idle.clone();
    }
}

fn ensure_click_animation(meta: &mut PetAnimationMeta, model_id: &str) {
    if meta.click_animation.is_some() {
        return;
    }
    meta.click_animation = detect_click_animation(&meta.animations);
    if meta.click_animation.is_none() {
        if let Some(pref) = read_import_action_template().click_animation {
            meta.click_animation = match_animation_name(&pref, &meta.animations);
        }
    }
    if meta.click_animation.is_none() {
        if let Some(defaults) = read_bundled_animation_meta(model_id) {
            meta.click_animation = defaults.click_animation.clone();
        }
    }
}

fn is_viewer_ex_spine_config(raw: &serde_json::Value) -> bool {
    raw.get("type").and_then(|t| t.as_i64()) == Some(9)
        && raw
            .get("skeleton")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.trim().is_empty())
}

fn meta_from_json_value(v: &serde_json::Value) -> PetAnimationMeta {
    let animations = v
        .get("animations")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let idle_animation = v
        .get("idle_animation")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    PetAnimationMeta {
        animations,
        idle_animation,
        click_animation: v
            .get("click_animation")
            .and_then(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        random_animations: v
            .get("random_animations")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        random_min_sec: v
            .get("random_min_sec")
            .and_then(|x| x.as_i64())
            .unwrap_or_else(default_random_min_sec),
        random_max_sec: v
            .get("random_max_sec")
            .and_then(|x| x.as_i64())
            .unwrap_or_else(default_random_max_sec),
        boot_animation: v
            .get("boot_animation")
            .and_then(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        return_idle_animation: v
            .get("return_idle_animation")
            .and_then(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        drag_animation: v
            .get("drag_animation")
            .and_then(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        lines: v
            .get("lines")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let text = item.get("text")?.as_str()?.trim();
                        if text.is_empty() {
                            return None;
                        }
                        let animation = item
                            .get("animation")
                            .and_then(|x| x.as_str())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty());
                        Some(PetRemarkLine {
                            text: text.to_string(),
                            animation,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        power_mode: v
            .get("power_mode")
            .and_then(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| matches!(s.as_str(), "minimal" | "balanced" | "full")),
        scale: v.get("scale").and_then(|x| x.as_f64()),
        remark_interval_sec: v.get("remark_interval_sec").and_then(|x| x.as_i64()),
    }
}

fn read_animation_meta_file(dir: &Path) -> Option<PetAnimationMeta> {
    let sidecar = dir.join(ANIM_META_FILENAME);
    if let Ok(raw) = fs::read_to_string(&sidecar) {
        if let Ok(meta) = serde_json::from_str::<PetAnimationMeta>(&raw) {
            return Some(meta);
        }
    }
    let model_path = dir.join("model.json");
    if let Ok(raw) = fs::read_to_string(&model_path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            if is_viewer_ex_spine_config(&v) {
                return None;
            }
            return Some(meta_from_json_value(&v));
        }
    }
    None
}

fn write_animation_meta_file(dir: &Path, meta: &PetAnimationMeta) -> Result<(), String> {
    fs::write(
        dir.join(ANIM_META_FILENAME),
        serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn read_model_json_file(dir: &Path) -> serde_json::Value {
    let path = dir.join("model.json");
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str(&raw) {
            return v;
        }
    }
    serde_json::json!({})
}

fn has_saved_animation_meta(
    db: &rusqlite::Connection,
    data_dir: &Path,
    model_id: &str,
) -> bool {
    if crate::db::get_setting(db, &meta_db_key(model_id)).is_some() {
        return true;
    }
    meta_persist_dir(data_dir, model_id)
        .join(ANIM_META_FILENAME)
        .is_file()
}

pub fn read_animation_meta(
    data_dir: &Path,
    db: &rusqlite::Connection,
    model_id: &str,
) -> PetAnimationMeta {
    if let Some(raw) = crate::db::get_setting(db, &meta_db_key(model_id)) {
        if let Ok(mut meta) = serde_json::from_str::<PetAnimationMeta>(&raw) {
            normalize_idle_animation(&mut meta);
            ensure_click_animation(&mut meta, model_id);
            return meta;
        }
    }
    let persist = meta_persist_dir(data_dir, model_id).join(ANIM_META_FILENAME);
    if persist.is_file() {
        if let Ok(raw) = fs::read_to_string(&persist) {
            if let Ok(mut meta) = serde_json::from_str::<PetAnimationMeta>(&raw) {
                normalize_idle_animation(&mut meta);
                ensure_click_animation(&mut meta, model_id);
                return meta;
            }
        }
    }
    if !is_builtin_id(resolve_builtin_id(model_id)) {
        let dir = models_dir(data_dir).join(model_id);
        if dir.is_dir() {
            if let Some(mut meta) = read_animation_meta_file(&dir) {
                normalize_idle_animation(&mut meta);
                ensure_click_animation(&mut meta, model_id);
                return meta;
            }
        }
    }
    if let Some(mut meta) = read_bundled_animation_meta(model_id) {
        normalize_idle_animation(&mut meta);
        return meta;
    }
    PetAnimationMeta::default()
}

pub fn save_animation_meta(
    data_dir: &Path,
    db: &rusqlite::Connection,
    model_id: &str,
    meta: &PetAnimationMeta,
) -> Result<(), String> {
    let json = serde_json::to_string(meta).map_err(|e| e.to_string())?;
    crate::db::set_setting(db, &meta_db_key(model_id), &json).map_err(|e| e.to_string())?;
    let persist_dir = meta_persist_dir(data_dir, model_id);
    fs::create_dir_all(&persist_dir).map_err(|e| e.to_string())?;
    fs::write(persist_dir.join(ANIM_META_FILENAME), &json).map_err(|e| e.to_string())?;
    if !is_builtin_id(resolve_builtin_id(model_id)) {
        let dir = models_dir(data_dir).join(model_id);
        if dir.is_dir() {
            write_animation_meta_file(&dir, meta)?;
        }
    }
    Ok(())
}

pub fn sync_animations(
    data_dir: &Path,
    db: &rusqlite::Connection,
    payload: &PetSyncAnimationsPayload,
) -> Result<PetAnimationMeta, String> {
    let has_saved = has_saved_animation_meta(db, data_dir, &payload.model_id);
    let mut meta = read_animation_meta(data_dir, db, &payload.model_id);
    let had_animation_list = !meta.animations.is_empty();
    let mut names: Vec<String> = payload
        .animations
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    names.sort();
    names.dedup();
    meta.animations = names;
    if let Some(idle) = payload.idle_animation.as_ref().map(|s| s.trim().to_string()) {
        if !idle.is_empty() && meta.animations.iter().any(|n| n == &idle) {
            meta.idle_animation = Some(idle);
        }
    }
    normalize_idle_animation(&mut meta);

    let needs_template_resolve =
        has_saved && !had_animation_list && !meta.animations.is_empty();

    if needs_template_resolve {
        apply_template_to_meta(&mut meta, &read_import_action_template());
    } else if !has_saved {
        if meta.boot_animation.is_none() {
            meta.boot_animation = meta.idle_animation.clone();
        }
        if meta.return_idle_animation.is_none() {
            meta.return_idle_animation = meta.idle_animation.clone();
        }
        if meta.click_animation.is_none() {
            meta.click_animation = detect_click_animation(&meta.animations);
        }
        if meta.drag_animation.is_none() {
            meta.drag_animation = detect_drag_animation(&meta.animations);
        }
        if let Some(defaults) = read_bundled_animation_meta(&payload.model_id) {
            merge_meta_defaults(&mut meta, &defaults);
        } else if !is_builtin_id(resolve_builtin_id(&payload.model_id)) {
            apply_template_to_meta(&mut meta, &read_import_action_template());
        }
    }
    prune_animation_refs(&mut meta);
    save_animation_meta(data_dir, db, &payload.model_id, &meta)?;
    Ok(meta)
}

fn prune_animation_refs(meta: &mut PetAnimationMeta) {
    meta.random_animations.retain(|n| meta.animations.contains(n));
   if meta
       .click_animation
       .as_ref()
       .is_some_and(|n| !meta.animations.contains(n))
   {
       meta.click_animation = detect_click_animation(&meta.animations);
   }
    if meta
        .boot_animation
        .as_ref()
        .is_some_and(|n| !meta.animations.contains(n))
    {
        meta.boot_animation = meta.idle_animation.clone();
    }
   if meta
       .return_idle_animation
       .as_ref()
       .is_some_and(|n| !meta.animations.contains(n))
   {
       meta.return_idle_animation = meta.idle_animation.clone();
   }
   if meta
       .drag_animation
       .as_ref()
       .is_some_and(|n| !meta.animations.contains(n))
   {
       meta.drag_animation = None;
   }
   if let Some(idle) = &meta.idle_animation {
       meta.random_animations.retain(|n| n != idle);
   }
    meta.lines.retain(|line| {
        line.text.trim().len() >= 1
            && line.animation.as_ref().is_none_or(|a| {
                meta.animations.is_empty() || meta.animations.iter().any(|n| n == a)
            })
    });
}

fn detect_idle_animation(names: &[String]) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    for pref in ["normal", "stand", "idle", "standby", "default"] {
        if let Some(n) = names
            .iter()
            .find(|n| n.eq_ignore_ascii_case(pref))
        {
            return Some(n.clone());
        }
    }
    for key in ["idle", "stand", "normal"] {
        if let Some(n) = names
            .iter()
            .find(|n| n.to_ascii_lowercase().contains(key))
        {
            return Some(n.clone());
        }
    }
    Some(names[0].clone())
}

fn detect_click_animation(names: &[String]) -> Option<String> {
    const KEYS: [&str; 4] = ["tap", "click", "touch", "hit"];
    names
        .iter()
        .find(|n| {
            let lower = n.to_ascii_lowercase();
            KEYS.iter().any(|k| lower.contains(k))
        })
        .cloned()
}

fn detect_drag_animation(names: &[String]) -> Option<String> {
    const KEYS: [&str; 3] = ["tuozhuai", "drag", "move"];
    names
        .iter()
        .find(|n| {
            let lower = n.to_ascii_lowercase();
            KEYS.iter().any(|k| lower.contains(k))
        })
        .cloned()
}

pub fn set_idle_animation(
    data_dir: &Path,
    db: &rusqlite::Connection,
    model_id: &str,
    idle: &str,
) -> Result<PetAnimationMeta, String> {
    let idle = idle.trim();
    if idle.is_empty() {
        return Err("待机动作不能为空".into());
    }
    let mut meta = read_animation_meta(data_dir, db, model_id);
    if !meta.animations.is_empty() && !meta.animations.iter().any(|n| n == idle) {
        return Err(format!("动作「{idle}」不在模型列表中"));
    }
    meta.idle_animation = Some(idle.to_string());
    prune_animation_refs(&mut meta);
    save_animation_meta(data_dir, db, model_id, &meta)?;
    Ok(meta)
}

pub fn set_click_animation(
    data_dir: &Path,
    db: &rusqlite::Connection,
    model_id: &str,
    click: &str,
) -> Result<PetAnimationMeta, String> {
    let click = click.trim();
    let mut meta = read_animation_meta(data_dir, db, model_id);
    if click.is_empty() {
        meta.click_animation = None;
    } else {
        if !meta.animations.is_empty() && !meta.animations.iter().any(|n| n == click) {
            return Err(format!("动作「{click}」不在模型列表中"));
        }
        meta.click_animation = Some(click.to_string());
    }
    save_animation_meta(data_dir, db, model_id, &meta)?;
    Ok(meta)
}

#[derive(Debug, Deserialize)]
pub struct PetRandomAnimationsPayload {
    pub model_id: String,
    pub animations: Vec<String>,
    pub min_sec: i64,
    pub max_sec: i64,
}

#[derive(Debug, Deserialize)]
pub struct PetAnimationLayoutPayload {
    pub model_id: String,
    pub idle_animation: Option<String>,
    pub click_animation: Option<String>,
    pub boot_animation: Option<String>,
   pub return_idle_animation: Option<String>,
   pub drag_animation: Option<String>,
   pub random_animations: Vec<String>,
   pub random_min_sec: i64,
    pub random_max_sec: i64,
    #[serde(default)]
    pub lines: Vec<PetRemarkLine>,
}

#[derive(Debug, Deserialize)]
pub struct PetImportLinesPayload {
    pub model_id: String,
    pub lines: Vec<PetRemarkLine>,
    #[serde(default)]
    pub append: bool,
}

pub fn set_random_animations(
    data_dir: &Path,
    db: &rusqlite::Connection,
    payload: &PetRandomAnimationsPayload,
) -> Result<PetAnimationMeta, String> {
    let mut meta = read_animation_meta(data_dir, db, &payload.model_id);
    let idle = meta.idle_animation.as_deref().unwrap_or("");
    let mut names: Vec<String> = payload
        .animations
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.as_str() != idle)
        .collect();
    names.retain(|n| meta.animations.is_empty() || meta.animations.iter().any(|a| a == n));
    names.sort();
    names.dedup();
    meta.random_animations = names;
    let min_sec = payload.min_sec.clamp(5, 3600);
    let max_sec = payload.max_sec.clamp(min_sec, 7200);
    meta.random_min_sec = min_sec;
    meta.random_max_sec = max_sec;
    prune_animation_refs(&mut meta);
    save_animation_meta(data_dir, db, &payload.model_id, &meta)?;
    Ok(meta)
}

pub fn save_animation_layout(
    data_dir: &Path,
    db: &rusqlite::Connection,
    payload: &PetAnimationLayoutPayload,
) -> Result<PetAnimationMeta, String> {
    let mut meta = read_animation_meta(data_dir, db, &payload.model_id);
    let known = meta.animations.clone();
    let has = |name: &str| known.is_empty() || known.iter().any(|n| n == name);
    let pick = |raw: &Option<String>, valid: bool| -> Option<String> {
        raw.as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && valid)
    };
    if let Some(idle) = pick(&payload.idle_animation, payload.idle_animation.as_ref().is_some_and(|s| has(s))) {
        meta.idle_animation = Some(idle);
    }
    normalize_idle_animation(&mut meta);
    meta.click_animation = pick(
        &payload.click_animation,
        payload.click_animation.as_ref().is_some_and(|s| has(s)),
    );
    meta.boot_animation = pick(
        &payload.boot_animation,
        payload.boot_animation.as_ref().is_some_and(|s| has(s)),
    );
    meta.return_idle_animation = pick(
        &payload.return_idle_animation,
        payload.return_idle_animation.as_ref().is_some_and(|s| has(s)),
    );
    meta.drag_animation = pick(
        &payload.drag_animation,
        payload.drag_animation.as_ref().is_some_and(|s| has(s)),
    );
    let idle = meta.idle_animation.as_deref().unwrap_or("");
    let mut random: Vec<String> = payload
        .random_animations
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.as_str() != idle && has(s))
        .collect();
    random.sort();
    random.dedup();
    meta.random_animations = random;
    meta.random_min_sec = payload.random_min_sec.clamp(5, 3600);
    meta.random_max_sec = payload.random_max_sec.clamp(meta.random_min_sec, 7200);
    meta.lines = payload
        .lines
        .iter()
        .filter_map(|line| {
            let text = line.text.trim();
            if text.is_empty() {
                return None;
            }
            let animation = line
                .animation
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && has(s));
            Some(PetRemarkLine {
                text: text.to_string(),
                animation,
            })
        })
        .collect();
    if meta.boot_animation.is_none() {
        meta.boot_animation = meta.idle_animation.clone();
    }
    normalize_idle_animation(&mut meta);
    prune_animation_refs(&mut meta);
    save_animation_meta(data_dir, db, &payload.model_id, &meta)?;
    Ok(meta)
}

pub fn import_lines(
    data_dir: &Path,
    db: &rusqlite::Connection,
    payload: &PetImportLinesPayload,
) -> Result<PetAnimationMeta, String> {
    let mut meta = read_animation_meta(data_dir, db, &payload.model_id);
    let has = |name: &str| meta.animations.is_empty() || meta.animations.iter().any(|n| n == name);
    let incoming: Vec<PetRemarkLine> = payload
        .lines
        .iter()
        .filter_map(|line| {
            let text = line.text.trim();
            if text.is_empty() {
                return None;
            }
            let animation = line
                .animation
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && has(s));
            Some(PetRemarkLine {
                text: text.to_string(),
                animation,
            })
        })
        .collect();
    if payload.append {
        meta.lines.extend(incoming);
    } else {
        meta.lines = incoming;
    }
    prune_animation_refs(&mut meta);
    save_animation_meta(data_dir, db, &payload.model_id, &meta)?;
    Ok(meta)
}

pub fn save_model_display_settings(
    data_dir: &Path,
    db: &rusqlite::Connection,
    model_id: &str,
    _power_mode: Option<String>,
    scale: Option<f64>,
    remark_interval_sec: Option<i64>,
) -> Result<PetAnimationMeta, String> {
    let mut meta = read_animation_meta(data_dir, db, model_id);
    // 省电模式已移除，固定 balanced；忽略传入的 power_mode
    meta.power_mode = None;
    if let Some(s) = scale {
        meta.scale = Some(s.clamp(0.4, 1.5));
    }
    if let Some(r) = remark_interval_sec {
        meta.remark_interval_sec = Some(r.clamp(0, 3600));
    }
    save_animation_meta(data_dir, db, model_id, &meta)?;
    Ok(meta)
}

pub fn pick_remark_line(meta: &PetAnimationMeta, animation: Option<&str>) -> Option<PetRemarkLine> {
    if meta.lines.is_empty() {
        return None;
    }
    let mut pool: Vec<&PetRemarkLine> = meta
        .lines
        .iter()
        .filter(|line| match (animation, line.animation.as_deref()) {
            (Some(anim), Some(bind)) => bind == anim,
            (None, None) => true,
            (Some(_), None) => true,
            (None, Some(_)) => false,
        })
        .collect();
    if pool.is_empty() {
        pool = meta.lines.iter().collect();
    }
    if pool.is_empty() {
        return None;
    }
    let idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0) as usize)
        % pool.len();
    Some(pool[idx].clone())
}

fn read_model_name(dir: &Path) -> Option<String> {
    if let Ok(raw) = fs::read_to_string(dir.join("display_name.txt")) {
        let name = raw.trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    let meta = dir.join("model.json");
    if let Ok(raw) = fs::read_to_string(&meta) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            if is_viewer_ex_spine_config(&v) {
                return None;
            }
            return v
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
    }
    None
}

fn write_model_name(dir: &Path, name: &str) -> Result<(), String> {
    let existing = read_model_json_file(dir);
    if is_viewer_ex_spine_config(&existing) {
        fs::write(dir.join("display_name.txt"), name).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let mut file_meta = existing;
    if let Some(obj) = file_meta.as_object_mut() {
        obj.insert("name".into(), serde_json::json!(name));
    } else {
        file_meta = serde_json::json!({ "name": name });
    }
    fs::write(
        dir.join("model.json"),
        serde_json::to_string_pretty(&file_meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn slugify_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("无效模型 ID".into());
    }

    let mut id = String::new();
    for c in trimmed.chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if c == '-' || c == '_' {
            if !id.ends_with('-') {
                id.push('-');
            }
        } else if c.is_whitespace() {
            if !id.is_empty() && !id.ends_with('-') {
                id.push('-');
            }
        }
    }
    let id = id.trim_matches('-').to_string();

    let id = if id.is_empty() {
        fallback_model_id(trimmed)
    } else if id.len() > 48 {
        id.chars().take(48).collect()
    } else {
        id
    };

    if id.is_empty() {
        return Err("无效模型 ID".into());
    }
    Ok(id)
}

fn fallback_model_id(raw: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("m{:08x}", hasher.finish() as u32)
}

fn unique_id(data_dir: &Path, base: &str) -> String {
    let root = models_dir(data_dir);
    let mut id = base.to_string();
    let mut n = 2;
    while root.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

pub fn import_from_folder(
    data_dir: &Path,
    name: &str,
    folder: &Path,
) -> Result<PetModelInfo, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请填写模型名称".into());
    }
    let triple = inspect_spine_folder(folder)?;
    let id = unique_id(data_dir, &slugify_id(name)?);
    let dest = models_dir(data_dir).join(&id);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    copy_model_files(folder, &dest, &triple)?;
    write_model_name(&dest, name)?;
    Ok(PetModelInfo {
        id,
        name: name.to_string(),
        builtin: false,
    })
}

pub fn import_from_files(
    data_dir: &Path,
    payload: &PetImportFilesPayload,
) -> Result<PetModelInfo, String> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err("请填写模型名称".into());
    }
    let id = unique_id(data_dir, &slugify_id(name)?);
    let dest = models_dir(data_dir).join(&id);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    let skel_name = sanitize_spine_filename("model.skel", "skel");
    let atlas_name = sanitize_spine_filename("model.atlas", "atlas");
    let png_name = sanitize_spine_filename("model.png", "png");

    let skel = decode_and_write(&dest, &skel_name, &payload.skel_b64)?;
    let atlas = decode_and_write(&dest, &atlas_name, &payload.atlas_b64)?;
    let png = decode_and_write(&dest, &png_name, &payload.png_b64)?;
    write_model_name(&dest, name)?;

    let triple = SpineTriple(skel, atlas, png);
    sync_model_package(&dest, &triple)?;
    Ok(PetModelInfo {
        id,
        name: name.to_string(),
        builtin: false,
    })
}

fn decode_and_write(dir: &Path, filename: &str, b64: &str) -> Result<String, String> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("Base64 解码失败: {e}"))?;
    if data.is_empty() {
        return Err(format!("{filename} 内容为空"));
    }
    fs::write(dir.join(filename), &data).map_err(|e| e.to_string())?;
    Ok(filename.to_string())
}

fn find_config_file(dir: &Path) -> Option<String> {
    for name in ["config.json", ".config.json"] {
        if dir.join(name).is_file() {
            return Some(name.to_string());
        }
    }
    let model_path = dir.join("model.json");
    if model_path.is_file() {
        if let Ok(raw) = fs::read_to_string(&model_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if is_viewer_ex_spine_config(&v) {
                    return Some("model.json".to_string());
                }
            }
        }
    }
    None
}

fn copy_model_files(src_dir: &Path, dest: &Path, triple: &SpineTriple) -> Result<(), String> {
    for f in [&triple.0, &triple.1, &triple.2] {
        fs::copy(src_dir.join(f), dest.join(f)).map_err(|e| e.to_string())?;
    }
    sync_model_package(dest, triple)?;
    Ok(())
}

pub fn delete_model(
    data_dir: &Path,
    db: &rusqlite::Connection,
    model_id: &str,
) -> Result<(), String> {
    if is_builtin_id(resolve_builtin_id(model_id)) {
        return Err("内置模型不能删除".into());
    }
    let dir = models_dir(data_dir).join(model_id);
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let meta_dir = meta_persist_dir(data_dir, model_id);
    if meta_dir.is_dir() {
        let _ = fs::remove_dir_all(&meta_dir);
    }
    let _ = crate::db::set_setting(db, &meta_db_key(model_id), "");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_model_id() {
        assert_eq!(slugify_id("My Pet 01").unwrap(), "my-pet-01");
    }

    #[test]
    fn slugify_model_id_chinese_name() {
        let id = slugify_id("柴郡").unwrap();
        assert!(id.starts_with('m'));
        assert_eq!(id, slugify_id("柴郡").unwrap());
    }

    #[test]
    fn sync_model_package_fixes_atlas_texture_page() {
        let dir = std::env::temp_dir().join(format!("pet-sync-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("model.skel"), b"skel").unwrap();
        fs::write(dir.join("model.atlas"), "edu_3.png\nsize: 1,1\n").unwrap();
        fs::write(dir.join("model.png"), b"png").unwrap();
        let triple = SpineTriple("model.skel".into(), "model.atlas".into(), "model.png".into());
        sync_model_package(&dir, &triple).unwrap();
        let atlas = fs::read_to_string(dir.join("model.atlas")).unwrap();
        assert!(atlas.starts_with("model.png"));
        assert!(dir.join("config.json").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn try_triple_from_config_reads_viewer_ex() {
        let dir = std::env::temp_dir().join(format!("pet-cfg-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("edu_3.skel"), b"skel").unwrap();
        fs::write(dir.join("edu_3.atlas"), "edu_3.png\nsize: 1,1\n").unwrap();
        fs::write(dir.join("edu_3.png"), b"png").unwrap();
        fs::write(
            dir.join(".config.json"),
            r#"{"type":9,"skeleton":"edu_3.skel","atlases":[{"atlas":"edu_3.atlas","textures":["edu_3.png"]}]}"#,
        )
        .unwrap();
        let triple = inspect_spine_folder(&dir).unwrap();
        assert_eq!(triple.0, "edu_3.skel");
        assert_eq!(triple.1, "edu_3.atlas");
        assert_eq!(triple.2, "edu_3.png");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_click_animation_finds_tap() {
        let names = vec!["idle".into(), "tap_happy".into(), "walk".into()];
        assert_eq!(detect_click_animation(&names).as_deref(), Some("tap_happy"));
    }

    #[test]
    fn detect_idle_animation_prefers_normal_over_sorted_first() {
        let names = vec![
            "dance".into(),
            "normal".into(),
            "stand".into(),
            "touch".into(),
        ];
        assert_eq!(detect_idle_animation(&names).as_deref(), Some("normal"));
    }

    #[test]
    fn apply_template_resolves_animation_names() {
        let template = read_import_action_template();
        let mut meta = PetAnimationMeta {
            animations: vec![
                "Idle".into(),
                "Tap_Happy".into(),
                "DanceMove".into(),
                "Sleepy".into(),
                "DragMove".into(),
            ],
            ..Default::default()
        };
        apply_template_to_meta(&mut meta, &template);
        assert_eq!(meta.idle_animation.as_deref(), Some("Idle"));
        assert_eq!(meta.click_animation.as_deref(), Some("Tap_Happy"));
        assert_eq!(meta.drag_animation.as_deref(), Some("DragMove"));
        assert_eq!(
            meta.random_animations,
            vec!["DanceMove".to_string(), "Sleepy".to_string()]
        );
        assert_eq!(meta.lines.len(), template.lines.len());
    }

    #[test]
    fn import_action_template_loads_defaults() {
        let template = read_import_action_template();
        assert_eq!(template.click_animation.as_deref(), Some("touch"));
        assert_eq!(template.random_min_sec, 30);
        assert!(!template.lines.is_empty());
    }

    #[test]
    fn viewer_ex_config_detected_in_model_json() {
        let raw = serde_json::json!({
            "type": 9,
            "skeleton": "pet.skel",
            "atlases": [{"atlas": "pet.atlas", "textures": ["pet.png"]}]
        });
        assert!(is_viewer_ex_spine_config(&raw));
    }

    #[test]
    fn generate_viewer_ex_config_matches_azurlanesd_shape() {
        let cfg = generate_viewer_ex_config("edu_3.skel", "edu_3.atlas", "edu_3.png");
        assert_eq!(cfg["type"], 9);
        assert_eq!(cfg["skeleton"], "edu_3.skel");
        assert_eq!(cfg["atlases"][0]["atlas"], "edu_3.atlas");
        assert_eq!(cfg["atlases"][0]["textures"][0], "edu_3.png");
        assert_eq!(cfg["atlases"][0]["tex_names"][0], "edu_3");
    }
}
