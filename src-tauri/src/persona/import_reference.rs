//! [live2d-only] 人设参考文本导入（Wiki / 本地 BWIKI / 粘贴文本，无 AI）

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

use crate::db::character_profiles::CharacterProfileData;
use crate::live2d::VaultState;
use crate::persona::PersonaImportResult;

const PERSONA_IMPORT_STEP_TOTAL: u32 = 3;
const PERSONA_WIKI_IMPORT_STEP_TOTAL: u32 = 4;

pub struct ImportReferenceProgress {
    offset: u32,
    total: u32,
}

impl ImportReferenceProgress {
    pub fn text() -> Self {
        Self {
            offset: 0,
            total: PERSONA_IMPORT_STEP_TOTAL,
        }
    }

    pub fn wiki_pipeline() -> Self {
        Self {
            offset: 2,
            total: PERSONA_WIKI_IMPORT_STEP_TOTAL,
        }
    }

    pub fn character_wiki_pipeline() -> Self {
        Self {
            offset: 2,
            total: 6,
        }
    }

    fn step(&self, n: u32) -> u32 {
        self.offset + n
    }
}

pub struct ImportReferenceContext<'a> {
    pub data_dir: &'a Path,
    pub db: &'a Mutex<Connection>,
    pub vault: &'a VaultState,
    pub app: Option<&'a AppHandle>,
}

fn emit_progress(app: Option<&AppHandle>, step: &str, message: &str, step_index: u32, step_total: u32) {
    let _ = (app, step, message, step_index, step_total);
    if let Some(app) = app {
        let _ = app.emit(
            "persona-import-progress",
            &crate::persona::PersonaImportProgressEvent {
                step: step.to_string(),
                message: message.to_string(),
                step_index,
                step_total,
            },
        );
    }
}

fn preprocess_local(
    ctx: &ImportReferenceContext<'_>,
    name: &str,
    source: &str,
    text: &str,
    from_wiki: bool,
    progress: &ImportReferenceProgress,
) -> Result<CharacterProfileData, String> {
    emit_progress(
        ctx.app,
        "preprocess",
        if from_wiki {
            "正在本地解析 Wiki 资料…"
        } else {
            "正在解析参考文本…"
        },
        progress.step(1),
        progress.total,
    );

    if from_wiki {
        if let Some(profile) = crate::persona_builder::try_profile_from_wiki_reference(text, name, source) {
            if crate::persona_builder::profile_has_content(&profile) {
                return Ok(profile);
            }
        }
        return Err("无法从 Wiki 文本解析出结构化人设，请检查页面内容".into());
    }

    Ok(CharacterProfileData {
        name: name.to_string(),
        source: source.to_string(),
        introduction: text.chars().take(2000).collect(),
        ..Default::default()
    })
}

fn skill_md_from_profile(profile: &CharacterProfileData) -> String {
    let name = profile.name.trim();
    let title = if name.is_empty() { "桌宠角色" } else { name };
    let mut md = format!("# {title}\n\n");
    if !profile.source.trim().is_empty() {
        md.push_str(&format!("来源：{}\n\n", profile.source.trim()));
    }
    if !profile.introduction.is_empty() {
        md.push_str("## 介绍\n\n");
        md.push_str(profile.introduction.trim());
        md.push_str("\n\n");
    }
    if !profile.personality.is_empty() {
        md.push_str("## 性格\n\n");
        for p in &profile.personality {
            md.push_str(&format!("- {p}\n"));
        }
        md.push('\n');
    }
    if !profile.speech_style.is_empty() {
        md.push_str("## 说话风格\n\n");
        md.push_str(profile.speech_style.trim());
        md.push('\n');
    }
    if !profile.sample_lines.is_empty() {
        md.push_str("\n## 台词示例\n\n");
        for line in &profile.sample_lines {
            md.push_str(&format!("- {line}\n"));
        }
    }
    md
}

pub async fn import_from_reference(
    ctx: &ImportReferenceContext<'_>,
    persona_id: Option<&str>,
    new_id: Option<&str>,
    name: Option<&str>,
    source: Option<&str>,
    text: &str,
    progress: ImportReferenceProgress,
    from_wiki: bool,
    _force_ai: bool,
) -> Result<PersonaImportResult, String> {
    let _ = ctx.vault;
    let text = text.trim();
    if text.is_empty() {
        return Err("参考文本不能为空".into());
    }

    let args = crate::persona::resolve_reference_import(
        ctx.data_dir,
        persona_id,
        new_id,
        name,
        source,
    )?;

    let display_name = args
        .name_hint
        .as_deref()
        .filter(|n| !n.is_empty())
        .unwrap_or(&args.id);
    let display_source = args.source_hint.as_deref().unwrap_or("");

    let profile = preprocess_local(ctx, display_name, display_source, text, from_wiki, &progress)?;

    emit_progress(
        ctx.app,
        "skill",
        "正在写入 Skill 文档…",
        progress.step(2),
        progress.total,
    );
    let skill_md = skill_md_from_profile(&profile);

    emit_progress(
        ctx.app,
        "save",
        "正在写入人设文件…",
        progress.step(3),
        progress.total,
    );

    let meta = crate::persona_builder::save_processed_persona(
        ctx.data_dir,
        &args.id,
        profile,
        &skill_md,
        args.name_hint.as_deref(),
        args.source_hint.as_deref(),
        false,
    )?;

    let _ = crate::persona_builder::save_persona_reference(ctx.data_dir, &args.id, text);
    let _ = crate::character::sync_character_manifest_from_personas(ctx.data_dir);

    Ok(PersonaImportResult {
        imported_ids: vec![args.id.clone()],
        message: if args.is_update {
            format!("已解析参考文本并更新人设「{}」", meta.name)
        } else {
            format!("已解析参考文本并创建人设「{}」", meta.name)
        },
    })
}

pub fn wiki_step_total() -> u32 {
    PERSONA_WIKI_IMPORT_STEP_TOTAL
}

pub fn resolve_blhx_db_path() -> Result<std::path::PathBuf, String> {
    if let Ok(p) = std::env::var("BLHX_WIKI_DB_PATH") {
        let path = std::path::PathBuf::from(p.trim());
        if path.exists() {
            return Ok(path);
        }
        return Err(format!("BLHX_WIKI_DB_PATH 不存在: {}", path.display()));
    }
    if let Ok(cwd) = std::env::current_dir() {
        for base in [cwd.clone(), cwd.join(".."), cwd.join("../..")] {
            let candidate = base.join("mcp/blhx-wiki/data/blhx.sqlite");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let candidate = std::path::PathBuf::from(appdata)
            .join("HANDAILY")
            .join("mcp")
            .join("blhx-wiki")
            .join("data")
            .join("blhx.sqlite");
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("未找到本地 BWIKI 数据库；请设置 BLHX_WIKI_DB_PATH".into())
}

pub fn load_blhx_ship_reference(
    blhx_path: &Path,
    wiki_title: &str,
) -> Result<(String, String), String> {
    let conn = Connection::open(blhx_path).map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT display_name, persona_reference FROM ships WHERE wiki_title = ?1",
        [wiki_title],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(|_| format!("本地 BWIKI 库中未找到舰娘「{wiki_title}」"))
}

pub fn import_persona_from_blhx_reference_fast(
    data_dir: &Path,
    blhx_path: &Path,
    wiki_title: &str,
    skip_existing: bool,
) -> Result<String, String> {
    let (display_name, reference) = load_blhx_ship_reference(blhx_path, wiki_title)?;
    let reference = reference.trim();
    if reference.is_empty() {
        return Err(format!("「{display_name}」参考文本为空"));
    }

    let persona_id = crate::persona::suggest_persona_id(data_dir, &display_name)?;
    if skip_existing {
        let manifest = crate::persona::load_manifest(data_dir);
        if manifest
            .personas
            .iter()
            .any(|p| p.id == persona_id || p.name == display_name)
        {
            return Ok(format!("跳过已存在「{display_name}」"));
        }
    }
    if crate::persona::is_builtin_persona(&persona_id) {
        return Ok(format!("跳过内置「{display_name}」"));
    }

    let profile = crate::persona_builder::try_profile_from_wiki_reference(
        reference,
        &display_name,
        "碧蓝航线 BWIKI",
    )
    .unwrap_or(CharacterProfileData {
        name: display_name.clone(),
        source: "碧蓝航线 BWIKI".into(),
        introduction: reference.chars().take(400).collect(),
        ..Default::default()
    });

    let skill_md = skill_md_from_profile(&profile);

    let meta = crate::persona_builder::save_processed_persona(
        data_dir,
        &persona_id,
        profile,
        &skill_md,
        Some(display_name.as_str()),
        Some("碧蓝航线 BWIKI"),
        false,
    )?;

    let _ = crate::persona_builder::save_persona_reference(data_dir, &persona_id, reference);
    let _ = crate::character::sync_character_manifest_from_personas(data_dir);

    Ok(format!("已导入人设「{}」({})", meta.name, persona_id))
}

pub fn try_acquire_persona_ai_batch() -> Result<tokio::sync::MutexGuard<'static, ()>, String> {
    Err("纯桌宠分支不支持 AI 批量任务".into())
}
