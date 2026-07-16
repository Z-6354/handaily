use std::fs::{self, File};
use std::io::copy;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;

use crate::character::{self, CharacterManifest};
use crate::data_layout;
use crate::persona::{self, PersonaManifest};

use super::{PackMeta, META_FILENAME, PACK_FORMAT, PACK_VERSION};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterPackImportProgress {
    pub phase: String,
    pub index: u32,
    pub total: u32,
    pub message: String,
    pub characters_added: u32,
    pub characters_updated: u32,
    pub models_copied: u32,
    pub models_skipped: u32,
    pub personas_copied: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterPackImportResult {
    pub pack_label: String,
    pub characters_added: u32,
    pub characters_updated: u32,
    pub models_copied: u32,
    pub models_skipped: u32,
    pub personas_copied: u32,
}

pub fn emit_progress(app: &AppHandle, payload: RosterPackImportProgress) {
    let _ = app.emit("roster-pack-import-progress", payload);
}

pub fn import_from_zip(
    data_dir: &Path,
    zip_path: &Path,
    app: Option<&AppHandle>,
) -> Result<RosterPackImportResult, String> {
    let temp = std::env::temp_dir().join(format!(
        "handaily-roster-import-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;

    let result = (|| {
        extract_zip(zip_path, &temp)?;
        let meta = read_pack_meta(&temp)?;
        validate_meta(&meta)?;

        if let Some(app) = app {
            emit_progress(
                app,
                RosterPackImportProgress {
                    phase: "extract".into(),
                    index: 1,
                    total: 4,
                    message: format!("已解压「{}」", meta.pack_label),
                    characters_added: 0,
                    characters_updated: 0,
                    models_copied: 0,
                    models_skipped: 0,
                    personas_copied: 0,
                },
            );
        }

        let mut stats = ImportStats::default();
        merge_personas(data_dir, &temp, app, &mut stats)?;
        merge_characters(data_dir, &temp, app, &mut stats)?;
        copy_models_and_meta(data_dir, &temp, app, &mut stats)?;
        let _ = character::sync_character_manifest_from_personas(data_dir)?;

        Ok(RosterPackImportResult {
            pack_label: meta.pack_label,
            characters_added: stats.characters_added,
            characters_updated: stats.characters_updated,
            models_copied: stats.models_copied,
            models_skipped: stats.models_skipped,
            personas_copied: stats.personas_copied,
        })
    })();

    let _ = fs::remove_dir_all(&temp);
    result
}

#[derive(Default)]
struct ImportStats {
    characters_added: u32,
    characters_updated: u32,
    models_copied: u32,
    models_skipped: u32,
    personas_copied: u32,
}

fn read_pack_meta(staging: &Path) -> Result<PackMeta, String> {
    let path = staging.join(META_FILENAME);
    let raw = fs::read_to_string(&path).map_err(|_| "不是有效的角色包：缺少 handaily-roster-pack.json".to_string())?;
    serde_json::from_str(&raw).map_err(|e| format!("角色包元信息无效: {e}"))
}

fn validate_meta(meta: &PackMeta) -> Result<(), String> {
    if meta.format != PACK_FORMAT {
        return Err(format!("不支持的包格式: {}", meta.format));
    }
    if meta.version > PACK_VERSION {
        return Err(format!(
            "角色包版本 {} 高于当前应用支持的 {}",
            meta.version, PACK_VERSION
        ));
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file = File::open(zip_path).map_err(|e| format!("无法打开 zip: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("无法读取 zip: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out_file = File::create(&out).map_err(|e| e.to_string())?;
        copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn merge_personas(
    data_dir: &Path,
    staging: &Path,
    app: Option<&AppHandle>,
    stats: &mut ImportStats,
) -> Result<(), String> {
    let incoming_path = data_layout::roster_staging_personas_manifest(staging);
    if !incoming_path.is_file() {
        return Err(format!(
            "角色包缺少 {}",
            data_layout::ROSTER_PERSONAS_MANIFEST
        ));
    }
    let raw = fs::read_to_string(&incoming_path).map_err(|e| e.to_string())?;
    let incoming: PersonaManifest = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    persona::mutate_persona_manifest(data_dir, |manifest| {
        for p in &incoming.personas {
            if let Some(existing) = manifest.personas.iter_mut().find(|x| x.id == p.id) {
                existing.name = p.name.clone();
                existing.source = p.source.clone();
                existing.description = p.description.clone();
            } else {
                manifest.personas.push(p.clone());
            }
        }
        Ok(())
    })?;

    let src_personas = data_layout::roster_staging_personas_dir(staging);
    let dest_personas = persona::personas_dir(data_dir);
    fs::create_dir_all(&dest_personas).map_err(|e| e.to_string())?;
    for p in &incoming.personas {
        let mut copied = false;
        for ext in ["md", "json"] {
            let src = src_personas.join(format!("{}.{ext}", p.id));
            if src.is_file() {
                fs::copy(&src, dest_personas.join(format!("{}.{ext}", p.id)))
                    .map_err(|e| e.to_string())?;
                copied = true;
            }
        }
        if copied {
            stats.personas_copied += 1;
        }
    }

    if let Some(app) = app {
        emit_progress(
            app,
            RosterPackImportProgress {
                phase: "personas".into(),
                index: 2,
                total: 4,
                message: format!("已合并 {} 个人设", incoming.personas.len()),
                characters_added: stats.characters_added,
                characters_updated: stats.characters_updated,
                models_copied: stats.models_copied,
                models_skipped: stats.models_skipped,
                personas_copied: stats.personas_copied,
            },
        );
    }
    Ok(())
}

fn merge_characters(
    data_dir: &Path,
    staging: &Path,
    app: Option<&AppHandle>,
    stats: &mut ImportStats,
) -> Result<(), String> {
    let incoming_path = data_layout::roster_staging_characters_manifest(staging);
    if !incoming_path.is_file() {
        return Err(format!(
            "角色包缺少 {}",
            data_layout::ROSTER_CHARACTERS_MANIFEST
        ));
    }
    let raw = fs::read_to_string(&incoming_path).map_err(|e| e.to_string())?;
    let incoming: CharacterManifest = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    character::mutate_character_manifest(data_dir, |manifest| {
        for inc in incoming.characters {
            if let Some(existing) = manifest.characters.iter_mut().find(|c| c.id == inc.id) {
                *existing = inc;
                stats.characters_updated += 1;
            } else {
                manifest.characters.push(inc);
                stats.characters_added += 1;
            }
        }
        Ok(())
    })?;

    if let Some(app) = app {
        emit_progress(
            app,
            RosterPackImportProgress {
                phase: "characters".into(),
                index: 3,
                total: 4,
                message: format!(
                    "已合并角色：新增 {}，更新 {}",
                    stats.characters_added, stats.characters_updated
                ),
                characters_added: stats.characters_added,
                characters_updated: stats.characters_updated,
                models_copied: stats.models_copied,
                models_skipped: stats.models_skipped,
                personas_copied: stats.personas_copied,
            },
        );
    }
    Ok(())
}

fn copy_models_and_meta(
    data_dir: &Path,
    staging: &Path,
    app: Option<&AppHandle>,
    stats: &mut ImportStats,
) -> Result<(), String> {
    let models_src = data_layout::roster_staging_pet_models_dir(staging);
    if !models_src.is_dir() {
        return Ok(());
    }
    let models_dest = crate::pet::models::models_dir(data_dir);
    fs::create_dir_all(&models_dest).map_err(|e| e.to_string())?;

    let entries: Vec<PathBuf> = fs::read_dir(&models_src)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();

    for (i, src) in entries.iter().enumerate() {
        let model_id = src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if model_id.is_empty() {
            continue;
        }
        if crate::pet::models::is_builtin_model(&model_id) {
            stats.models_skipped += 1;
            continue;
        }
        let dest = models_dest.join(&model_id);
        copy_dir_all(src, &dest)?;
        stats.models_copied += 1;

        let meta_src = data_layout::roster_staging_pet_meta_file(staging, &model_id);
        if meta_src.is_file() {
            let meta_dest_dir = data_layout::pet_meta_model_dir(data_dir, &model_id);
            fs::create_dir_all(&meta_dest_dir).map_err(|e| e.to_string())?;
            fs::copy(
                &meta_src,
                data_layout::pet_meta_file(data_dir, &model_id),
            )
            .map_err(|e| e.to_string())?;
        }

        if let Some(app) = app {
            if i % 20 == 0 || i + 1 == entries.len() {
                emit_progress(
                    app,
                    RosterPackImportProgress {
                        phase: "models".into(),
                        index: 4,
                        total: 4,
                        message: format!(
                            "复制模型 {}/{}（跳过已存在 {}）",
                            i + 1,
                            entries.len(),
                            stats.models_skipped
                        ),
                        characters_added: stats.characters_added,
                        characters_updated: stats.characters_updated,
                        models_copied: stats.models_copied,
                        models_skipped: stats.models_skipped,
                        personas_copied: stats.personas_copied,
                    },
                );
            }
        }
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let dest_path = dest.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
