//! `#[tauri::command]` 处理器

use std::sync::Arc;

use chrono::Local;
use serde::Serialize;
use tauri::{Emitter, Manager, State};

use crate::state::AppState;
use crate::tracker::{display_name, ForegroundPayload, Segment};
use crate::vault::VaultEntryInput;

#[tauri::command]
pub fn app_ping() -> String {
    "xiaohan-daily v0.2.0".to_string()
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
pub async fn app_get_prompts_path(st: State<'_, Arc<AppState>>) -> Result<String, String> {
    Ok(crate::prompts::prompts_dir(st.data_dir())
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub async fn app_get_vendors_config_path(st: State<'_, Arc<AppState>>) -> Result<String, String> {
    Ok(crate::ai::vendors_config_path(st.data_dir())
        .to_string_lossy()
        .into_owned())
}

#[derive(Serialize)]
pub struct StatusPayload {
    pub tracking: bool,
    pub open_segment: Option<Segment>,
    pub foreground: Option<ForegroundPayload>,
}

#[tauri::command]
pub async fn tracking_get_status(st: State<'_, Arc<AppState>>) -> Result<StatusPayload, String> {
    let tracking = st
        .tracking_enabled
        .load(std::sync::atomic::Ordering::Relaxed);
    let open = st.open_segment.lock().map_err(|e| e.to_string())?.clone();
    let foreground = st.foreground.lock().map_err(|e| e.to_string())?.clone();
    Ok(StatusPayload {
        tracking,
        open_segment: open,
        foreground,
    })
}

#[tauri::command]
pub async fn tracking_set_enabled(
    st: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    st.set_tracking_enabled(enabled)?;
    if let Some(tray) = st.app.try_state::<crate::tray::TrayMenuState>() {
        let label = if enabled { "暂停采集" } else { "恢复采集" };
        let _ = tray.pause_item.set_text(label);
    }
    Ok(())
}

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

/// 今日概览：前台=有效应用时长，后台=采集会话墙钟时长
#[derive(Serialize)]
pub struct OverviewPayload {
    pub foreground_ms: u64,
    pub background_ms: u64,
    pub app_usage_ms: u64,
    pub companion_ms: u64,
    pub switch_count: u64,
    pub top_app: Option<String>,
    pub top_app_display: Option<String>,
}

#[tauri::command]
pub async fn stats_today_overview(st: State<'_, Arc<AppState>>) -> Result<OverviewPayload, String> {
    let snap = st.aggregator.read().map_err(|e| e.to_string())?.clone();
    let today = Local::now().date_naive();
    let (background_ms, app_usage_ms, companion_ms) = {
        let db = crate::db::lock_conn(&st.db)?;
        let background_ms = crate::db::sessions::background_ms_for_date(&db, today)
            .map_err(|e| e.to_string())?;
        let app_usage_ms = crate::db::usage::app_usage_ms_for_date(&db, today)
            .map_err(|e| e.to_string())?;
        let companion_ms = crate::db::usage::companion_ms_for_date(&db, today)
            .map_err(|e| e.to_string())?;
        (background_ms, app_usage_ms, companion_ms)
    };
    let top_key = snap
        .app_breakdown
        .iter()
        .max_by_key(|(_, &v)| v)
        .map(|(k, _)| k.clone());
    let top_app_display = top_key.as_ref().map(|k| display_name::friendly_from_key(k));
    Ok(OverviewPayload {
        foreground_ms: snap.total_ms,
        background_ms,
        app_usage_ms,
        companion_ms,
        switch_count: snap.switch_count,
        top_app: top_key,
        top_app_display,
    })
}

#[derive(Serialize)]
pub struct AppBreakdownItem {
    pub key: String,
    pub display_name: String,
    pub ms: u64,
    pub icon: Option<String>,
}

#[tauri::command]
pub async fn stats_app_breakdown(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<AppBreakdownItem>, String> {
    let snap = st.aggregator.read().map_err(|e| e.to_string())?.clone();
    let db = crate::db::lock_conn(&st.db)?;
    let keys: Vec<String> = snap.app_breakdown.keys().cloned().collect();
    let exe_paths = crate::db::stats::latest_exe_paths_for_keys(&db, &keys);

    let mut items: Vec<AppBreakdownItem> = snap
        .app_breakdown
        .iter()
        .map(|(k, &v)| {
            let exe_path = exe_paths.get(k).cloned().unwrap_or_default();
            let icon_path = crate::tracker::icon::resolve_icon_path(k, &exe_path);
            let icon = icon_path.and_then(|p| crate::tracker::icon::icon_data_url(&p));
            AppBreakdownItem {
                key: k.clone(),
                display_name: display_name::friendly_from_key(k),
                ms: v,
                icon,
            }
        })
        .collect();
    items.sort_by(|a, b| b.ms.cmp(&a.ms));
    Ok(items)
}

#[tauri::command]
pub async fn stats_hourly_activity(st: State<'_, Arc<AppState>>) -> Result<[u64; 24], String> {
    Ok(st.aggregator.read().map_err(|e| e.to_string())?.hourly)
}

#[tauri::command]
pub async fn stats_three_day_heatmap(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::db::stats::HeatmapDay>, String> {
    let st = (*st).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let db = crate::db::lock_conn(&st.db)?;
        crate::db::stats::query_three_day_heatmap(&db).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn stats_timeline(
    st: State<'_, Arc<AppState>>,
    limit: Option<i64>,
    offset: Option<i64>,
    since_minutes: Option<i64>,
) -> Result<crate::db::stats::TimelinePage, String> {
    let limit = limit.unwrap_or(50).min(200);
    let offset = offset.unwrap_or(0);
    let date = Local::now().date_naive();
    let st = (*st).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let db = crate::db::lock_conn(&st.db)?;
        crate::db::stats::query_timeline(&db, date, limit, offset, since_minutes)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── 密码本 ──

#[tauri::command]
pub async fn vault_get_status(st: State<'_, Arc<AppState>>) -> Result<crate::vault::VaultStatus, String> {
    Ok(st.vault.status())
}

#[tauri::command]
pub async fn vault_setup(
    st: State<'_, Arc<AppState>>,
    password: Option<String>,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    st.vault.setup(&db, password.as_deref())
}

#[tauri::command]
pub async fn vault_unlock(
    st: State<'_, Arc<AppState>>,
    password: Option<String>,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    st.vault.unlock(&db, password.as_deref())
}

#[tauri::command]
pub async fn vault_lock(st: State<'_, Arc<AppState>>) -> Result<(), String> {
    st.vault.lock();
    Ok(())
}

#[derive(Serialize)]
pub struct VaultEntryPublic {
    pub id: i64,
    pub name: String,
    pub website_url: String,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub async fn vault_list_entries(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<VaultEntryPublic>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let items = st.vault.list_entries(&db)?;
    Ok(items
        .into_iter()
        .map(|e| VaultEntryPublic {
            id: e.id,
            name: e.name,
            website_url: e.website_url,
            created_at: e.created_at,
            updated_at: e.updated_at,
        })
        .collect())
}

#[tauri::command]
pub async fn vault_add_entry(
    st: State<'_, Arc<AppState>>,
    entry: VaultEntryInput,
) -> Result<i64, String> {
    let db = crate::db::lock_conn(&st.db)?;
    st.vault.add_entry(
        &db,
        &entry.name,
        &entry.website_url,
        &entry.secret,
    )
}

#[tauri::command]
pub async fn vault_update_entry(
    st: State<'_, Arc<AppState>>,
    id: i64,
    entry: VaultEntryInput,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    st.vault.update_entry(
        &db,
        id,
        &entry.name,
        &entry.website_url,
        &entry.secret,
    )
}

#[tauri::command]
pub async fn vault_delete_entry(
    st: State<'_, Arc<AppState>>,
    id: i64,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    st.vault.delete_entry(&db, id)
}

#[tauri::command]
pub async fn vault_get_secret(st: State<'_, Arc<AppState>>, id: i64) -> Result<String, String> {
    let db = crate::db::lock_conn(&st.db)?;
    st.vault.get_secret(&db, id)
}

#[tauri::command]
pub async fn analysis_get_status(
    st: State<'_, Arc<AppState>>,
) -> Result<crate::db::insights::TodayAnalysisStats, String> {
    crate::analysis::coordinator::query_today_stats(&st)
}

#[tauri::command]
pub async fn analysis_list_insights(
    st: State<'_, Arc<AppState>>,
    limit: Option<i64>,
) -> Result<Vec<crate::db::insights::InsightPublic>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::db::insights::list_today(&db, limit.unwrap_or(30)).map_err(|e| e.to_string())
}

// ── 系统性能 ──

#[tauri::command]
pub async fn system_get_performance(
) -> Result<crate::system::performance::PerformanceSnapshot, String> {
    tauri::async_runtime::spawn_blocking(crate::system::performance::capture_snapshot)
        .await
        .map_err(|e| e.to_string())
}

// ── 今日输入/文件指标 ──

#[tauri::command]
pub async fn stats_today_metrics(
    st: State<'_, Arc<AppState>>,
) -> Result<crate::db::metrics::DailyMetrics, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let base = crate::db::metrics::load_today(&db).map_err(|e| e.to_string())?;
    Ok(st.input_stats.live_totals(&base))
}

// ── AI 配置 ──

#[tauri::command]
pub async fn ai_get_config(st: State<'_, Arc<AppState>>) -> Result<crate::ai::AiConfig, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::ai::AiConfig::load(&db, st.data_dir()))
}

#[tauri::command]
pub async fn ai_is_text_ready(st: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let config = crate::ai::AiConfig::load(&db, st.data_dir());
    let catalog = crate::ai::load_catalog(st.data_dir());
    Ok(crate::ai::is_text_ai_ready(
        &config,
        &catalog,
        &st.vault,
        &db,
    ))
}

#[tauri::command]
pub async fn ai_save_config(
    st: State<'_, Arc<AppState>>,
    config: crate::ai::AiConfig,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    config.save(&db).map_err(|e| e.to_string())
}

fn vendor_api_key(
    st: &AppState,
    db: &rusqlite::Connection,
    config: &crate::ai::AiConfig,
    vendor_id: &str,
) -> Option<String> {
    config.vendor(vendor_id)?.vault_entry_id.and_then(|id| {
        if st.vault.is_unlocked() {
            st.vault.get_secret(db, id).ok()
        } else {
            None
        }
    })
}

#[derive(Serialize)]
pub struct AiModelOption {
    pub id: String,
    pub name: String,
    pub custom: bool,
}

#[tauri::command]
pub async fn ai_list_models(
    st: State<'_, Arc<AppState>>,
    vendor_id: String,
    kind: String,
) -> Result<Vec<AiModelOption>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let catalog = crate::ai::load_catalog(st.data_dir());
    let config = crate::ai::AiConfig::load(&db, st.data_dir());
    let model_kind = parse_model_kind(&kind);
    Ok(config
        .models_for(&catalog, &vendor_id, model_kind)
        .into_iter()
        .map(|m| AiModelOption {
            id: m.id,
            name: m.name,
            custom: m.custom,
        })
        .collect())
}

fn persist_imported_models(
    db: &rusqlite::Connection,
    data_dir: &std::path::Path,
    vendor_id: &str,
    kind: crate::ai::ModelKind,
    remote: &[crate::ai::providers::RemoteModel],
) -> Result<(), String> {
    let catalog = crate::ai::load_catalog(data_dir);
    let excluded = catalog
        .vendor(vendor_id)
        .map(|v| v.excluded_models.as_slice())
        .unwrap_or(&[]);
    let mut config = crate::ai::AiConfig::load(db, data_dir);
    config.imported_models.retain(|m| m.vendor_id != vendor_id || m.kind != kind);
    config.imported_models.extend(remote.iter().filter(|m| {
        !excluded.iter().any(|x| x == &m.id)
    }).map(|m| crate::ai::AiModelEntry {
        id: m.id.clone(),
        name: m.name.clone(),
        vendor_id: vendor_id.to_string(),
        kind,
        custom: false,
    }));
    config.save(db).map_err(|e| e.to_string())
}

async fn fetch_vendor_models_for_import(
    def: &crate::ai::VendorDefinition,
    api_key: Option<&str>,
) -> (Vec<crate::ai::providers::RemoteModel>, Vec<crate::ai::providers::RemoteModel>) {
    let text = crate::ai::providers::fetch_remote_models(
        def,
        crate::ai::ModelKind::Text,
        api_key,
    )
    .await
    .unwrap_or_default();
    let vision = crate::ai::providers::fetch_remote_models(
        def,
        crate::ai::ModelKind::Vision,
        api_key,
    )
    .await
    .unwrap_or_default();
    (text, vision)
}

fn persist_fetched_vendor_models(
    db: &rusqlite::Connection,
    data_dir: &std::path::Path,
    vendor_id: &str,
    text: &[crate::ai::providers::RemoteModel],
    vision: &[crate::ai::providers::RemoteModel],
) -> Result<(usize, usize), String> {
    if !text.is_empty() {
        persist_imported_models(db, data_dir, vendor_id, crate::ai::ModelKind::Text, text)?;
    }
    if !vision.is_empty() {
        persist_imported_models(db, data_dir, vendor_id, crate::ai::ModelKind::Vision, vision)?;
    }
    Ok((text.len(), vision.len()))
}

#[tauri::command]
pub async fn ai_import_models(
    st: State<'_, Arc<AppState>>,
    vendor_id: String,
    kind: String,
) -> Result<Vec<AiModelOption>, String> {
    let (def, model_kind, api_key) = {
        let db = crate::db::lock_conn(&st.db)?;
        let catalog = crate::ai::load_catalog(st.data_dir());
        let config = crate::ai::AiConfig::load(&db, st.data_dir());
        let def = catalog
            .vendor(&vendor_id)
            .ok_or("未知供应商")?
            .clone();
        let model_kind = parse_model_kind(&kind);
        let api_key = vendor_api_key(&st, &db, &config, &vendor_id);
        (def, model_kind, api_key)
    };
    let remote =
        crate::ai::providers::fetch_remote_models(&def, model_kind, api_key.as_deref()).await?;
    if remote.is_empty() {
        return Err("未获取到模型，请使用「手动添加」填写模型 ID".into());
    }
    {
        let db = crate::db::lock_conn(&st.db)?;
        persist_imported_models(&db, st.data_dir(), &vendor_id, model_kind, &remote)?;
    }
    Ok(remote
        .into_iter()
        .map(|m| AiModelOption {
            id: m.id,
            name: m.name,
            custom: false,
        })
        .collect())
}

#[tauri::command]
pub async fn ai_test_vendor(
    st: State<'_, Arc<AppState>>,
    vendor_id: String,
) -> Result<crate::ai::providers::VendorTestResult, String> {
    let (def, api_key) = {
        let db = crate::db::lock_conn(&st.db)?;
        let catalog = crate::ai::load_catalog(st.data_dir());
        let config = crate::ai::AiConfig::load(&db, st.data_dir());
        let def = catalog
            .vendor(&vendor_id)
            .ok_or("未知供应商")?
            .clone();
        let api_key = vendor_api_key(&st, &db, &config, &vendor_id);
        (def, api_key)
    };
    let test = crate::ai::providers::test_vendor_connection(&def, api_key.as_deref()).await;
    if !test.ok {
        return Ok(test);
    }
    let (text_models, vision_models) =
        fetch_vendor_models_for_import(&def, api_key.as_deref()).await;
    let (imported_text, imported_vision) = {
        let db = crate::db::lock_conn(&st.db)?;
        persist_fetched_vendor_models(
            &db,
            st.data_dir(),
            &vendor_id,
            &text_models,
            &vision_models,
        )?
    };
    let message = if imported_text + imported_vision > 0 {
        format!(
            "{}，已保存 {} 个文本 / {} 个多模态模型到本地",
            test.message, imported_text, imported_vision
        )
    } else {
        test.message
    };
    Ok(crate::ai::providers::VendorTestResult {
        ok: true,
        message,
        imported_text,
        imported_vision,
    })
}

#[tauri::command]
pub async fn ai_add_custom_model(
    st: State<'_, Arc<AppState>>,
    vendor_id: String,
    kind: String,
    id: String,
    name: String,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    let mut config = crate::ai::AiConfig::load(&db, st.data_dir());
    let model_kind = parse_model_kind(&kind);
    config.custom_models.retain(|m| !(m.vendor_id == vendor_id && m.id == id));
    config.custom_models.push(crate::ai::AiModelEntry {
        id: id.clone(),
        name,
        vendor_id: vendor_id.clone(),
        kind: model_kind,
        custom: true,
    });
    match model_kind {
        crate::ai::ModelKind::Text if config.text_vendor_id == vendor_id => {
            config.text_model = id;
        }
        crate::ai::ModelKind::Vision if config.vision_vendor_id == vendor_id => {
            config.vision_model = id;
        }
        crate::ai::ModelKind::Thinking if config.thinking_vendor_id == vendor_id => {
            config.thinking_model = id;
        }
        _ => {}
    }
    config.save(&db).map_err(|e| e.to_string())
}

fn parse_model_kind(kind: &str) -> crate::ai::ModelKind {
    match kind {
        "vision" => crate::ai::ModelKind::Vision,
        "thinking" => crate::ai::ModelKind::Thinking,
        _ => crate::ai::ModelKind::Text,
    }
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
pub async fn app_get_personas_path(st: State<'_, Arc<AppState>>) -> Result<String, String> {
    Ok(crate::persona::personas_dir(st.data_dir())
        .to_string_lossy()
        .into_owned())
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
pub async fn persona_regenerate_profile(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    persona_id: String,
) -> Result<crate::persona::PersonaImportResult, String> {
    let data_dir = st.data_dir();
    let ctx = crate::persona::import_reference::ImportReferenceContext {
        data_dir,
        db: &st.db,
        vault: &st.vault,
        app: Some(&app),
    };
    crate::persona::import_reference::regenerate_persona_profile(&ctx, persona_id.trim()).await
}

#[tauri::command]
pub async fn persona_batch_regenerate_profiles(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    limit: Option<usize>,
    only_missing: Option<bool>,
) -> Result<crate::persona::import_reference::PersonaBatchRegenerateResult, String> {
    let data_dir = st.data_dir();
    let ctx = crate::persona::import_reference::ImportReferenceContext {
        data_dir,
        db: &st.db,
        vault: &st.vault,
        app: Some(&app),
    };
    crate::persona::import_reference::batch_regenerate_persona_profiles(
        &ctx,
        limit.unwrap_or(10),
        only_missing.unwrap_or(true),
    )
    .await
}

#[tauri::command]
pub async fn agent_get_status(st: State<'_, Arc<AppState>>) -> Result<crate::agent_http::AgentStatus, String> {
    Ok(crate::agent_http::status(st.inner().clone()))
}

#[tauri::command]
pub async fn agent_set_enabled(
    st: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<crate::agent_http::AgentStatus, String> {
    crate::agent_http::set_enabled(st.inner().clone(), enabled)
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
    crate::persona::import_reference::import_from_reference(
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
    .await
}

const CHARACTER_WIKI_IMPORT_STEP_TOTAL: u32 = 6;

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
                )? as u32;
                let active = crate::pet::models::active_model_id(&db);
                drop(db);
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

const PERSONA_WIKI_IMPORT_STEP_TOTAL: u32 = 4;

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

#[derive(Serialize)]
pub struct PersonaTestResult {
    pub ok: bool,
    pub message: String,
    pub reply: Option<String>,
}

#[tauri::command]
pub async fn ai_test_persona(
    st: State<'_, Arc<AppState>>,
) -> Result<PersonaTestResult, String> {
    let data_dir = st.data_dir();
    let ai_prep = {
        let db = crate::db::lock_conn(&st.db)?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        if config.text_model.trim().is_empty() {
            return Ok(PersonaTestResult {
                ok: false,
                message: "请先在 AI 配置中选择文本模型".into(),
                reply: None,
            });
        }
        let catalog = crate::ai::load_catalog(data_dir);
        let prompt = crate::prompts::render(data_dir, "persona-test", &[]);
        crate::ai::PreparedTextChat::prepare(&config, &catalog, &st.vault, &db, data_dir, prompt)?
    };

    let Some(prep) = ai_prep else {
        return Ok(PersonaTestResult {
            ok: false,
            message: "请先在 AI 配置中选择文本模型".into(),
            reply: None,
        });
    };

    match prep.run_async().await {
        Ok(reply) => {
            let trimmed = reply.trim();
            if trimmed.is_empty() {
                Ok(PersonaTestResult {
                    ok: false,
                    message: "模型返回为空".into(),
                    reply: None,
                })
            } else {
                Ok(PersonaTestResult {
                    ok: true,
                    message: "人设与模型连通正常".into(),
                    reply: Some(trimmed.to_string()),
                })
            }
        }
        Err(e) => Ok(PersonaTestResult {
            ok: false,
            message: e,
            reply: None,
        }),
    }
}

// ── 人物（性格 + 皮肤 → 模型）──

#[tauri::command]
pub async fn characters_list(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::character::CharacterInfo>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::character::list_characters(st.data_dir(), &db))
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
) -> Result<(), String> {
    let data_dir = st.data_dir();
    let model_id = {
        let db = crate::db::lock_conn(&st.db)?;
        let manifest = crate::persona::load_manifest(data_dir);
        crate::character::set_active_character(data_dir, &db, &manifest, &character_id)?;
        crate::pet::models::active_model_id(&db)
    };
    crate::pet::set_active_model(&app, st.inner().clone(), &model_id)
}

#[tauri::command]
pub async fn characters_set_skin(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    character_id: String,
    skin_id: String,
) -> Result<(), String> {
    let data_dir = st.data_dir();
    let model_id = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::character::select_character_skin(data_dir, &db, &character_id, &skin_id)?
    };
    crate::pet::set_active_model(&app, st.inner().clone(), &model_id)
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
    let db = crate::db::lock_conn(&st.db)?;
    crate::live2d_import::run_live2d_import(
        &data_dir,
        &db,
        &plan,
        &live2d_root,
        limit,
        false,
    )
}

#[tauri::command]
pub async fn character_list(
    st: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::db::character_profiles::CharacterProfileRow>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::list_profiles(&db)
}

#[tauri::command]
pub async fn character_get(
    st: State<'_, Arc<AppState>>,
    id: i64,
) -> Result<crate::db::character_profiles::CharacterProfileRow, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::get_profile(&db, id)
}

#[tauri::command]
pub async fn character_create(
    st: State<'_, Arc<AppState>>,
    name: String,
    source: Option<String>,
    raw_text: String,
) -> Result<crate::db::character_profiles::CharacterProfileRow, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::create_profile(&db, &name, source.as_deref().unwrap_or(""), &raw_text)
}

#[tauri::command]
pub async fn character_update_raw(
    st: State<'_, Arc<AppState>>,
    id: i64,
    raw_text: String,
) -> Result<crate::db::character_profiles::CharacterProfileRow, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::update_raw_text(&db, id, &raw_text)
}

#[tauri::command]
pub async fn character_update_json(
    st: State<'_, Arc<AppState>>,
    id: i64,
    profile_json: crate::db::character_profiles::CharacterProfileData,
) -> Result<crate::db::character_profiles::CharacterProfileRow, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::update_profile_data(&db, id, profile_json)
}

#[tauri::command]
pub async fn character_save_skill(
    st: State<'_, Arc<AppState>>,
    id: i64,
    skill_md: String,
) -> Result<crate::db::character_profiles::CharacterProfileRow, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::save_skill_md(&db, id, &skill_md)
}

#[tauri::command]
pub async fn character_delete(
    st: State<'_, Arc<AppState>>,
    id: i64,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::delete_profile(&db, id)
}

#[tauri::command]
pub async fn character_preprocess(
    st: State<'_, Arc<AppState>>,
    id: i64,
) -> Result<crate::persona_builder::CharacterOpResult, String> {
    let data_dir = st.data_dir();
    let prep = {
        let db = crate::db::lock_conn(&st.db)?;
        let row = crate::persona_builder::get_profile(&db, id)?;
        let prompt = crate::persona_builder::build_preprocess_prompt(data_dir, &row)?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        crate::ai::PreparedThinkingChat::prepare(
            &config,
            &catalog,
            &st.vault,
            &db,
            data_dir,
            prompt,
        )?
    };
    let Some(prep) = prep else {
        return Err("请先在设置中配置思考模型（用于文本预处理与 Skill 生成）".into());
    };
    let raw = prep.run_async().await?;
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::apply_preprocessed(&db, id, &raw)
}

#[tauri::command]
pub async fn character_merge_text(
    st: State<'_, Arc<AppState>>,
    id: i64,
    text: String,
) -> Result<crate::persona_builder::CharacterOpResult, String> {
    if text.trim().is_empty() {
        return Err("补充文本不能为空".into());
    }
    let data_dir = st.data_dir();
    let prep = {
        let db = crate::db::lock_conn(&st.db)?;
        let row = crate::persona_builder::get_profile(&db, id)?;
        let prompt = crate::persona_builder::build_merge_prompt(data_dir, &row, &text)?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        crate::ai::PreparedThinkingChat::prepare(
            &config,
            &catalog,
            &st.vault,
            &db,
            data_dir,
            prompt,
        )?
    };
    let Some(prep) = prep else {
        return Err("请先在设置中配置思考模型".into());
    };
    let raw = prep.run_async().await?;
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::apply_merged(&db, id, &raw)
}

#[tauri::command]
pub async fn character_generate_skill(
    st: State<'_, Arc<AppState>>,
    id: i64,
) -> Result<crate::persona_builder::CharacterOpResult, String> {
    let data_dir = st.data_dir();
    let prep = {
        let db = crate::db::lock_conn(&st.db)?;
        let row = crate::persona_builder::get_profile(&db, id)?;
        let prompt = crate::persona_builder::build_skill_prompt(data_dir, &row)?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        crate::ai::PreparedThinkingChat::prepare(
            &config,
            &catalog,
            &st.vault,
            &db,
            data_dir,
            prompt,
        )?
    };
    let Some(prep) = prep else {
        return Err("请先在设置中配置思考模型".into());
    };
    let raw = prep.run_async().await?;
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::apply_generated_skill(&db, id, &raw)
}

#[tauri::command]
pub async fn character_apply_persona(
    st: State<'_, Arc<AppState>>,
    id: i64,
    activate: Option<bool>,
) -> Result<crate::persona_builder::CharacterOpResult, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::persona_builder::apply_to_persona(&db, st.data_dir(), id, activate.unwrap_or(true))
}

// ── 报告生成 ──

#[derive(Serialize)]
pub struct ReportGenerateResult {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub used_ai: bool,
    pub template_id: String,
    pub date_from: String,
    pub date_to: String,
}

#[tauri::command]
pub async fn report_generate(
    st: State<'_, Arc<AppState>>,
    template_id: String,
    date_from: String,
    date_to: String,
) -> Result<ReportGenerateResult, String> {
    let data_dir = st.data_dir();

    let (gathered, ai_prep) = {
        let db = crate::db::lock_conn(&st.db)?;
        let gathered = crate::report::gather(&db, &template_id, &date_from, &date_to)?;
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        let prep = crate::report::prepare_ai_chat(
            &gathered,
            &config,
            &catalog,
            &st.vault,
            &db,
            data_dir,
        )?;
        (gathered, prep)
    };

    let ai_content = if let Some(prep) = ai_prep {
        match prep.run_async().await {
            Ok(s) if !s.trim().is_empty() => Some(s),
            Ok(_) => None,
            Err(e) => {
                crate::log::warn(format!("report AI fallback: {e}"));
                None
            }
        }
    } else {
        None
    };

    let draft = crate::report::compose(&gathered, ai_content);

    let id = {
        let db = crate::db::lock_conn(&st.db)?;
        crate::report::save_generated(&db, &template_id, &draft, &date_from, &date_to)?
    };

    Ok(ReportGenerateResult {
        id,
        title: draft.title,
        content: draft.content,
        used_ai: draft.used_ai,
        template_id,
        date_from,
        date_to,
    })
}

#[tauri::command]
pub async fn report_list(
    st: State<'_, Arc<AppState>>,
    limit: Option<i64>,
) -> Result<Vec<crate::db::reports::GeneratedReport>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::db::reports::list_reports(&db, limit.unwrap_or(50)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn report_delete(
    st: State<'_, Arc<AppState>>,
    id: i64,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    if crate::db::reports::delete_report(&db, id).map_err(|e| e.to_string())? {
        Ok(())
    } else {
        Err("报告不存在".into())
    }
}

#[tauri::command]
pub async fn app_get_timeline_ai_logs_path(st: State<'_, Arc<AppState>>) -> Result<String, String> {
    let data_dir = st.data_dir();
    crate::timeline::json_log::ensure_logs_dir(data_dir).map_err(|e| e.to_string())?;
    Ok(crate::timeline::json_log::logs_dir(data_dir)
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub fn timeline_cached(
    st: State<'_, Arc<AppState>>,
    date: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    since_minutes: Option<i64>,
) -> Result<Vec<crate::db::timeline_cache::TimelineAiEntry>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    let date = crate::timeline::parse_timeline_date(date.as_deref())?;
    let limit = limit.unwrap_or(50).min(200);
    let offset = offset.unwrap_or(0);
    let (_, cached) =
        crate::timeline::plan_describe(&db, date, limit, offset, since_minutes)
            .map_err(|e| e.to_string())?;
    Ok(cached)
}

#[tauri::command]
pub async fn timeline_describe(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    date: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    since_minutes: Option<i64>,
) -> Result<Vec<crate::db::timeline_cache::TimelineAiEntry>, String> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    crate::timeline::describe::run_page_describe(
        &app,
        st.inner(),
        date,
        limit,
        offset,
        since_minutes,
    )
    .await
}

// ── 工作类型 ──

#[tauri::command]
pub async fn work_types_get(
    st: State<'_, Arc<AppState>>,
) -> Result<crate::work_type::WorkTypeConfig, String> {
    let db = crate::db::lock_conn(&st.db)?;
    Ok(crate::work_type::WorkTypeConfig::load(&db))
}

#[tauri::command]
pub async fn work_types_save(
    st: State<'_, Arc<AppState>>,
    config: crate::work_type::WorkTypeConfig,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    config.save(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn period_list_summaries(
    st: State<'_, Arc<AppState>>,
    limit: Option<i64>,
) -> Result<Vec<crate::db::periods::PeriodSummaryPublic>, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::db::periods::list_period_summaries_today(&db, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

// ── 桌宠 ──

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
) -> Result<crate::pet::PetPoint, String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::save_position(&db, x, y)
}

#[tauri::command]
pub fn pet_get_screen_bounds() -> crate::pet::PetScreenBounds {
    crate::pet::screen_bounds()
}

#[tauri::command]
pub fn app_exit(app: tauri::AppHandle, st: State<'_, Arc<AppState>>) {
    crate::prepare_app_exit(&app, st.inner());
    app.exit(0);
}

#[tauri::command]
pub async fn pet_open_main(
    app: tauri::AppHandle,
    page: Option<String>,
) -> Result<(), String> {
    crate::pet::show_main_window(&app, page.as_deref())
}

#[tauri::command]
pub async fn pet_reload(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    crate::pet::reload_pet(&app, st.inner())
}

#[tauri::command]
pub fn pet_nudge(app: tauri::AppHandle) -> Result<(), String> {
    crate::pet::nudge_pet(&app);
    Ok(())
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
pub fn pet_mark_spine_ready(app: tauri::AppHandle) {
    crate::pet::mark_spine_ready(&app);
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
    st: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::set_bubble_enabled(&db, enabled)
}

#[tauri::command]
pub async fn pet_get_model_status(
    st: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<crate::pet::PetStatusPayload, String> {
    crate::pet::model_status(st.inner(), &model_id)
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
    scale: Option<f64>,
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
            scale,
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
pub async fn pet_ai_suggest_lines(
    st: State<'_, Arc<AppState>>,
    model_id: String,
    count: Option<usize>,
) -> Result<Vec<crate::pet::models::PetRemarkLine>, String> {
    crate::pet::ai_suggest_lines(st.inner(), &model_id, count.unwrap_or(8).clamp(1, 30)).await
}

#[tauri::command]
pub async fn pet_ai_import_lines(
    app: tauri::AppHandle,
    st: State<'_, Arc<AppState>>,
    model_id: String,
    raw_text: String,
) -> Result<Vec<crate::pet::models::PetRemarkLine>, String> {
    crate::pet::lines_import::ai_import_lines(&app, st.inner(), &model_id, &raw_text).await
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
pub async fn pet_save_layout(
    st: State<'_, Arc<AppState>>,
    width: f64,
    height: f64,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) -> Result<(), String> {
    let db = crate::db::lock_conn(&st.db)?;
    crate::pet::save_layout(&db, width, height, scale, offset_x, offset_y)
}

// ── 微信绑定 ──

#[tauri::command]
pub fn wechat_get_status(st: State<'_, Arc<AppState>>) -> Result<crate::wechat::WechatStatus, String> {
    crate::wechat::get_status(&st)
}

#[tauri::command]
pub async fn wechat_start_qr(st: State<'_, Arc<AppState>>) -> Result<crate::wechat::WechatQrStart, String> {
    crate::wechat::start_qr_login(&st).await
}

#[tauri::command]
pub async fn wechat_poll_qr(
    st: State<'_, Arc<AppState>>,
    qrcode_id: String,
) -> Result<crate::wechat::WechatQrPoll, String> {
    crate::wechat::poll_qr_login(&st, &qrcode_id).await
}

#[tauri::command]
pub fn wechat_logout(st: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::wechat::logout(&st)
}

#[tauri::command]
pub fn wechat_set_push_enabled(
    st: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    crate::wechat::set_push_enabled(&st, enabled)
}

#[tauri::command]
pub async fn wechat_test_send(st: State<'_, Arc<AppState>>) -> Result<String, String> {
    crate::wechat::test_send(&st).await
}

#[tauri::command]
pub fn wechat_prepare_rebind(st: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::wechat::prepare_rebind(&st)
}

#[tauri::command]
pub fn wechat_import_hanagent(st: State<'_, Arc<AppState>>) -> Result<bool, String> {
    crate::wechat::import_from_hanagent(&st)
}
