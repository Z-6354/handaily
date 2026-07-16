use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::character::{self, CharacterManifest, CharacterMeta};
use crate::data_layout;
use crate::persona::{self, PersonaManifest};

use super::{
    faction_pack_name, PackMeta, MAIN_FACTIONS, META_FILENAME, PACK_CHESHIRE, PACK_FORMAT,
    PACK_FULL, PACK_OTHER, PACK_VERSION,
};

#[derive(Debug, Clone)]
struct PackSpec {
    file_stem: String,
    pack_id: String,
    pack_kind: String,
    pack_label: String,
}

#[derive(Debug, Clone)]
pub struct PackExportSummary {
    pub output_dir: PathBuf,
    pub packs: Vec<ExportedPackInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportedPackInfo {
    pub file_name: String,
    pub pack_label: String,
    pub character_count: usize,
    pub model_count: usize,
    pub size_bytes: u64,
}

pub fn export_all_packs(data_dir: &Path, output_dir: &Path) -> Result<PackExportSummary, String> {
    fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;

    let char_manifest = character::load_manifest(data_dir);
    let persona_manifest = persona::load_manifest(data_dir);
    let with_model: Vec<CharacterMeta> = char_manifest
        .characters
        .iter()
        .filter(|c| character_has_model(data_dir, c))
        .cloned()
        .collect();

    let mut specs = vec![PackSpec {
        file_stem: PACK_FULL.into(),
        pack_id: "full".into(),
        pack_kind: "full".into(),
        pack_label: PACK_FULL.into(),
    }];

    for faction in MAIN_FACTIONS {
        specs.push(PackSpec {
            file_stem: faction_pack_name(faction),
            pack_id: slugify_pack_id(faction),
            pack_kind: "faction".into(),
            pack_label: faction_pack_name(faction),
        });
    }

    specs.push(PackSpec {
        file_stem: PACK_OTHER.into(),
        pack_id: "other".into(),
        pack_kind: "other".into(),
        pack_label: PACK_OTHER.into(),
    });

    specs.push(PackSpec {
        file_stem: PACK_CHESHIRE.into(),
        pack_id: "cheshire".into(),
        pack_kind: "special".into(),
        pack_label: PACK_CHESHIRE.into(),
    });

    let mut packs = Vec::new();
    for spec in specs {
        let selected = select_characters(data_dir, &with_model, &spec);
        if selected.is_empty() {
            continue;
        }
        let info = write_pack(data_dir, output_dir, &persona_manifest, &spec, &selected)?;
        packs.push(info);
    }

    Ok(PackExportSummary {
        output_dir: output_dir.to_path_buf(),
        packs,
    })
}

fn select_characters(
    data_dir: &Path,
    all: &[CharacterMeta],
    spec: &PackSpec,
) -> Vec<CharacterMeta> {
    match spec.pack_kind.as_str() {
        "full" => all.to_vec(),
        "special" => all
            .iter()
            .filter(|c| c.id == "cheshire" || c.persona_id == "cheshire")
            .cloned()
            .collect(),
        "faction" => {
            let faction = spec
                .pack_label
                .strip_prefix("模型-")
                .and_then(|s| s.strip_suffix("阵营角色包"))
                .unwrap_or("");
            all.iter()
                .filter(|c| resolve_faction(data_dir, c) == faction)
                .cloned()
                .collect()
        }
        "other" => all
            .iter()
            .filter(|c| {
                c.id != "cheshire"
                    && c.persona_id != "cheshire"
                    && !MAIN_FACTIONS.contains(&resolve_faction(data_dir, c).as_str())
            })
            .cloned()
            .collect(),
        _ => Vec::new(),
    }
}

pub fn resolve_faction(data_dir: &Path, meta: &CharacterMeta) -> String {
    let trimmed = meta.faction.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    if let Some(profile) = persona::load_persona_profile(data_dir, &meta.persona_id) {
        if let Some(faction) = profile.extra.get("faction") {
            let s = faction.trim();
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    for kw in [
        "皇家", "白鹰", "重樱", "铁血", "北方联合", "维希教廷", "撒丁帝国", "自由鸢尾",
        "传颂之物", "哔哩哔哩", "维纳斯假期",
    ] {
        if meta.description.contains(kw) {
            return kw.to_string();
        }
    }
    "未分类".to_string()
}

fn character_has_model(data_dir: &Path, meta: &CharacterMeta) -> bool {
    meta.skins.iter().any(|s| {
        crate::pet::models::resolve_assets(data_dir, &s.model_id).is_ok()
    })
}

fn collect_model_ids(characters: &[CharacterMeta]) -> HashSet<String> {
    characters
        .iter()
        .flat_map(|c| c.skins.iter().map(|s| s.model_id.clone()))
        .collect()
}

fn write_pack(
    data_dir: &Path,
    output_dir: &Path,
    persona_manifest: &PersonaManifest,
    spec: &PackSpec,
    characters: &[CharacterMeta],
) -> Result<ExportedPackInfo, String> {
    let model_ids = collect_model_ids(characters);
    let persona_ids: HashSet<String> = characters.iter().map(|c| c.persona_id.clone()).collect();

    let staging = output_dir.join(format!(".staging-{}", spec.pack_id));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let char_manifest = CharacterManifest {
        version: char_manifest_version(data_dir),
        default_id: characters
            .first()
            .map(|c| c.id.clone())
            .unwrap_or_else(|| "cheshire".into()),
        characters: characters.to_vec(),
    };
    write_json(
        &data_layout::roster_staging_characters_manifest(&staging),
        &char_manifest,
    )?;

    let personas: PersonaManifest = PersonaManifest {
        version: persona_manifest.version,
        default_id: persona_manifest.default_id.clone(),
        personas: persona_manifest
            .personas
            .iter()
            .filter(|p| persona_ids.contains(&p.id))
            .cloned()
            .collect(),
    };
    write_json(
        &data_layout::roster_staging_personas_manifest(&staging),
        &personas,
    )?;

    let personas_dir = data_layout::roster_staging_personas_dir(&staging);
    fs::create_dir_all(&personas_dir).map_err(|e| e.to_string())?;
    for pid in &persona_ids {
        for ext in ["md", "json"] {
            let src = persona::personas_dir(data_dir).join(format!("{pid}.{ext}"));
            if src.is_file() {
                fs::copy(&src, personas_dir.join(format!("{pid}.{ext}")))
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    for model_id in model_ids
        .iter()
        .filter(|id| !crate::pet::models::is_builtin_model(id))
    {
        copy_model_tree(data_dir, &staging, model_id)?;
    }

    let exported_model_count = model_ids
        .iter()
        .filter(|id| !crate::pet::models::is_builtin_model(id))
        .count() as u32;

    let meta = PackMeta {
        format: PACK_FORMAT.into(),
        version: PACK_VERSION,
        pack_kind: spec.pack_kind.clone(),
        pack_id: spec.pack_id.clone(),
        pack_label: spec.pack_label.clone(),
        character_count: characters.len() as u32,
        model_count: exported_model_count,
        exported_at: Utc::now().to_rfc3339(),
    };
    write_json(&staging.join(META_FILENAME), &meta)?;

    let zip_name = format!("{}.zip", spec.file_stem);
    let zip_path = output_dir.join(&zip_name);
    if let Err(e) = zip_dir(&staging, &zip_path) {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;

    let size_bytes = fs::metadata(&zip_path).map_err(|e| e.to_string())?.len();
    Ok(ExportedPackInfo {
        file_name: zip_name,
        pack_label: spec.pack_label.clone(),
        character_count: characters.len(),
        model_count: exported_model_count as usize,
        size_bytes,
    })
}

fn char_manifest_version(data_dir: &Path) -> u32 {
    character::load_manifest(data_dir).version
}

fn copy_model_tree(data_dir: &Path, staging: &Path, model_id: &str) -> Result<(), String> {
    if crate::pet::models::is_builtin_model(model_id) {
        return Ok(());
    }
    let assets = crate::pet::models::resolve_assets(data_dir, model_id)?;
    if assets.use_file_src {
        let src_dir = crate::pet::models::models_dir(data_dir).join(model_id);
        let dest_dir = data_layout::roster_staging_pet_model_dir(staging, model_id);
        copy_dir_all(&src_dir, &dest_dir)?;
    } else {
        let src_dir = crate::data_layout::bundled_pet_model_dir(model_id);
        let dest_dir = data_layout::roster_staging_pet_model_dir(staging, model_id);
        copy_dir_all(&src_dir, &dest_dir)?;
    }

    let meta_src = data_layout::pet_meta_file(data_dir, model_id);
    let meta_src = if meta_src.is_file() {
        meta_src
    } else {
        data_layout::bundled_pet_meta_file(model_id)
    };
    if meta_src.is_file() {
        let meta_dest = data_layout::roster_staging_pet_meta_file(staging, model_id);
        if let Some(parent) = meta_dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(&meta_src, &meta_dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("目录不存在: {}", src.display()));
    }
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let dest_path = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

fn zip_dir(src: &Path, dest_zip: &Path) -> Result<(), String> {
    let file = File::create(dest_zip).map_err(|e| e.to_string())?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    zip_dir_recursive(src, src, &mut writer, options)?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn zip_dir_recursive(
    root: &Path,
    current: &Path,
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            zip_dir_recursive(root, &path, writer, options)?;
            continue;
        }
        let name = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        writer.start_file(name, options).map_err(|e| e.to_string())?;
        let mut f = File::open(&path).map_err(|e| e.to_string())?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
        writer.write_all(&buffer).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn slugify_pack_id(label: &str) -> String {
    match label {
        "皇家" => "huangjia".into(),
        "白鹰" => "baiying".into(),
        "重樱" => "zhongying".into(),
        "铁血" => "tiexue".into(),
        other => other.to_string(),
    }
}
