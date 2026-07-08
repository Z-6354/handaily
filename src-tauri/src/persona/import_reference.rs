//! 人设参考文本导入（Wiki / 本地 BWIKI / 粘贴文本共用 AI 流水线）

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

use crate::persona::PersonaImportResult;
use crate::vault::VaultState;

const PERSONA_THINKING_MODEL_ERR: &str =
    "请先在设置中配置思考模型（用于解析参考文本并生成 Skill）";

static PERSONA_AI_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

fn persona_ai_lock() -> &'static tokio::sync::Mutex<()> {
    PERSONA_AI_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Agent 批量接口：若已有任务在执行则立即拒绝，避免并发 AI 调用卡死
pub fn try_acquire_persona_ai_batch() -> Result<tokio::sync::MutexGuard<'static, ()>, String> {
    persona_ai_lock()
        .try_lock()
        .map_err(|_| "已有性格 AI 任务进行中，请等待当前批次完成后再试".into())
}

async fn acquire_persona_ai() -> tokio::sync::MutexGuard<'static, ()> {
    persona_ai_lock().lock().await
}

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

async fn run_thinking_prompt(
    ctx: &ImportReferenceContext<'_>,
    prompt: String,
    options: crate::ai::adapters::ChatOptions,
) -> Result<String, String> {
    let prep = {
        let db = ctx.db.lock().map_err(|e| e.to_string())?;
        let config = crate::ai::AiConfig::load(&db, ctx.data_dir);
        let catalog = crate::ai::load_catalog(ctx.data_dir);
        crate::ai::PreparedThinkingChat::prepare(
            &config,
            &catalog,
            ctx.vault,
            &db,
            ctx.data_dir,
            prompt,
        )?
    };
    let Some(prep) = prep else {
        return Err(PERSONA_THINKING_MODEL_ERR.into());
    };
    prep.run_async_with_options(options).await
}

async fn preprocess_reference_text(
    ctx: &ImportReferenceContext<'_>,
    name: &str,
    source: &str,
    text: &str,
    from_wiki: bool,
    progress: &ImportReferenceProgress,
    _force_ai: bool,
) -> Result<crate::db::character_profiles::CharacterProfileData, String> {
    let local_wiki = if from_wiki {
        crate::persona_builder::try_profile_from_wiki_reference(text, name, source)
    } else {
        None
    };

    if let Some(ref profile) = local_wiki {
        if crate::persona_builder::profile_is_structured(profile) {
            emit_progress(
                ctx.app,
                "preprocess",
                "已本地解析 Wiki 资料，跳过 JSON 预处理…",
                progress.step(1),
                progress.total,
            );
            return Ok(profile.clone());
        }
        emit_progress(
            ctx.app,
            "preprocess",
            "本地解析不完整，继续调用思考模型补全性格字段…",
            progress.step(1),
            progress.total,
        );
    }

    let tiers = crate::persona_builder::preprocess_attempt_tiers(from_wiki, text.chars().count());
    let mut last_err = String::new();

    for (attempt, (mode, max_chars)) in tiers.iter().enumerate() {
        emit_progress(
            ctx.app,
            "preprocess",
            &if attempt == 0 {
                if from_wiki {
                    "正在解析 Wiki 资料为结构化 JSON…".to_string()
                } else {
                    "正在解析参考文本为结构化资料…".to_string()
                }
            } else {
                format!(
                    "JSON 不完整，正在压缩重试（{}/{}）…",
                    attempt + 1,
                    tiers.len()
                )
            },
            progress.step(1),
            progress.total,
        );

        let preprocess_prompt = crate::persona_builder::build_preprocess_prompt_limited(
            ctx.data_dir,
            name,
            source,
            text,
            *mode,
            Some(*max_chars),
        )?;

        let raw_profile = run_thinking_prompt(
            ctx,
            preprocess_prompt,
            crate::ai::adapters::ChatOptions::preprocess(),
        )
        .await?;

        let is_last = attempt + 1 == tiers.len();
        match crate::persona_builder::parse_profile_json_with_repair(&raw_profile, is_last) {
            Ok(profile) => return Ok(profile),
            Err(e) if !is_last && crate::persona_builder::is_truncated_profile_error(&e) => {
                last_err = e;
            }
            Err(e) => return Err(e),
        }
    }

    if let Some(profile) = local_wiki {
        if crate::persona_builder::profile_has_content(&profile) {
            emit_progress(
                ctx.app,
                "preprocess",
                "思考模型 JSON 不完整，已回退为本地 Wiki 解析结果…",
                progress.step(1),
                progress.total,
            );
            return Ok(profile);
        }
    }

    Err(if last_err.is_empty() {
        "AI 未能生成完整 JSON，请更换思考模型或缩短参考文本".into()
    } else {
        last_err
    })
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
    force_ai: bool,
) -> Result<PersonaImportResult, String> {
    let _ai_guard = acquire_persona_ai().await;
    import_from_reference_inner(
        ctx,
        persona_id,
        new_id,
        name,
        source,
        text,
        progress,
        from_wiki,
        force_ai,
    )
    .await
}

async fn import_from_reference_inner(
    ctx: &ImportReferenceContext<'_>,
    persona_id: Option<&str>,
    new_id: Option<&str>,
    name: Option<&str>,
    source: Option<&str>,
    text: &str,
    progress: ImportReferenceProgress,
    from_wiki: bool,
    force_ai: bool,
) -> Result<PersonaImportResult, String> {
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

    let profile = if args.is_update && !force_ai {
        if let Some(existing) = crate::persona::load_persona_profile(ctx.data_dir, &args.id) {
            if crate::persona_builder::profile_is_structured(&existing) {
                let merge_prompt = crate::persona_builder::build_merge_prompt_from_profile(
                    ctx.data_dir,
                    &existing,
                    text,
                )?;
                emit_progress(
                    ctx.app,
                    "preprocess",
                    "正在合并现有资料与新参考文本…",
                    progress.step(1),
                    progress.total,
                );
                let raw = run_thinking_prompt(
                    ctx,
                    merge_prompt,
                    crate::ai::adapters::ChatOptions::preprocess(),
                )
                .await?;
                crate::persona_builder::parse_profile_json_with_repair(&raw, true)?
            } else {
                preprocess_reference_text(
                    ctx,
                    display_name,
                    display_source,
                    text,
                    from_wiki,
                    &progress,
                    force_ai,
                )
                .await?
            }
        } else {
            preprocess_reference_text(
                ctx,
                display_name,
                display_source,
                text,
                from_wiki,
                &progress,
                force_ai,
            )
            .await?
        }
    } else {
        preprocess_reference_text(
            ctx,
            display_name,
            display_source,
            text,
            from_wiki,
            &progress,
            force_ai,
        )
        .await?
    };

    emit_progress(
        ctx.app,
        "skill",
        "正在生成 Skill 文档…",
        progress.step(2),
        progress.total,
    );

    let skill_prompt =
        crate::persona_builder::build_skill_prompt_from_profile(ctx.data_dir, &profile)?;
    let raw_skill = run_thinking_prompt(
        ctx,
        skill_prompt,
        crate::ai::adapters::ChatOptions::skill_generate(),
    )
    .await?;
    let skill_md = crate::persona_builder::strip_md_fence(&raw_skill);

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
        true,
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

/// 从 BWIKI 本地库快速导入人设（不调用 AI，直接写入 reference 文本）
pub fn import_persona_from_blhx_reference_fast(
    data_dir: &Path,
    blhx_path: &Path,
    wiki_title: &str,
    skip_existing: bool,
) -> Result<String, String> {
    use crate::db::character_profiles::CharacterProfileData;

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

    let skill_md = if crate::persona_builder::profile_is_structured(&profile) {
        let mut md = format!("# {display_name}\n\n");
        if !profile.introduction.is_empty() {
            md.push_str("## 介绍\n\n");
            md.push_str(&profile.introduction);
            md.push('\n');
        }
        if !profile.personality.is_empty() {
            md.push_str("\n## 性格\n\n");
            for p in &profile.personality {
                md.push_str(&format!("- {p}\n"));
            }
        }
        if !profile.speech_style.is_empty() {
            md.push_str("\n## 说话风格\n\n");
            md.push_str(&profile.speech_style);
            md.push('\n');
        }
        md
    } else {
        format!(
            "# {display_name}\n\n来源：碧蓝航线 BWIKI（本地库快速导入）\n\n{}\n",
            reference.chars().take(8000).collect::<String>()
        )
    };

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

    Ok(format!("已导入「{}」({})", meta.name, persona_id))
}

pub fn resolve_reference_for_persona(
    data_dir: &Path,
    persona_id: &str,
) -> Result<(String, String), String> {
    if let Some(text) = crate::persona_builder::load_persona_reference(data_dir, persona_id) {
        let manifest = crate::persona::load_manifest(data_dir);
        let name = manifest
            .personas
            .iter()
            .find(|p| p.id == persona_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| persona_id.to_string());
        return Ok((name, text));
    }
    let manifest = crate::persona::load_manifest(data_dir);
    let meta = manifest
        .personas
        .iter()
        .find(|p| p.id == persona_id)
        .ok_or_else(|| format!("未知人设: {persona_id}"))?;
    let blhx_path = resolve_blhx_db_path()?;
    let (display_name, text) = load_blhx_ship_reference(&blhx_path, &meta.name)?;
    let _ = crate::persona_builder::save_persona_reference(data_dir, persona_id, &text);
    Ok((display_name, text))
}

/// 强制用思考模型从 Wiki 参考文本重新生成结构化性格资料
pub async fn regenerate_persona_profile(
    ctx: &ImportReferenceContext<'_>,
    persona_id: &str,
) -> Result<PersonaImportResult, String> {
    let (name, text) = resolve_reference_for_persona(ctx.data_dir, persona_id)?;
    import_from_reference_inner(
        ctx,
        Some(persona_id),
        None,
        Some(name.as_str()),
        Some("碧蓝航线 BWIKI"),
        &text,
        ImportReferenceProgress::text(),
        true,
        true,
    )
    .await
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonaBatchRegenerateResult {
    pub ok: usize,
    pub skipped: usize,
    pub failed: usize,
    pub remaining: usize,
    pub message: String,
    pub last_id: Option<String>,
    pub last_error: Option<String>,
}

pub fn count_pending_regenerate(data_dir: &Path, only_missing: bool) -> usize {
    let manifest = crate::persona::load_manifest(data_dir);
    manifest
        .personas
        .iter()
        .filter(|p| !crate::persona::is_builtin_persona(&p.id))
        .filter(|p| {
            if !only_missing {
                return true;
            }
            crate::persona::load_persona_profile(data_dir, &p.id)
                .map(|prof| !crate::persona_builder::profile_is_structured(&prof))
                .unwrap_or(true)
        })
        .count()
}

pub async fn batch_regenerate_persona_profiles(
    ctx: &ImportReferenceContext<'_>,
    limit: usize,
    only_missing: bool,
) -> Result<PersonaBatchRegenerateResult, String> {
    let _batch_guard = try_acquire_persona_ai_batch()?;
    let manifest = crate::persona::load_manifest(ctx.data_dir);
    let mut pending: Vec<String> = Vec::new();
    for p in &manifest.personas {
        if crate::persona::is_builtin_persona(&p.id) {
            continue;
        }
        if only_missing {
            if let Some(profile) = crate::persona::load_persona_profile(ctx.data_dir, &p.id) {
                if crate::persona_builder::profile_is_structured(&profile) {
                    continue;
                }
            }
        }
        pending.push(p.id.clone());
    }
    let batch: Vec<_> = pending.into_iter().take(limit.max(1).min(50)).collect();
    let mut ok = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut last_id = None;
    let mut last_error = None;
    for (i, id) in batch.iter().enumerate() {
        crate::log::warn(format!(
            "性格 AI 批量更新 ({}/{})：{id}",
            i + 1,
            batch.len()
        ));
        last_id = Some(id.clone());
        match regenerate_persona_profile(ctx, id).await {
            Ok(_) => {
                ok += 1;
                last_error = None;
            }
            Err(e) if e.contains("跳过") => skipped += 1,
            Err(e) => {
                failed += 1;
                crate::log::warn(format!("性格 AI 更新失败 {id}: {e}"));
                last_error = Some(e);
            }
        }
    }
    let remaining = count_pending_regenerate(ctx.data_dir, only_missing);
    Ok(PersonaBatchRegenerateResult {
        ok,
        skipped,
        failed,
        remaining,
        last_id,
        last_error,
        message: format!(
            "本批处理 {} 人：AI 更新 {ok}，跳过 {skipped}，失败 {failed}，剩余约 {remaining} 人",
            batch.len()
        ),
    })
}
