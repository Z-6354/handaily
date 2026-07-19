//! Import `handaily-skin-slot` zips (single slot or outer multi-slot download pack).

use std::fs::{self, File};
use std::io::{copy, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;

use crate::character::{
    self, CharacterManifest, CharacterMeta, CharacterSkinLine, CharacterSkinMeta,
};
use crate::data_layout;

pub const SLOT_FORMAT: &str = "handaily-skin-slot";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotPackImportProgress {
    pub phase: String,
    pub index: u32,
    pub total: u32,
    pub message: String,
    pub slots_imported: u32,
    pub characters_added: u32,
    pub characters_updated: u32,
    pub models_copied: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotPackImportResult {
    pub pack_label: String,
    pub slots_imported: u32,
    pub slots_failed: u32,
    pub characters_added: u32,
    pub characters_updated: u32,
    pub models_copied: u32,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SlotManifest {
    format: String,
    #[serde(default)]
    format_version: u32,
    character: SlotCharacter,
    skin: SlotSkin,
}

#[derive(Debug, Deserialize)]
struct SlotCharacter {
    id: String,
    name_zh: String,
    #[serde(default)]
    name_en: String,
    #[serde(default)]
    faction: String,
    #[serde(default)]
    wiki_title: String,
}

#[derive(Debug, Deserialize)]
struct SlotSkin {
    id: String,
    name_zh: String,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    pet_model_id: String,
    #[serde(default)]
    kanmusu_dir: String,
    #[serde(default)]
    has_pet: bool,
    #[serde(default)]
    has_kanmusu: bool,
}

#[derive(Debug, Deserialize)]
struct SlotLineRow {
    #[serde(default)]
    text: String,
    #[serde(default)]
    animation: String,
    #[serde(default)]
    wiki_key: String,
}

#[derive(Default)]
struct ImportStats {
    slots_imported: u32,
    slots_failed: u32,
    characters_added: u32,
    characters_updated: u32,
    models_copied: u32,
    errors: Vec<String>,
    seen_new_chars: std::collections::HashSet<String>,
    seen_upd_chars: std::collections::HashSet<String>,
}

pub fn emit_progress(app: &AppHandle, payload: SlotPackImportProgress) {
    let _ = app.emit("slot-pack-import-progress", payload);
}

pub fn import_from_zip(
    data_dir: &Path,
    zip_path: &Path,
    app: Option<&AppHandle>,
) -> Result<SlotPackImportResult, String> {
    let temp = std::env::temp_dir().join(format!(
        "handaily-slot-import-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    if temp.exists() {
        let _ = fs::remove_dir_all(&temp);
    }
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;

    let result = (|| {
        extract_zip(zip_path, &temp)?;
        let slot_zips = list_slot_zips(&temp);
        let mut stats = ImportStats::default();

        if !slot_zips.is_empty() {
            let total = slot_zips.len() as u32;
            let label = zip_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("皮肤包")
                .to_string();
            if let Some(app) = app {
                emit_progress(
                    app,
                    SlotPackImportProgress {
                        phase: "extract".into(),
                        index: 0,
                        total,
                        message: format!("已解压「{label}」，共 {total} 个皮肤槽"),
                        slots_imported: 0,
                        characters_added: 0,
                        characters_updated: 0,
                        models_copied: 0,
                    },
                );
            }
            for (i, inner) in slot_zips.iter().enumerate() {
                let idx = (i + 1) as u32;
                match import_one_slot_zip(data_dir, inner, &mut stats) {
                    Ok(()) => {
                        if let Some(app) = app {
                            emit_progress(
                                app,
                                SlotPackImportProgress {
                                    phase: "slot".into(),
                                    index: idx,
                                    total,
                                    message: format!(
                                        "已导入 {}/{} {}",
                                        idx,
                                        total,
                                        inner
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("slot")
                                    ),
                                    slots_imported: stats.slots_imported,
                                    characters_added: stats.characters_added,
                                    characters_updated: stats.characters_updated,
                                    models_copied: stats.models_copied,
                                },
                            );
                        }
                    }
                    Err(e) => {
                        stats.slots_failed += 1;
                        stats.errors.push(format!(
                            "{}: {e}",
                            inner
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("slot")
                        ));
                    }
                }
            }
            return Ok(SlotPackImportResult {
                pack_label: label,
                slots_imported: stats.slots_imported,
                slots_failed: stats.slots_failed,
                characters_added: stats.characters_added,
                characters_updated: stats.characters_updated,
                models_copied: stats.models_copied,
                errors: stats.errors,
            });
        }

        // Single slot: staging root is already the slot contents
        if temp.join("manifest.json").is_file() {
            import_one_slot_dir(data_dir, &temp, &mut stats)?;
            let label = zip_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("皮肤槽")
                .to_string();
            return Ok(SlotPackImportResult {
                pack_label: label,
                slots_imported: stats.slots_imported,
                slots_failed: stats.slots_failed,
                characters_added: stats.characters_added,
                characters_updated: stats.characters_updated,
                models_copied: stats.models_copied,
                errors: stats.errors,
            });
        }

        Err(
            "不是有效的皮肤分发包：缺少 *.slot.zip 或 handaily-skin-slot manifest.json"
                .into(),
        )
    })();

    let _ = fs::remove_dir_all(&temp);
    result
}

fn list_slot_zips(staging: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(staging) else {
        return out;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_file() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.ends_with(".slot.zip") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
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

fn import_one_slot_zip(
    data_dir: &Path,
    zip_path: &Path,
    stats: &mut ImportStats,
) -> Result<(), String> {
    let slot_temp = std::env::temp_dir().join(format!(
        "handaily-slot-one-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    if slot_temp.exists() {
        let _ = fs::remove_dir_all(&slot_temp);
    }
    fs::create_dir_all(&slot_temp).map_err(|e| e.to_string())?;
    let res = (|| {
        extract_zip(zip_path, &slot_temp)?;
        import_one_slot_dir(data_dir, &slot_temp, stats)
    })();
    let _ = fs::remove_dir_all(&slot_temp);
    res
}

fn import_one_slot_dir(
    data_dir: &Path,
    slot_dir: &Path,
    stats: &mut ImportStats,
) -> Result<(), String> {
    let manifest_path = slot_dir.join("manifest.json");
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|_| "缺少 manifest.json".to_string())?;
    let meta: SlotManifest =
        serde_json::from_str(&raw).map_err(|e| format!("manifest 无效: {e}"))?;
    if meta.format != SLOT_FORMAT {
        return Err(format!("不支持的格式: {}", meta.format));
    }
    if meta.format_version > 1 {
        return Err(format!(
            "皮肤包版本 {} 高于当前支持的 1",
            meta.format_version
        ));
    }
    if !meta.skin.has_pet && meta.skin.pet_model_id.trim().is_empty() {
        return Err("皮肤槽缺少桌宠".into());
    }

    let pet_id = meta.skin.pet_model_id.trim().to_string();
    if !pet_id.is_empty() {
        let src_pet = slot_dir.join("pet").join(&pet_id);
        if src_pet.is_dir() {
            let dest = data_layout::pet_models_dir(data_dir).join(&pet_id);
            copy_dir_overwrite(&src_pet, &dest)?;
            stats.models_copied += 1;
        } else {
            return Err(format!("包内缺少 pet/{pet_id}"));
        }
    }

    let km = meta.skin.kanmusu_dir.trim().to_string();
    let mut kanmusu_dir_opt: Option<String> = None;
    if meta.skin.has_kanmusu && !km.is_empty() {
        let src_km = slot_dir.join("skin").join(&km);
        if src_km.is_dir() {
            let dest = data_layout::kanmusu_model_dir(data_dir, &km);
            copy_dir_overwrite(&src_km, &dest)?;
            kanmusu_dir_opt = Some(km);
            stats.models_copied += 1;
        }
    }

    // avatar
    let av_dir = data_layout::avatars_dir(data_dir);
    fs::create_dir_all(&av_dir).map_err(|e| e.to_string())?;
    for ent in fs::read_dir(slot_dir).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let p = ent.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with("avatar.") && p.is_file() {
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("jpg");
            let dest = av_dir.join(format!("{}.{}", meta.character.id, ext));
            fs::copy(&p, &dest).map_err(|e| e.to_string())?;
            break;
        }
    }

    let lines = load_lines(slot_dir);
    merge_character_skin(data_dir, &meta, kanmusu_dir_opt, lines, stats)?;
    stats.slots_imported += 1;
    Ok(())
}

fn load_lines(slot_dir: &Path) -> Vec<CharacterSkinLine> {
    let path = slot_dir.join("lines.json");
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(rows) = serde_json::from_str::<Vec<SlotLineRow>>(&raw) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter(|r| !r.text.trim().is_empty())
        .map(|r| CharacterSkinLine {
            text: r.text,
            animation: if r.animation.is_empty() {
                None
            } else {
                Some(r.animation)
            },
            wiki_key: if r.wiki_key.is_empty() {
                None
            } else {
                Some(r.wiki_key)
            },
            audio_url: None,
            audio_relpath: None,
        })
        .collect()
}

fn merge_character_skin(
    data_dir: &Path,
    meta: &SlotManifest,
    kanmusu_dir: Option<String>,
    lines: Vec<CharacterSkinLine>,
    stats: &mut ImportStats,
) -> Result<(), String> {
    let cid = meta.character.id.clone();
    let skin_id = meta.skin.id.clone();
    let name_zh = meta.character.name_zh.clone();
    let name_en = meta.character.name_en.clone();
    let faction = meta.character.faction.clone();
    let wiki_title = meta.character.wiki_title.clone();
    let skin_name = meta.skin.name_zh.clone();
    let is_default = meta.skin.is_default;
    let model_id = meta.skin.pet_model_id.trim().to_string();

    character::mutate_character_manifest(data_dir, |manifest: &mut CharacterManifest| {
        let existing = manifest.characters.iter_mut().find(|c| c.id == cid);
        let skin = CharacterSkinMeta {
            id: skin_id.clone(),
            name: skin_name,
            model_id,
            default: is_default,
            skin_index: None,
            kanmusu_dir,
            english_name: String::new(),
            lines,
        };

        if let Some(ch) = existing {
            if let Some(s) = ch.skins.iter_mut().find(|s| s.id == skin_id) {
                *s = skin;
            } else {
                ch.skins.push(skin);
            }
            if is_default {
                for s in &mut ch.skins {
                    s.default = s.id == skin_id;
                }
            }
            if !name_zh.is_empty() {
                ch.name = name_zh.clone();
            }
            if !name_en.is_empty() {
                ch.english_name = name_en.clone();
            }
            if !faction.is_empty() {
                ch.faction = faction.clone();
            }
            if !wiki_title.is_empty() {
                ch.wiki_title = wiki_title.clone();
            }
            if stats.seen_new_chars.contains(&cid) {
                // already counted as added this batch
            } else if stats.seen_upd_chars.insert(cid.clone()) {
                stats.characters_updated += 1;
            }
        } else {
            let ch = CharacterMeta {
                id: cid.clone(),
                name: name_zh,
                source: "skin-slot".into(),
                description: String::new(),
                persona_id: cid.clone(),
                skins: vec![skin],
                preferred_skin_id: None,
                faction,
                ship_type: String::new(),
                rarity: String::new(),
                english_name: name_en,
                wiki_title,
                cv: String::new(),
            };
            if manifest.default_id.is_empty() {
                manifest.default_id = cid.clone();
            }
            if manifest.version == 0 {
                manifest.version = 1;
            }
            manifest.characters.push(ch);
            if stats.seen_new_chars.insert(cid.clone()) {
                stats.characters_added += 1;
            }
        }
        Ok(())
    })?;
    Ok(())
}

fn copy_dir_overwrite(src: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    copy_dir_recursive(src, dest)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    for ent in fs::read_dir(src).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let from = ent.path();
        let to = dest.join(ent.file_name());
        if from.is_dir() {
            fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            copy_dir_recursive(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn write_minimal_slot_zip(path: &Path) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let manifest = r#"{
  "format": "handaily-skin-slot",
  "format_version": 1,
  "character": {"id": "pad500572", "name_zh": "四万十", "name_en": "Shimanto", "faction": "重樱", "wiki_title": "四万十"},
  "skin": {
    "id": "pad500572-default",
    "name_zh": "默认皮肤",
    "is_default": true,
    "is_oath": false,
    "pet_model_id": "siwanshi",
    "kanmusu_dir": "",
    "has_pet": true,
    "has_kanmusu": false
  },
  "lines": {"path": "lines.json"},
  "packed_at": "2026-07-19T00:00:00Z"
}"#;
        zip.start_file("manifest.json", opts).unwrap();
        zip.write_all(manifest.as_bytes()).unwrap();
        zip.start_file("lines.json", opts).unwrap();
        zip.write_all(b"[]\n").unwrap();
        zip.start_file("avatar.jpg", opts).unwrap();
        zip.write_all(b"JPEGDATA").unwrap();
        zip.start_file("pet/siwanshi/siwanshi.skel", opts).unwrap();
        zip.write_all(b"skel").unwrap();
        zip.start_file("pet/siwanshi/siwanshi.atlas", opts).unwrap();
        zip.write_all(b"a.png\nsize:1,1\n").unwrap();
        zip.start_file("pet/siwanshi/siwanshi.png", opts).unwrap();
        zip.write_all(b"\x89PNG").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn import_single_slot_roundtrip() {
        let root = std::env::temp_dir().join(format!(
            "slot-pack-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let zip_path = root.join("pad500572__pad500572-default.slot.zip");
        write_minimal_slot_zip(&zip_path);
        let data_dir = root.join("data");
        fs::create_dir_all(&data_dir).unwrap();

        let result = import_from_zip(&data_dir, &zip_path, None).unwrap();
        assert_eq!(result.slots_imported, 1);
        assert_eq!(result.characters_added, 1);
        assert!(data_dir.join("pet-models/siwanshi/siwanshi.skel").is_file());
        assert!(data_dir.join("characters/avatars/pad500572.jpg").is_file());
        let man = character::load_manifest(&data_dir);
        let ch = man.characters.iter().find(|c| c.id == "pad500572").unwrap();
        assert_eq!(ch.name, "四万十");
        assert_eq!(ch.skins[0].model_id, "siwanshi");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn import_downloads_two_char_pack_if_present() {
        let zip = PathBuf::from(r"d:\Downloads\四万十，拉菲等2个角色导入包.zip");
        if !zip.is_file() {
            eprintln!("skip: downloads pack not present");
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "slot-pack-dl-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let data_dir = root.join("data");
        fs::create_dir_all(&data_dir).unwrap();
        let result = import_from_zip(&data_dir, &zip, None).unwrap();
        assert!(
            result.slots_imported >= 14,
            "expected ~15 slots, got {} fails={:?}",
            result.slots_imported,
            result.errors
        );
        assert!(data_dir.join("pet-models").is_dir());
        let man = character::load_manifest(&data_dir);
        assert!(man.characters.iter().any(|c| c.id == "pad500572"));
        assert!(man.characters.iter().any(|c| c.id == "p2b17e642"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn import_outer_multi_pack() {
        let root = std::env::temp_dir().join(format!(
            "slot-pack-multi-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let slot = root.join("one.slot.zip");
        write_minimal_slot_zip(&slot);
        let outer = root.join("bundle.zip");
        {
            let file = File::create(&outer).unwrap();
            let mut zip = ZipWriter::new(file);
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("catalog-meta.json", opts).unwrap();
            zip.write_all(br#"{"skin_ids":["pad500572-default"]}"#).unwrap();
            let mut f = File::open(&slot).unwrap();
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).unwrap();
            zip.start_file("pad500572__pad500572-default.slot.zip", opts)
                .unwrap();
            zip.write_all(&buf).unwrap();
            zip.finish().unwrap();
        }
        let data_dir = root.join("data");
        let result = import_from_zip(&data_dir, &outer, None).unwrap();
        assert_eq!(result.slots_imported, 1);
        assert!(data_dir.join("pet-models/siwanshi/siwanshi.skel").is_file());
        let _ = fs::remove_dir_all(&root);
    }
}
