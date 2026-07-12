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

// ── 仓库 bundled 源（开发 / 导出内置模型）────────────────────────

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub fn bundled_roster_dir() -> PathBuf {
    repo_root().join("bundled/roster")
}

pub fn bundled_prompts_dir() -> PathBuf {
    repo_root().join("bundled/prompts")
}

pub fn bundled_app_icon_source() -> PathBuf {
    repo_root().join("bundled/app-icon-square.png")
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
