//! Live2D 批量导入：读取 plan.json，导入 Spine 模型并绑定人物皮肤

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::character;
use crate::pet::models;

#[derive(Debug, Deserialize)]
struct Live2dPlanFile {
    plan: Vec<Live2dPlanItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Live2dPlanItem {
    pub folder: String,
    #[serde(default)]
    pub folder_path: String,
    pub skin_name: String,
    pub model_name: String,
    pub display_name: String,
    #[serde(default)]
    pub character_id: Option<String>,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Live2dImportResult {
    pub ok: usize,
    pub skipped: usize,
    pub failed: usize,
    pub processed: usize,
    pub remaining: usize,
    pub message: String,
}

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

pub fn resolve_live2d_root() -> PathBuf {
    if let Ok(p) = std::env::var("HANDAILY_LIVE2D_PATH") {
        let path = PathBuf::from(p.trim());
        if path.is_dir() {
            return path;
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for base in [cwd.clone(), cwd.join(".."), cwd.join("../..")] {
            let candidate = base.join("live2d");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    PathBuf::from("live2d")
}

pub fn resolve_plan_path(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        return Err(format!("计划文件不存在: {}", p.display()));
    }
    if let Ok(p) = std::env::var("HANDAILY_LIVE2D_PLAN") {
        let path = PathBuf::from(p.trim());
        if path.is_file() {
            return Ok(path);
        }
    }
    let candidates = [
        handaily_data_dir()
            .ok()
            .map(|d| d.join("live2d-plan.json")),
        std::env::current_dir()
            .ok()
            .map(|c| c.join("mcp/blhx-wiki/plan.json")),
        std::env::current_dir()
            .ok()
            .map(|c| c.join("../mcp/blhx-wiki/plan.json")),
    ];
    for c in candidates.into_iter().flatten() {
        if c.is_file() {
            return Ok(c);
        }
    }
    Err("未找到 live2d 导入计划；请先运行 npm run live2d-plan 或设置 HANDAILY_LIVE2D_PLAN".into())
}

fn resolve_folder_path(item: &Live2dPlanItem, live2d_root: &Path) -> PathBuf {
    if !item.folder_path.is_empty() {
        return PathBuf::from(&item.folder_path);
    }
    live2d_root.join(&item.folder)
}

fn resolve_character_id(data_dir: &Path, item: &Live2dPlanItem) -> Option<String> {
    if let Some(id) = item.character_id.as_ref().filter(|s| !s.trim().is_empty()) {
        return Some(id.clone());
    }
    let manifest = crate::persona::load_manifest(data_dir);
    manifest
        .personas
        .iter()
        .find(|p| p.name == item.display_name)
        .map(|p| p.id.clone())
}

fn existing_model_names(data_dir: &Path) -> HashSet<String> {
    models::list_models(data_dir)
        .ok()
        .map(|list| list.into_iter().map(|m| m.name).collect())
        .unwrap_or_default()
}

pub fn run_live2d_import(
    data_dir: &Path,
    _db: &rusqlite::Connection,
    plan_path: &Path,
    live2d_root: &Path,
    limit: usize,
    dry_run: bool,
) -> Result<Live2dImportResult, String> {
    let raw = fs::read_to_string(plan_path).map_err(|e| format!("读取计划失败: {e}"))?;
    let file: Live2dPlanFile =
        serde_json::from_str(&raw).map_err(|e| format!("解析计划 JSON 失败: {e}"))?;

    let mut known_names = existing_model_names(data_dir);
    let pending: Vec<_> = file
        .plan
        .into_iter()
        .filter(|p| p.action == "import")
        .filter(|p| !known_names.contains(&p.model_name))
        .collect();

    let remaining_total = pending.len();
    let batch: Vec<_> = pending.into_iter().take(limit.max(1)).collect();

    let mut ok = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut manifest_dirty = false;
    let mut manifest = if dry_run {
        None
    } else {
        Some(character::load_manifest(data_dir))
    };

    for item in &batch {
        let folder = resolve_folder_path(item, live2d_root);
        let character_id = match resolve_character_id(data_dir, item) {
            Some(id) => id,
            None => {
                skipped += 1;
                continue;
            }
        };

        if dry_run {
            ok += 1;
            continue;
        }

        match models::import_from_folder(data_dir, &item.model_name, &folder) {
            Ok(model) => {
                known_names.insert(model.name.clone());
                if let Some(manifest) = manifest.as_mut() {
                    match character::attach_model_in_manifest(
                        data_dir,
                        manifest,
                        &character_id,
                        &model,
                        &item.skin_name,
                    ) {
                        Ok(()) => {
                            manifest_dirty = true;
                            ok += 1;
                        }
                        Err(_) => failed += 1,
                    }
                } else {
                    ok += 1;
                }
            }
            Err(_) => failed += 1,
        }
    }

    if manifest_dirty {
        if let Some(mut manifest) = manifest {
            let _ = character::repair_character_manifest_skins(data_dir, &mut manifest);
            character::save_manifest(data_dir, &manifest)?;
        }
    }

    let remaining = remaining_total.saturating_sub(ok + skipped + failed);
    let message = format!(
        "本批处理 {} 条：成功 {ok}，跳过 {skipped}，失败 {failed}，剩余约 {remaining} 条",
        batch.len()
    );

    Ok(Live2dImportResult {
        ok,
        skipped,
        failed,
        processed: batch.len(),
        remaining,
        message,
    })
}

/// 为指定人物从 Live2D 计划导入全部待导入皮肤
pub fn run_live2d_import_for_character(
    data_dir: &Path,
    plan_path: &Path,
    live2d_root: &Path,
    character_id: &str,
) -> Result<Live2dImportResult, String> {
    let meta = character::find_character_meta(data_dir, character_id)?;
    let char_name = meta.name.clone();

    let raw = fs::read_to_string(plan_path).map_err(|e| format!("读取计划失败: {e}"))?;
    let file: Live2dPlanFile =
        serde_json::from_str(&raw).map_err(|e| format!("解析计划 JSON 失败: {e}"))?;

    let mut known_names = existing_model_names(data_dir);
    let pending: Vec<_> = file
        .plan
        .into_iter()
        .filter(|p| p.action == "import")
        .filter(|p| !known_names.contains(&p.model_name))
        .filter(|p| {
            p.character_id
                .as_ref()
                .filter(|s| !s.trim().is_empty())
                .map(|id| id == character_id)
                .unwrap_or_else(|| p.display_name == char_name)
        })
        .collect();

    if pending.is_empty() {
        return Ok(Live2dImportResult {
            ok: 0,
            skipped: 0,
            failed: 0,
            processed: 0,
            remaining: 0,
            message: format!("「{}」没有可导入的 Live2D 模型", char_name),
        });
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut manifest = character::load_manifest(data_dir);

    for item in &pending {
        let folder = resolve_folder_path(item, live2d_root);
        match models::import_from_folder(data_dir, &item.model_name, &folder) {
            Ok(model) => {
                known_names.insert(model.name.clone());
                match character::attach_model_in_manifest(
                    data_dir,
                    &mut manifest,
                    character_id,
                    &model,
                    &item.skin_name,
                ) {
                    Ok(()) => ok += 1,
                    Err(_) => failed += 1,
                }
            }
            Err(_) => failed += 1,
        }
    }

    if ok > 0 {
        let _ = character::repair_character_manifest_skins(data_dir, &mut manifest);
        character::save_manifest(data_dir, &manifest)?;
    }

    let message = if ok > 0 {
        format!("已为「{char_name}」导入 {ok} 套 Live2D 皮肤")
    } else {
        format!("「{char_name}」Live2D 导入失败（{failed} 条）")
    };

    Ok(Live2dImportResult {
        ok,
        skipped: 0,
        failed,
        processed: pending.len(),
        remaining: 0,
        message,
    })
}
