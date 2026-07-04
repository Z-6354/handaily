//! 时间线 AI 简介生成（供 IPC 与后台调度共用）

use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;
use tauri::{AppHandle, Emitter};

use crate::db::timeline_cache;
use crate::state::AppState;
use crate::work_type::WorkTypeConfig;

use super::{
    build_prompt, finalize_chunk, finalized_to_log, filter_valid_cache, json_log, list_uncached_for_date,
    parse_timeline_date, persist_entries, plan_describe, TimelineDescribeChunkEvent, SegmentContext,
};

/// 为指定分页生成/补全简介（与 `timeline_describe` IPC 等价）
pub async fn run_page_describe(
    app: &AppHandle,
    st: &Arc<AppState>,
    date: Option<String>,
    limit: i64,
    offset: i64,
    since_minutes: Option<i64>,
) -> Result<Vec<timeline_cache::TimelineAiEntry>, String> {
    let data_dir = st.data_dir();
    let date = parse_timeline_date(date.as_deref())?;
    let limit = limit.min(200);

    let (contexts, config, catalog, work_types, uncached) = {
        let db = crate::db::lock_conn(&st.db)?;
        let (contexts, cached) = plan_describe(&db, date, limit, offset, since_minutes)?;
        let cached_keys: HashSet<_> = cached.iter().map(|c| c.cache_key.clone()).collect();
        let uncached: Vec<_> = contexts
            .iter()
            .filter(|c| !cached_keys.contains(&c.cache_key))
            .cloned()
            .collect();
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        let work_types = crate::work_type::WorkTypeConfig::load(&db);
        (contexts, config, catalog, work_types, uncached)
    };

    describe_chunks(
        app,
        st,
        data_dir,
        date,
        limit,
        offset,
        &config,
        &catalog,
        &work_types,
        &uncached,
        true,
    )
    .await?;

    let db = crate::db::lock_conn(&st.db)?;
    let keys: Vec<String> = contexts.iter().map(|c| c.cache_key.clone()).collect();
    let complete = timeline_cache::get_cached(&db, &keys).map_err(|e| e.to_string())?;
    let mut complete = filter_valid_cache(complete);
    complete.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    Ok(complete)
}

/// 后台：为今日所有未缓存片段生成简介并落库
pub async fn run_today_uncached(st: &Arc<AppState>, app: &AppHandle) -> Result<usize, String> {
    let data_dir = st.data_dir();
    let today = chrono::Local::now().date_naive();

    let (uncached, config, catalog, work_types) = {
        let db = crate::db::lock_conn(&st.db)?;
        let uncached = list_uncached_for_date(&db, today)?;
        if uncached.is_empty() {
            return Ok(0);
        }
        let config = crate::ai::AiConfig::load(&db, data_dir);
        let catalog = crate::ai::load_catalog(data_dir);
        let work_types = crate::work_type::WorkTypeConfig::load(&db);
        (uncached, config, catalog, work_types)
    };

    let total = uncached.len();
    describe_chunks(
        app,
        st,
        data_dir,
        today,
        total as i64,
        0,
        &config,
        &catalog,
        &work_types,
        &uncached,
        false,
    )
    .await?;
    Ok(total)
}

async fn describe_chunks(
    app: &AppHandle,
    st: &Arc<AppState>,
    data_dir: &std::path::Path,
    date: NaiveDate,
    limit: i64,
    offset: i64,
    config: &crate::ai::AiConfig,
    catalog: &crate::ai::VendorCatalog,
    work_types: &WorkTypeConfig,
    uncached: &[SegmentContext],
    emit_pet_remarks: bool,
) -> Result<(), String> {
    if uncached.is_empty() {
        return Ok(());
    }

    let date_str = date.format("%Y-%m-%d").to_string();

    for chunk in uncached.chunks(20) {
        let prompt = build_prompt(data_dir, work_types, chunk);

        // 按批（≤20 条）加锁，避免后台任务占锁整轮 AI 导致前端 timeline_describe 长时间阻塞
        let _describe_guard = st.timeline_describe_lock.lock().await;

        let (prep_result, persona_id, system_prompt) = {
            let db = crate::db::lock_conn(&st.db)?;
            let manifest = crate::persona::load_manifest(data_dir);
            let persona_id = crate::persona::active_persona_id(&db, &manifest);
            let system_prompt = crate::persona::system_prompt(data_dir, &db);
            let prep_result = crate::ai::PreparedTextChat::prepare(
                config,
                catalog,
                &st.vault,
                &db,
                data_dir,
                prompt.clone(),
            );
            (prep_result, persona_id, system_prompt)
        };

        let (ai_map, request_snapshot, response_log) = match prep_result {
            Ok(Some(prep)) => {
                let request_snapshot = json_log::TimelineAiRequestLog {
                    vendor_id: prep.vendor_id().to_string(),
                    vendor_name: prep.vendor_name().to_string(),
                    model: prep.model().to_string(),
                    persona_id: persona_id.clone(),
                    system_prompt: prep.system_prompt().to_string(),
                    user_prompt: prep.user_prompt().to_string(),
                    work_type_options: work_types.type_names(),
                    segments: chunk.to_vec(),
                };
                match prep.run_async().await {
                    Ok(raw) => {
                        let parsed_map = super::parse_ai_map(&raw, chunk);
                        let response = json_log::TimelineAiResponseLog {
                            used_ai: parsed_map.is_some(),
                            raw: Some(raw),
                            parsed: parsed_map
                                .as_ref()
                                .map(super::ai_map_to_parsed),
                            error: if parsed_map.is_some() {
                                None
                            } else {
                                Some("模型返回无法解析为 JSON 数组，已使用本地简介".into())
                            },
                        };
                        (parsed_map, request_snapshot, response)
                    }
                    Err(e) => {
                        let response = json_log::TimelineAiResponseLog {
                            used_ai: false,
                            raw: None,
                            parsed: None,
                            error: Some(e),
                        };
                        (None, request_snapshot, response)
                    }
                }
            }
            Ok(None) => {
                let request_snapshot = json_log::TimelineAiRequestLog {
                    vendor_id: config.text_vendor_id.clone(),
                    vendor_name: config
                        .vendor(&config.text_vendor_id)
                        .map(|v| v.name.clone())
                        .unwrap_or_default(),
                    model: config.text_model.clone(),
                    persona_id: persona_id.clone(),
                    system_prompt: system_prompt.clone(),
                    user_prompt: prompt.clone(),
                    work_type_options: work_types.type_names(),
                    segments: chunk.to_vec(),
                };
                let response = json_log::TimelineAiResponseLog {
                    used_ai: false,
                    raw: None,
                    parsed: None,
                    error: Some("未配置文本模型，使用本地简介".into()),
                };
                (None, request_snapshot, response)
            }
            Err(e) => {
                let request_snapshot = json_log::TimelineAiRequestLog {
                    vendor_id: config.text_vendor_id.clone(),
                    vendor_name: config
                        .vendor(&config.text_vendor_id)
                        .map(|v| v.name.clone())
                        .unwrap_or_default(),
                    model: config.text_model.clone(),
                    persona_id: persona_id.clone(),
                    system_prompt: system_prompt.clone(),
                    user_prompt: prompt,
                    work_type_options: work_types.type_names(),
                    segments: chunk.to_vec(),
                };
                let response = json_log::TimelineAiResponseLog {
                    used_ai: false,
                    raw: None,
                    parsed: None,
                    error: Some(e),
                };
                (None, request_snapshot, response)
            }
        };

        let finalized = finalize_chunk(chunk, ai_map.as_ref(), work_types);
        let finalized_log = finalized_to_log(&finalized);

        match json_log::save_batch_log(
            data_dir,
            &date_str,
            limit,
            offset,
            request_snapshot,
            response_log,
            finalized_log,
        ) {
            Ok(path) => eprintln!("xiaohan-daily: timeline AI log saved: {}", path.display()),
            Err(e) => eprintln!("xiaohan-daily: timeline AI log save failed: {e}"),
        }

        let saved = {
            let db = crate::db::lock_conn(&st.db)?;
            persist_entries(&db, &finalized)?
        };
        if emit_pet_remarks {
            let ai_ready = {
                let db = crate::db::lock_conn(&st.db)?;
                crate::ai::is_text_ai_ready(config, catalog, &st.vault, &db)
            };
            if ai_ready {
                for (_, _, summary, used_ai) in &finalized {
                    if *used_ai {
                        crate::pet::emit_remark(app, st, summary, "timeline", None);
                        break;
                    }
                }
            }
        }
        let _ = app.emit(
            "timeline-describe-chunk",
            &TimelineDescribeChunkEvent {
                offset,
                limit,
                entries: saved,
            },
        );
    }

    Ok(())
}
