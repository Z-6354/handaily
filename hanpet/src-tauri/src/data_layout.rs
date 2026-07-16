//! AppData 与角色包（roster pack）目录布局 — 路径唯一来源

use std::path::{Path, PathBuf};

// ── AppData 子目录名 ──────────────────────────────────────────────

pub const DIR_CHARACTERS: &str = "characters";
pub const DIR_PERSONAS: &str = "personas";
pub const DIR_PET_MODELS: &str = "pet-models";
pub const DIR_PET_META: &str = "pet-meta";
pub const DIR_PROMPTS: &str = "prompts";
pub const DIR_LOGS: &str = "logs";
pub const DIR_PET_IMPORT_STAGING: &str = "pet-import-staging";
pub const DIR_KANMUSU: &str = "kanmusu";
pub const DIR_KANMUSU_MODELS: &str = "kanmusu-models";

pub const SUBDIR_AVATARS: &str = "avatars";

pub const FILE_DB: &str = "xiaohan.sqlite";
pub const FILE_MANIFEST: &str = "manifest.json";
pub const FILE_ANIMATIONS_META: &str = "animations.meta.json";
pub const FILE_LIVE2D_PLAN: &str = "live2d-plan.json";

// ── 角色包 zip / staging 内相对路径 ───────────────────────────────

pub const ROSTER_CHARACTERS_MANIFEST: &str = "characters/manifest.json";
pub const ROSTER_PERSONAS_MANIFEST: &str = "personas/manifest.json";
pub const ROSTER_DIR_PERSONAS: &str = "personas";
pub const ROSTER_DIR_PET_MODELS: &str = "pet-models";
pub const ROSTER_DIR_PET_META: &str = "pet-meta";

// ── 数据根目录 ────────────────────────────────────────────────────

pub fn handaily_data_dir() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("HANDAILY_DATA_DIR") {
        let path = PathBuf::from(p.trim());
        if !path.as_os_str().is_empty() {
            return Ok(path);
        }
    }
    let appdata = std::env::var("APPDATA").map_err(|_| "无法读取 APPDATA".to_string())?;
    Ok(PathBuf::from(appdata).join("xiaohan-daily").join("data"))
}

// ── AppData 路径 ──────────────────────────────────────────────────

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_DB)
}

pub fn characters_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_CHARACTERS)
}

pub fn characters_manifest_path(data_dir: &Path) -> PathBuf {
    characters_dir(data_dir).join(FILE_MANIFEST)
}

pub fn avatars_dir(data_dir: &Path) -> PathBuf {
    characters_dir(data_dir).join(SUBDIR_AVATARS)
}

pub fn personas_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_PERSONAS)
}

pub fn pet_models_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_PET_MODELS)
}

pub fn pet_meta_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_PET_META)
}

pub fn pet_meta_model_dir(data_dir: &Path, model_id: &str) -> PathBuf {
    pet_meta_dir(data_dir).join(model_id)
}

pub fn pet_meta_file(data_dir: &Path, model_id: &str) -> PathBuf {
    pet_meta_model_dir(data_dir, model_id).join(FILE_ANIMATIONS_META)
}

pub fn prompts_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_PROMPTS)
}

pub fn logs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_LOGS)
}

pub fn pet_import_staging_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_PET_IMPORT_STAGING)
}

pub fn live2d_plan_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_LIVE2D_PLAN)
}

pub fn kanmusu_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_KANMUSU)
}

pub fn kanmusu_manifest_path(data_dir: &Path) -> PathBuf {
    kanmusu_dir(data_dir).join(FILE_MANIFEST)
}

pub fn kanmusu_models_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_KANMUSU_MODELS)
}

pub fn kanmusu_model_dir(data_dir: &Path, slug: &str) -> PathBuf {
    kanmusu_models_dir(data_dir).join(slug)
}

// ── 角色包 staging 路径 ───────────────────────────────────────────

pub fn roster_staging_characters_manifest(staging: &Path) -> PathBuf {
    staging.join(ROSTER_CHARACTERS_MANIFEST)
}

pub fn roster_staging_personas_manifest(staging: &Path) -> PathBuf {
    staging.join(ROSTER_PERSONAS_MANIFEST)
}

pub fn roster_staging_personas_dir(staging: &Path) -> PathBuf {
    staging.join(ROSTER_DIR_PERSONAS)
}

pub fn roster_staging_pet_models_dir(staging: &Path) -> PathBuf {
    staging.join(ROSTER_DIR_PET_MODELS)
}

pub fn roster_staging_pet_model_dir(staging: &Path, model_id: &str) -> PathBuf {
    roster_staging_pet_models_dir(staging).join(model_id)
}

pub fn roster_staging_pet_meta_file(staging: &Path, model_id: &str) -> PathBuf {
    staging
        .join(ROSTER_DIR_PET_META)
        .join(model_id)
        .join(FILE_ANIMATIONS_META)
}

// ── hanpet 应用根 / HANDAILY 项目根 ───────────────────────────────

/// hanpet 应用根（`hanpet/`，含 bundled、src）
pub fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// HANDAILY 项目根（含 `data/`、`hanimport/`、`mcp/`）
pub fn project_root() -> PathBuf {
    app_root().join("..")
}

/// @deprecated 使用 `app_root()`
pub fn repo_root() -> PathBuf {
    app_root()
}

pub fn bundled_roster_dir() -> PathBuf {
    app_root().join("bundled/roster")
}

pub fn bundled_prompts_dir() -> PathBuf {
    app_root().join("bundled/prompts")
}

pub fn bundled_app_icon_source() -> PathBuf {
    app_root().join("bundled/app-icon-square.png")
}

pub fn bundled_pet_models_dir() -> PathBuf {
    bundled_roster_dir().join(ROSTER_DIR_PET_MODELS)
}

pub fn bundled_pet_model_dir(model_id: &str) -> PathBuf {
    bundled_pet_models_dir().join(model_id)
}

pub fn bundled_pet_meta_file(model_id: &str) -> PathBuf {
    bundled_pet_model_dir(model_id).join(FILE_ANIMATIONS_META)
}

// ── 仓库 data/ 工作区（hanimport 开发数据）────────────────────────

pub const REPO_DATA_LIVE2D: &str = "data/live2d";
pub const REPO_DATA_MODEL_UNPACKED: &str = "data/model/unpacked";
pub const REPO_DATA_IMPORT_PLAN: &str = "data/import/live2d-plan.json";
pub const REPO_LEGACY_LIVE2D: &str = "live2d";

/// 解析仓库内 Spine 模型工作目录：`HANDAILY_LIVE2D_PATH` → `data/live2d` → 旧 `live2d/`
pub fn resolve_repo_live2d_root() -> PathBuf {
    if let Ok(p) = std::env::var("HANDAILY_LIVE2D_PATH") {
        let path = PathBuf::from(p.trim());
        if path.is_dir() {
            return path;
        }
    }
    let mut bases = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd.clone());
        bases.push(cwd.join(".."));
        bases.push(cwd.join("../.."));
    }
    bases.push(project_root());
    bases.push(app_root());
    for base in bases {
        for rel in [REPO_DATA_LIVE2D, REPO_LEGACY_LIVE2D] {
            let candidate = base.join(rel);
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    project_root().join(REPO_DATA_LIVE2D)
}

/// 解析仓库内 Cubism 解包目录：`HANDAILY_MODEL_UNPACKED` → `data/model/unpacked`
pub fn resolve_repo_model_unpacked_root() -> PathBuf {
    if let Ok(p) = std::env::var("HANDAILY_MODEL_UNPACKED") {
        let path = PathBuf::from(p.trim());
        if path.is_dir() {
            return path;
        }
    }
    let mut bases = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd.clone());
        bases.push(cwd.join(".."));
        bases.push(cwd.join("../.."));
    }
    bases.push(project_root());
    bases.push(app_root());
    for base in bases {
        let candidate = base.join(REPO_DATA_MODEL_UNPACKED);
        if candidate.is_dir() {
            return candidate;
        }
    }
    project_root().join(REPO_DATA_MODEL_UNPACKED)
}

/// 解析 live2d 导入计划 JSON 路径
pub fn resolve_repo_live2d_plan_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("HANDAILY_LIVE2D_PLAN") {
        let path = PathBuf::from(p.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    let mut bases = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd.clone());
        bases.push(cwd.join(".."));
        bases.push(cwd.join("../.."));
    }
    bases.push(project_root());
    bases.push(app_root());
    for base in bases {
        let candidate = base.join(REPO_DATA_IMPORT_PLAN);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
