//! Live2D-only `#[tauri::command]` 处理器子集

use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, State};

use crate::state::AppState;

// ── 应用 ──

#[tauri::command]
pub fn app_ping() -> String {
    "xiaohan-pet v0.1.0".to_string()
}

#[tauri::command]
pub fn app_memory_stats(st: State<'_, Arc<AppState>>) -> Result<crate::character::CharacterMemoryStats, String> {
    Ok(crate::character::memory_stats(st.data_dir()))
}

#[tauri::command]
pub async fn app_get_data_path(st: State<'_, Arc<AppState>>) -> Result<String, String> {
    Ok(st.db_path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn app_exit(app: tauri::AppHandle, st: State<'_, Arc<AppState>>) {
    crate::request_app_exit(&app, st.inner());
}

#[tauri::command]
pub async fn app_get_personas_path(st: State<'_, Arc<AppState>>) -> Result<String, String> {
    Ok(crate::persona::personas_dir(st.data_dir())
        .to_string_lossy()
        .into_owned())
}

// ── 设置 & 自启动 ──

#[tauri::command]
pub async fn settings_get(
    st: State<'_, Arc<AppState>>,
    key: String,
) -> Result<Option<String>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::db::get_setting(&db, &key))
}

#[tauri::command]
pub async fn settings_save(
    st: State<'_, Arc<AppState>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::db::set_setting(&db, &key, &value).map_err(|e| e.to_string())?;
    if key == "idle_threshold_secs" {
        if let Ok(secs) = value.parse::<u64>() {
            st.set_idle_threshold_secs(secs);
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct AutostartStatusPayload {
    pub enabled: bool,
    pub supported: bool,
}

#[tauri::command]
pub async fn autostart_get_status(
    st: State<'_, Arc<AppState>>,
) -> Result<AutostartStatusPayload, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(AutostartStatusPayload {
        enabled: crate::system::autostart::is_enabled(&db),
        supported: crate::system::autostart::platform_supported(),
    })
}

#[tauri::command]
pub async fn autostart_set_enabled(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::system::autostart::set_enabled(&app, &db, enabled)
}

// ── 人设导入进度 ──

const PERSONA_WIKI_IMPORT_STEP_TOTAL: u32 = 4;
const CHARACTER_WIKI_IMPORT_STEP_TOTAL: u32 = 6;

fn emit_persona_import_progress(
    app: &tauri::AppHandle,
    step: &str,
    message: &str,
    step_index: u32,
    step_total: u32,
) {
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

// ── 人设 ──

#[tauri::command]
pub async fn persona_list(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::persona::PersonaInfo>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::persona::list_personas(st.data_dir(), &db))
}

#[tauri::command]
pub async fn persona_set_active(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    persona_id: String,
) -> Result<(), String> {
    let data_dir = st.data_dir();
    let model_id = {
        let db = crate::db::lock_conn(&st.db)?;
        let manifest = crate::persona::load_manifest(data_dir);
        crate::persona::set_active_persona_id(&db, &manifest, &persona_id)?;
        crate::character::sync_from_persona(data_dir, &db, &persona_id);
        crate::pet::models::active_model_id(&db)
    };
    crate::pet::set_active_model(&app, st.inner().clone(), &model_id)
}

#[tauri::command]
pub async fn persona_get_detail(
    st: State<'_, Arc<AppState>>,
    persona_id: String,
) -> Result<crate::persona::PersonaDetail, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona::get_persona_detail(st.data_dir(), &db, &persona_id)
}

#[tauri::command]
pub async fn persona_import(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    files: Vec<crate::persona::PersonaImportFile>,
) -> Result<crate::persona::PersonaImportResult, String> {
    let data_dir = st.data_dir();
    let items = crate::persona::parse_import_files(files)?;
    let mut imported_ids = Vec::new();
    let mut last_message = String::new();

    for (id, text) in items {
        let exists = crate::persona::load_manifest(data_dir)
            .personas
            .iter()
            .any(|p| p.id == id);
        let db = crate::db::lock_conn(&st.db)?;
        let ctx = crate::persona::import_reference::ImportReferenceContext {
            data_dir,
            db: &st.db,
            vault: &st.vault,
            app: Some(&app),
        };
        drop(db);
        let result = crate::persona::import_reference::import_from_reference(
            &ctx,
            if exists {
                Some(id.as_str())
            } else {
                None
            },
            if exists { None } else { Some(id.as_str()) },
            None,
            None,
            &text,
            crate::persona::import_reference::ImportReferenceProgress::text(),
            false,
            false,
        )
        .await?;
        imported_ids.push(id);
        last_message = result.message;
    }

    imported_ids.sort();
    let message = if imported_ids.len() == 1 {
        last_message
    } else {
        format!("已 AI 处理并导入 {} 个人设", imported_ids.len())
    };

    Ok(crate::persona::PersonaImportResult {
        imported_ids,
        message,
    })
}

#[tauri::command]
pub async fn persona_import_text(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    persona_id: Option<String>,
    id: Option<String>,
    name: Option<String>,
    text: String,
    from_wiki: Option<bool>,
) -> Result<crate::persona::PersonaImportResult, String> {
    let data_dir = st.data_dir();
    let ctx = crate::persona::import_reference::ImportReferenceContext {
        data_dir,
        db: &st.db,
        vault: &st.vault,
        app: Some(&app),
    };
    crate::persona::import_reference::import_from_reference(
        &ctx,
        persona_id.as_deref(),
        id.as_deref(),
        name.as_deref(),
        None,
        &text,
        crate::persona::import_reference::ImportReferenceProgress::text(),
        from_wiki.unwrap_or(false),
        false,
    )
    .await
}

#[tauri::command]
pub async fn persona_import_blhx_local(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    wiki_title: String,
    persona_id: Option<String>,
    id: Option<String>,
    name: Option<String>,
) -> Result<crate::persona::PersonaImportResult, String> {
    let blhx_path = crate::persona::import_reference::resolve_blhx_db_path()?;
    let (display_name, text) =
        crate::persona::import_reference::load_blhx_ship_reference(&blhx_path, wiki_title.trim())?;
    let data_dir = st.data_dir();
    let resolved_name = name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(display_name.as_str());
    let new_id_owned = if persona_id.is_some() {
        None
    } else if id.as_deref().map(str::trim).is_some_and(|s| !s.is_empty()) {
        id.as_deref().map(str::trim).map(str::to_string)
    } else {
        Some(crate::persona::suggest_persona_id(data_dir, resolved_name)?)
    };

    emit_persona_import_progress(
        &app,
        "parse",
        "已从本地 BWIKI 库读取资料…",
        1,
        crate::persona::import_reference::wiki_step_total(),
    );

    let db = crate::db::lock_conn(&st.db)?;
    drop(db);
    let ctx = crate::persona::import_reference::ImportReferenceContext {
        data_dir,
        db: &st.db,
        vault: &st.vault,
        app: Some(&app),
    };
    crate::persona::import_reference::import_from_reference(
        &ctx,
        persona_id.as_deref(),
        new_id_owned.as_deref(),
        Some(resolved_name),
        Some("碧蓝航线 BWIKI"),
        &text,
        crate::persona::import_reference::ImportReferenceProgress::wiki_pipeline(),
        true,
        false,
    )
    .await
}

#[tauri::command]
pub async fn persona_import_wiki(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    url: Option<String>,
    wiki_title: Option<String>,
    persona_id: Option<String>,
    id: Option<String>,
    name: Option<String>,
) -> Result<crate::persona::PersonaImportResult, String> {
    let fetch_url =
        crate::pet::wiki_scrape::resolve_wiki_fetch_url(url.as_deref(), wiki_title.as_deref())?;

    emit_persona_import_progress(
        &app,
        "fetch",
        "正在爬取 Wiki 页面…",
        1,
        PERSONA_WIKI_IMPORT_STEP_TOTAL,
    );
    let html = crate::pet::wiki_scrape::fetch_wiki_page(&fetch_url).await?;

    emit_persona_import_progress(
        &app,
        "parse",
        "正在清洗并整理 Wiki 资料…",
        2,
        PERSONA_WIKI_IMPORT_STEP_TOTAL,
    );
    let extract = crate::pet::wiki_scrape::extract_persona_reference(&html, &fetch_url)?;

    let data_dir = st.data_dir();
    let resolved_name = name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .or(extract.name_hint.as_deref());
    let resolved_source = extract.source_hint.as_deref();

    let new_id_owned = if persona_id.is_some() {
        None
    } else if id.as_deref().map(str::trim).is_some_and(|s| !s.is_empty()) {
        id.as_deref().map(str::trim).map(str::to_string)
    } else if let Some(n) = resolved_name {
        Some(crate::persona::suggest_persona_id(data_dir, n)?)
    } else {
        return Err("无法从 Wiki 识别角色名，请填写显示名称或人设 ID".into());
    };

    let db = crate::db::lock_conn(&st.db)?;
    drop(db);
    let ctx = crate::persona::import_reference::ImportReferenceContext {
        data_dir,
        db: &st.db,
        vault: &st.vault,
        app: Some(&app),
    };
    let result = crate::persona::import_reference::import_from_reference(
        &ctx,
        persona_id.as_deref(),
        new_id_owned.as_deref(),
        resolved_name,
        resolved_source,
        &extract.text,
        crate::persona::import_reference::ImportReferenceProgress::wiki_pipeline(),
        true,
        false,
    )
    .await?;

    let wiki_title = resolved_name.unwrap_or("");
    if !wiki_title.is_empty() {
        let model_id = {
            let db = crate::db::lock_conn(&st.db)?;
            let persona_id = result.imported_ids.first().cloned().unwrap_or_default();
            let manifest = crate::character::load_manifest(data_dir);
            manifest
                .characters
                .iter()
                .find(|c| c.persona_id == persona_id)
                .and_then(|c| {
                    c.skins
                        .iter()
                        .find(|s| s.default)
                        .or_else(|| c.skins.first())
                        .map(|s| s.model_id.clone())
                })
                .unwrap_or_else(|| crate::pet::models::active_model_id(&db))
        };
        if !model_id.trim().is_empty() {
            if let Ok(count) = crate::pet::wiki_scrape::import_wiki_lines_if_needed(
                &app,
                st.inner(),
                &model_id,
                &html,
            )
            .await
            {
                if count > 0 {
                    let _ = app.emit("pet-model-meta-updated", model_id.clone());
                    let active = {
                        let db = crate::db::lock_conn(&st.db)?;
                        crate::pet::models::active_model_id(&db)
                    };
                    if active == model_id {
                        crate::pet::nudge_pet_animations(&app);
                    }
                }
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn persona_update(
    st: State<'_, Arc<AppState>>,
    persona_id: String,
    input: crate::persona::PersonaUpdateInput,
) -> Result<(), String> {
    crate::persona::update_persona(st.data_dir(), &persona_id, &input)
}

#[tauri::command]
pub async fn persona_delete(
    st: State<'_, Arc<AppState>>,
    persona_id: String,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona::delete_persona(st.data_dir(), &db, &persona_id)
}

// ── 人物（性格 + 皮肤 → 模型）──

#[tauri::command]
pub async fn character_import_wiki(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    character_id: String,
    wiki_title: Option<String>,
    url: Option<String>,
) -> Result<crate::character::CharacterWikiImportResult, String> {
    let fetch_url =
        crate::pet::wiki_scrape::resolve_wiki_fetch_url(url.as_deref(), wiki_title.as_deref())?;

    emit_persona_import_progress(
        &app,
        "fetch",
        "正在爬取 Wiki 页面…",
        1,
        CHARACTER_WIKI_IMPORT_STEP_TOTAL,
    );
    let html = crate::pet::wiki_scrape::fetch_wiki_page(&fetch_url).await?;

    emit_persona_import_progress(
        &app,
        "parse",
        "正在筛选性格、简介与台词资料…",
        2,
        CHARACTER_WIKI_IMPORT_STEP_TOTAL,
    );
    let extract = crate::pet::wiki_scrape::extract_persona_reference(&html, &fetch_url)?;

    let data_dir = st.data_dir();
    let (persona_id, model_id) = {
        let db = crate::db::lock_conn(&st.db)?;
        let meta = crate::character::find_character_meta(data_dir, &character_id)?;
        let detail = crate::character::get_character_detail(data_dir, &db, &character_id)?;
        (meta.persona_id, detail.active_model_id)
    };

    let resolved_name = wiki_title
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .or(extract.name_hint.as_deref());

    let ctx = crate::persona::import_reference::ImportReferenceContext {
        data_dir,
        db: &st.db,
        vault: &st.vault,
        app: Some(&app),
    };
    let persona_result = crate::persona::import_reference::import_from_reference(
        &ctx,
        Some(&persona_id),
        None,
        resolved_name,
        extract.source_hint.as_deref(),
        &extract.text,
        crate::persona::import_reference::ImportReferenceProgress::character_wiki_pipeline(),
        true,
        false,
    )
    .await?;

    let mut lines_imported = 0u32;
    if !model_id.trim().is_empty() {
        emit_persona_import_progress(
            &app,
            "lines",
            "正在提取并写入舰船台词…",
            6,
            CHARACTER_WIKI_IMPORT_STEP_TOTAL,
        );
        match crate::pet::wiki_scrape::extract_wiki_lines_from_html(
            &app,
            st.inner(),
            &model_id,
            &html,
        )
        .await
        {
            Ok(lines) if !lines.is_empty() => {
                let db = crate::db::lock_conn(&st.db)?;
                lines_imported = crate::pet::wiki_scrape::save_lines_to_model(
                    data_dir,
                    &db,
                    &model_id,
                    lines,
                    false,
                    true,
                )? as u32;
                let active = crate::pet::models::active_model_id(&db);
                drop(db);
                let _ = app.emit("pet-model-meta-updated", model_id.clone());
                if active == model_id {
                    crate::pet::nudge_pet_animations(&app);
                }
            }
            Ok(_) => {}
            Err(_) => {
                // 台词导入失败不阻断性格资料更新
            }
        }
    }

    let message = if lines_imported > 0 {
        format!(
            "{}；已导入 {} 条台词到当前皮肤模型",
            persona_result.message, lines_imported
        )
    } else {
        persona_result.message.clone()
    };

    Ok(crate::character::CharacterWikiImportResult {
        message,
        lines_imported,
        persona_id,
    })
}

#[tauri::command]
pub async fn characters_list(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::character::CharacterInfo>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::character::list_characters(st.data_dir(), &db))
}

#[tauri::command]
pub async fn characters_pet_menu_skins(
    st: State<'_, Arc<AppState>>,
) -> Result<crate::character::PetMenuSkinsPayload, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::character::list_pet_menu_skins(st.data_dir(), &db)
}

#[tauri::command]
pub async fn characters_pet_menu_skins_for(
    st: State<'_, Arc<AppState>>,
    character_id: String,
) -> Result<crate::character::PetMenuSkinsPayload, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::character::list_pet_menu_skins_for_character(st.data_dir(), &db, &character_id)
}

#[tauri::command]
pub async fn characters_pet_menu_favorites(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::character::CharacterBrief>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::character::list_pet_menu_favorite_characters(
        st.data_dir(),
        &db,
    ))
}

#[tauri::command]
pub async fn pet_resolve_model_preload_config(
    st: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<crate::pet::PetConfigPayload, String> {
    crate::pet::get_model_preload_config(st.data_dir(), &model_id)
}

#[tauri::command]
pub async fn characters_list_brief(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::character::CharacterBrief>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::character::list_characters_brief(st.data_dir(), &db))
}

#[tauri::command]
pub async fn characters_list_page(
    st: State<'_, Arc<AppState>>,
    offset: usize,
    limit: usize,
    query: Option<String>,
    favorites_only: Option<bool>,
    favorite_ids: Option<Vec<String>>,
) -> Result<crate::character::CharacterListPage, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::character::list_characters_page(
        st.data_dir(),
        &db,
        offset,
        limit,
        query.as_deref(),
        favorites_only.unwrap_or(false),
        favorite_ids.as_deref().unwrap_or(&[]),
    ))
}

#[tauri::command]
pub async fn characters_remove_skin(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    character_id: String,
    skin_id: String,
    delete_model_files: Option<bool>,
) -> Result<(), String> {
    let data_dir = st.data_dir();
    let reload_model = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::character::remove_character_skin(
            data_dir,
            &db,
            &character_id,
            &skin_id,
            delete_model_files.unwrap_or(true),
        )?
    };
    if let Some(model_id) = reload_model {
        crate::pet::set_active_model(&app, st.inner().clone(), &model_id)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn characters_cache_avatar(
    st: State<'_, Arc<AppState>>,
    character_id: String,
) -> Result<Option<String>, String> {
    let data_dir = st.data_dir();
    let meta = crate::character::find_character_meta(data_dir, &character_id)?;
    crate::character::avatar::ensure_avatar_cached(data_dir, &character_id, &meta.name).await
}

#[tauri::command]
pub async fn characters_cache_avatars_batch(
    st: State<'_, Arc<AppState>>,
    character_ids: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let data_dir = st.data_dir();
    Ok(crate::character::avatar::ensure_avatars_cached_batch(data_dir, &character_ids).await)
}

#[tauri::command]
pub fn characters_read_avatar(
    st: State<'_, Arc<AppState>>,
    character_id: String,
) -> Result<Option<String>, String> {
    crate::character::avatar::read_avatar_base64(st.data_dir(), &character_id)
}

#[tauri::command]
pub async fn characters_skins_page(
    st: State<'_, Arc<AppState>>,
    character_id: String,
    offset: usize,
    limit: usize,
) -> Result<crate::character::CharacterSkinsPage, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::character::list_character_skins_page(st.data_dir(), &db, &character_id, offset, limit)
}

#[tauri::command]
pub async fn characters_get_detail(
    st: State<'_, Arc<AppState>>,
    character_id: String,
) -> Result<crate::character::CharacterDetail, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::character::get_character_detail(st.data_dir(), &db, &character_id)
}

#[tauri::command]
pub async fn characters_set_active(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    character_id: String,
) -> Result<String, String> {
    let data_dir = st.data_dir();
    let model_id = {
        let db = crate::db::lock_conn(&st.db)?;
        let manifest = crate::persona::load_manifest(data_dir);
        crate::character::set_active_character(data_dir, &db, &manifest, &character_id)?;
        crate::pet::models::active_model_id(&db)
    };
    crate::pet::set_active_model(&app, st.inner().clone(), &model_id)?;
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::pet::models::active_model_id(&db))
}

#[tauri::command]
pub async fn characters_set_skin(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    character_id: String,
    skin_id: String,
) -> Result<String, String> {
    let data_dir = st.data_dir();
    let model_id = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::character::select_character_skin(data_dir, &db, &character_id, &skin_id)?
    };
    crate::pet::set_active_model(&app, st.inner().clone(), &model_id)?;
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::pet::models::active_model_id(&db))
}

#[tauri::command]
pub async fn characters_import_avatars_batch(
    st: State<'_, Arc<AppState>>,
    limit: Option<usize>,
    skip_existing: Option<bool>,
    sync_tags: Option<bool>,
) -> Result<crate::character::avatar::AvatarImportResult, String> {
    let data_dir = st.data_dir().to_path_buf();
    let limit = limit.unwrap_or(50).clamp(1, 200);
    crate::character::avatar::run_avatar_import_default(
        &data_dir,
        limit,
        skip_existing.unwrap_or(true),
        sync_tags.unwrap_or(true),
    )
    .await
}

#[tauri::command]
pub async fn characters_import_live2d(
    st: State<'_, Arc<AppState>>,
    character_id: String,
    plan_path: Option<String>,
) -> Result<crate::live2d_import::Live2dImportResult, String> {
    let data_dir = st.data_dir();
    let plan = crate::live2d_import::resolve_plan_path(
        plan_path.as_deref().map(std::path::Path::new),
    )?;
    let live2d_root = crate::live2d_import::resolve_live2d_root();
    crate::live2d_import::run_live2d_import_for_character(
        data_dir,
        &plan,
        &live2d_root,
        &character_id,
    )
}

#[tauri::command]
pub async fn live2d_import_batch(
    st: State<'_, Arc<AppState>>,
    limit: Option<usize>,
    plan_path: Option<String>,
) -> Result<crate::live2d_import::Live2dImportResult, String> {
    let data_dir = st.data_dir().to_path_buf();
    let plan = crate::live2d_import::resolve_plan_path(
        plan_path.as_deref().map(std::path::Path::new),
    )?;
    let live2d_root = crate::live2d_import::resolve_live2d_root();
    let limit = limit.unwrap_or(50).clamp(1, 200);
    crate::live2d_import::run_live2d_import(
        &data_dir,
        &plan,
        &live2d_root,
        limit,
        false,
    )
}

// ── 桌宠 ──

#[tauri::command]
pub async fn pet_get_wiki_bulk_import_progress(
    app: tauri::AppHandle,
) -> Option<crate::pet::lines_import::PetWikiBulkImportProgress> {
    crate::pet::wiki_bulk_import_progress(&app)
}

#[tauri::command]
pub async fn pet_start_wiki_bulk_import(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
) -> Result<crate::pet::lines_import::PetWikiBulkImportStartResult, String> {
    Ok(crate::pet::start_wiki_bulk_import(app, st.inner().clone()))
}

#[tauri::command]
pub async fn pet_pause_wiki_bulk_import(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(crate::pet::pause_wiki_bulk_import(&app))
}

#[tauri::command]
pub async fn pet_resume_wiki_bulk_import(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(crate::pet::resume_wiki_bulk_import(&app))
}

#[tauri::command]
pub async fn pet_stop_wiki_bulk_import(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(crate::pet::stop_wiki_bulk_import(&app))
}

#[tauri::command]
pub async fn pet_get_status(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
) -> Result<crate::pet::PetStatusPayload, String> {
    crate::pet::status(&app, &st)
}

#[tauri::command]
pub async fn pet_show(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    crate::pet::show_pet(&app, st.inner())?;
    crate::pet::ensure_remark_scheduler(app.clone(), st.inner().clone());
    Ok(())
}

#[tauri::command]
pub async fn pet_hide(
    app: tauri::AppHandle,
    destroy: Option<bool>,
) -> Result<(), String> {
    crate::pet::hide_pet(&app, destroy.unwrap_or(false))
}

#[tauri::command]
pub async fn pet_set_enabled(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    crate::pet::set_enabled(&app, st.inner().clone(), enabled)
}

#[tauri::command]
pub async fn pet_save_position(
    st: State<'_, Arc<AppState>>,
    x: i32,
    y: i32,
    win_width: Option<i32>,
    win_height: Option<i32>,
) -> Result<crate::pet::PetPoint, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::save_position(&db, x, y, win_width, win_height)
}

#[tauri::command]
pub fn pet_get_screen_bounds() -> crate::pet::PetScreenBounds {
    crate::pet::screen_bounds()
}

#[tauri::command]
pub async fn pet_open_main(
    app: tauri::AppHandle,
    page: Option<String>,
) -> Result<(), String> {
    crate::pet::show_main_window(&app, page.as_deref())
}

#[tauri::command]
pub fn pet_menu_show(app: tauri::AppHandle, x: i32, y: i32) -> Result<(), String> {
    crate::pet::show_pet_menu(&app, x, y)
}

#[tauri::command]
pub fn pet_menu_open_at_cursor(app: tauri::AppHandle) -> Result<bool, String> {
    crate::pet::toggle_pet_menu_at_cursor(&app)
}

#[tauri::command]
pub fn pet_menu_toggle_at_cursor(app: tauri::AppHandle) -> Result<bool, String> {
    crate::pet::toggle_pet_menu_at_cursor(&app)
}

#[tauri::command]
pub fn pet_poll_menu_dismiss(app: tauri::AppHandle) -> crate::pet::PetMenuDismissPoll {
    crate::pet::poll_menu_dismiss(&app)
}

#[tauri::command]
pub fn pet_is_right_mouse_down() -> bool {
    crate::pet::is_right_mouse_down()
}

#[tauri::command]
pub fn pet_is_left_mouse_down() -> bool {
    crate::pet::is_left_mouse_down()
}

#[tauri::command]
pub fn pet_menu_contains_cursor(app: tauri::AppHandle) -> bool {
    crate::pet::is_cursor_over_menu_window(&app)
}

#[tauri::command]
pub fn pet_menu_hide(app: tauri::AppHandle) -> Result<(), String> {
    crate::pet::hide_pet_menu(&app)
}

#[tauri::command]
pub fn pet_menu_sync_z_order(app: tauri::AppHandle) -> Result<(), String> {
    crate::pet::sync_menu_z_order_if_visible(&app);
    Ok(())
}

#[tauri::command]
pub fn pet_menu_toggle(app: tauri::AppHandle, x: i32, y: i32) -> Result<(), String> {
    crate::pet::toggle_pet_menu(&app, x, y)
}

#[tauri::command]
pub fn pet_enter_edit_bounds(app: tauri::AppHandle) -> Result<(), String> {
    crate::pet::enter_pet_edit_bounds(&app)
}

#[tauri::command]
pub async fn pet_reload(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    crate::pet::reload_pet(&app, st.inner())
}

#[tauri::command]
pub fn pet_nudge(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    crate::pet::nudge_pet(&app, st.inner());
    Ok(())
}

#[tauri::command]
pub async fn pet_await_spine_ready(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    expected_model_id: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<bool, String> {
    Ok(crate::pet::await_spine_ready(
        &app,
        st.inner(),
        expected_model_id,
        timeout_ms.unwrap_or(120_000),
    )
    .await)
}

#[tauri::command]
pub fn pet_refresh_animations(app: tauri::AppHandle) -> Result<(), String> {
    crate::pet::nudge_pet_animations(&app);
    Ok(())
}

#[tauri::command]
pub fn pet_preview_animation(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    animation: String,
    loop_anim: Option<bool>,
) -> Result<(), String> {
    crate::pet::preview_animation(&app, st.inner(), &animation, loop_anim.unwrap_or(false))
}

#[tauri::command]
pub async fn pet_menu_switch_skin(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    character_id: String,
    skin_id: String,
    timeout_ms: Option<u64>,
) -> Result<String, String> {
    crate::pet::menu_switch_skin(
        &app,
        st.inner().clone(),
        &character_id,
        &skin_id,
        timeout_ms.unwrap_or(30_000),
    )
    .await
}

#[tauri::command]
pub async fn pet_menu_switch_character(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    character_id: String,
    timeout_ms: Option<u64>,
) -> Result<String, String> {
    crate::pet::menu_switch_character(
        &app,
        st.inner().clone(),
        &character_id,
        timeout_ms.unwrap_or(30_000),
    )
    .await
}

#[tauri::command]
pub fn pet_confirm_switch(
    app: tauri::AppHandle,
    switch_id: u64,
    model_id: String,
) {
    crate::pet::confirm_switch(&app, switch_id, &model_id);
}

#[tauri::command]
pub fn pet_mark_spine_ready(
    app: tauri::AppHandle,
    model_id: Option<String>,
) {
    if let Some(id) = model_id.filter(|s| !s.trim().is_empty()) {
        crate::pet::mark_spine_ready(&app, &id);
    } else {
        crate::pet::clear_spine_ready(&app);
    }
}

#[tauri::command]
pub fn pet_clear_spine_ready(app: tauri::AppHandle) {
    crate::pet::clear_spine_ready(&app);
}

#[tauri::command]
pub async fn pet_get_bubble_enabled(
    st: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::pet::is_bubble_enabled(&db))
}

#[tauri::command]
pub async fn pet_set_bubble_enabled(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::set_bubble_enabled(&db, enabled)?;
    let _ = app.emit_to(crate::pet::PET_LABEL, "pet-bubble-enabled-changed", enabled);
    crate::pet::emit_pet_status_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn pet_get_model_status(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<crate::pet::PetStatusPayload, String> {
    crate::pet::model_status(&app, st.inner(), &model_id)
}

#[tauri::command]
pub async fn pet_get_config(
    st: State<'_, Arc<AppState>>,
) -> Result<crate::pet::PetConfigPayload, String> {
    crate::pet::get_config(&st)
}

#[tauri::command]
pub async fn pet_list_models(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::pet::models::PetModelInfo>, String> {
    crate::pet::models::list_models(st.data_dir())
}

#[tauri::command]
pub async fn pet_set_model(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    crate::pet::set_active_model(&app, st.inner().clone(), &model_id)
}

#[tauri::command]
pub async fn pet_save_model_settings(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    model_id: String,
    power_mode: Option<String>,
    remark_interval_sec: Option<i64>,
    apply_live: Option<bool>,
) -> Result<(), String> {
    {
        let db = crate::db::lock_conn(&st.db)?;
        crate::pet::models::save_model_display_settings(
            st.data_dir(),
            &db,
            &model_id,
            power_mode,
            remark_interval_sec,
        )?;
        let active = crate::pet::models::active_model_id(&db);
        if apply_live.unwrap_or(true) && active == model_id {
            drop(db);
            crate::pet::nudge_pet_animations(&app);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_set_scale(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    scale: f64,
) -> Result<(), String> {
    {
        let db = crate::db::lock_conn(&st.db)?;
        crate::pet::set_scale(&db, scale)?;
    }
    let clamped = scale.clamp(0.4, 1.5);
    crate::pet::nudge_pet_scale(&app, clamped);
    Ok(())
}

#[tauri::command]
pub async fn pet_pick_model_folder(window: tauri::Window) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    window
        .run_on_main_thread({
            let window = window.clone();
            move || {
                let picked = rfd::FileDialog::new()
                    .set_title("选择 Spine 模型文件夹")
                    .set_parent(&window)
                    .pick_folder()
                    .map(|p| p.to_string_lossy().to_string());
                let _ = tx.send(picked);
            }
        })
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "文件夹选择已中断".to_string())
}

#[tauri::command]
pub async fn pet_stage_folder_import(
    st: State<'_, Arc<AppState>>,
    folder: String,
) -> Result<crate::pet::models::PetImportStagingPreview, String> {
    crate::pet::models::stage_from_folder(st.data_dir(), std::path::Path::new(&folder))
}

#[tauri::command]
pub async fn pet_stage_files_import(
    st: State<'_, Arc<AppState>>,
    payload: crate::pet::models::PetStageFilesPayload,
) -> Result<crate::pet::models::PetImportStagingPreview, String> {
    crate::pet::models::stage_from_files(st.data_dir(), &payload)
}

#[tauri::command]
pub async fn pet_read_model_asset(
    st: State<'_, Arc<AppState>>,
    model_id: String,
    filename: String,
) -> Result<String, String> {
    crate::pet::models::read_model_asset_b64(st.data_dir(), &model_id, &filename)
}

#[tauri::command]
pub async fn pet_read_model_bundle(
    st: State<'_, Arc<AppState>>,
    model_id: String,
    filenames: Vec<String>,
) -> Result<crate::pet::models::PetModelAssetBundle, String> {
    crate::pet::models::read_model_asset_bundle(st.data_dir(), &model_id, &filenames)
}

#[tauri::command]
pub async fn pet_get_import_staging(
    st: State<'_, Arc<AppState>>,
) -> Result<Option<crate::pet::models::PetImportStagingPreview>, String> {
    crate::pet::models::get_import_staging(st.data_dir())
}

#[tauri::command]
pub async fn pet_clear_import_staging(st: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::pet::models::clear_import_staging(st.data_dir())
}

#[tauri::command]
pub async fn pet_commit_import(
    st: State<'_, Arc<AppState>>,
    name: String,
    character_id: Option<String>,
) -> Result<crate::pet::models::PetModelInfo, String> {
    let info = crate::pet::models::commit_staged_import(st.data_dir(), &name)?;
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::models::apply_import_action_template(st.data_dir(), &db, &info.id)?;
    let data_dir = st.data_dir();
    let target_id = character_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| crate::character::active_character_id(&db, data_dir));
    let set_active = crate::character::active_character_id(&db, data_dir) == target_id;
    crate::character::attach_model_to_character(data_dir, &db, &target_id, &info, &name, set_active)?;
    Ok(info)
}

#[tauri::command]
pub async fn pet_import_from_folder(
    st: State<'_, Arc<AppState>>,
    name: String,
    folder: String,
) -> Result<crate::pet::models::PetModelInfo, String> {
    let info = crate::pet::models::import_from_folder(
        st.data_dir(),
        &name,
        std::path::Path::new(&folder),
        None,
    )?;
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::models::apply_import_action_template(st.data_dir(), &db, &info.id)?;
    Ok(info)
}

#[tauri::command]
pub async fn pet_import_files(
    st: State<'_, Arc<AppState>>,
    payload: crate::pet::models::PetImportFilesPayload,
) -> Result<crate::pet::models::PetModelInfo, String> {
    let info = crate::pet::models::import_from_files(st.data_dir(), &payload)?;
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::models::apply_import_action_template(st.data_dir(), &db, &info.id)?;
    Ok(info)
}

#[tauri::command]
pub async fn pet_delete_model(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let data_dir = st.data_dir();
    crate::character::purge_model_from_manifest(data_dir, &model_id)?;
    {
        let db = crate::db::lock_conn(&st.db)?;
        crate::pet::models::delete_model(data_dir, &db, &model_id)?;
    }
    let active = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::pet::models::active_model_id(&db)
    };
    if active == model_id {
        crate::pet::set_active_model(&app, st.inner().clone(), crate::pet::models::BUILTIN_CHAIJUN)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_sync_animations(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    payload: crate::pet::models::PetSyncAnimationsPayload,
) -> Result<crate::pet::models::PetAnimationMeta, String> {
    let model_id = payload.model_id.clone();
    let db = crate::db::lock_conn(&st.db)?;
    let meta = crate::pet::models::sync_animations(st.data_dir(), &db, &payload)?;
    let _ = app.emit("pet-model-meta-updated", model_id);
    Ok(meta)
}

#[tauri::command]
pub async fn pet_set_idle_animation(
    st: State<'_, Arc<AppState>>,
    model_id: String,
    idle_animation: String,
) -> Result<crate::pet::models::PetAnimationMeta, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::models::set_idle_animation(st.data_dir(), &db, &model_id, &idle_animation)
}

#[tauri::command]
pub async fn pet_set_click_animation(
    st: State<'_, Arc<AppState>>,
    model_id: String,
    click_animation: String,
) -> Result<crate::pet::models::PetAnimationMeta, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::models::set_click_animation(st.data_dir(), &db, &model_id, &click_animation)
}

#[tauri::command]
pub async fn pet_set_random_animations(
    st: State<'_, Arc<AppState>>,
    payload: crate::pet::models::PetRandomAnimationsPayload,
) -> Result<crate::pet::models::PetAnimationMeta, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::models::set_random_animations(st.data_dir(), &db, &payload)
}

#[tauri::command]
pub async fn pet_save_animation_layout(
    st: State<'_, Arc<AppState>>,
    payload: crate::pet::models::PetAnimationLayoutPayload,
) -> Result<crate::pet::models::PetAnimationMeta, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::models::save_animation_layout(st.data_dir(), &db, &payload)
}

#[tauri::command]
pub async fn pet_import_lines(
    st: State<'_, Arc<AppState>>,
    payload: crate::pet::models::PetImportLinesPayload,
) -> Result<crate::pet::models::PetAnimationMeta, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::models::import_lines(st.data_dir(), &db, &payload)
}

#[tauri::command]
pub async fn pet_wiki_import_lines(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    model_id: String,
    url: String,
) -> Result<Vec<crate::pet::models::PetRemarkLine>, String> {
    crate::pet::wiki_scrape::wiki_import_lines(&app, st.inner(), &model_id, &url).await
}

#[tauri::command]
pub async fn pet_save_window_size(
    st: State<'_, Arc<AppState>>,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::save_window_size(&db, width, height)
}

#[tauri::command]
pub fn pet_get_window_bounds(
    app: tauri::AppHandle,
) -> Result<crate::pet::PetWindowBounds, String> {
    crate::pet::get_pet_window_bounds(&app)
}

#[tauri::command]
pub fn pet_set_window_bounds(
    app: tauri::AppHandle,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    #[allow(unused_variables)] move_x: Option<bool>,
    #[allow(unused_variables)] move_y: Option<bool>,
) -> Result<(), String> {
    crate::pet::set_pet_window_bounds(
        &app,
        x,
        y,
        width,
        height,
        move_x.unwrap_or(true),
        move_y.unwrap_or(true),
    )
}

#[tauri::command]
pub async fn pet_save_layout(
    st: State<'_, Arc<AppState>>,
    width: f64,
    height: f64,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
    position_win_width: Option<i32>,
    position_win_height: Option<i32>,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::save_layout(
        &db,
        width,
        height,
        scale,
        offset_x,
        offset_y,
        position_win_width,
        position_win_height,
    )
}

#[tauri::command]
pub fn pet_append_movement_logs(
    st: State<'_, Arc<AppState>>,
    lines: Vec<String>,
) -> Result<(), String> {
    crate::pet::append_movement_debug_logs(st.data_dir(), &lines)
}

#[tauri::command]
pub fn pet_append_display_logs(
    st: State<'_, Arc<AppState>>,
    lines: Vec<String>,
) -> Result<(), String> {
    crate::pet::append_display_debug_logs(st.data_dir(), &lines)
}

// ── 系统性能 ──

#[tauri::command]
pub async fn system_get_performance(
) -> Result<crate::system::performance::PerformanceSnapshot, String> {
    tauri::async_runtime::spawn_blocking(crate::system::performance::capture_snapshot)
        .await
        .map_err(|e| e.to_string())
}
