//! 视觉模型分析（经 AI 适配器调用）

use std::path::Path;

use base64::Engine;

use crate::ai::{self, runtime, AiConfig, VendorCatalog};
use crate::analysis::AnalysisSettings;
use crate::tracker::Segment;
use crate::vault::VaultState;
use crate::work_type::WorkTypeConfig;

#[derive(Debug, Clone)]
pub struct VisionInsight {
    pub category: String,
    pub summary: String,
    pub confidence: f32,
}

/// DB 侧准备数据；持锁阶段调用，不含网络 I/O。
#[derive(Debug, Clone)]
pub struct VisionPrepared {
    pub config: AiConfig,
    pub catalog: VendorCatalog,
    pub api_key: String,
    pub system: String,
    pub prompt: String,
    pub work_types: WorkTypeConfig,
}

pub fn prepare_screenshot(
    segment: &Segment,
    settings: &AnalysisSettings,
    vault: &VaultState,
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<Option<VisionPrepared>, String> {
    if !settings.vision_enabled {
        return Ok(None);
    }
    let catalog = ai::load_catalog(data_dir);
    let config = AiConfig::load(db, data_dir);
    let vendor = config
        .vendor(&config.vision_vendor_id)
        .ok_or("未配置视觉供应商")?;
    let model = config.resolve_model(
        &config.vision_vendor_id,
        &config.vision_model,
        ai::ModelKind::Vision,
    );
    if model.trim().is_empty() {
        return Err("请先在设置中导入或手动添加多模态模型".into());
    }
    if catalog.vendor(&config.vision_vendor_id).is_none() {
        return Err("未配置视觉供应商".into());
    }
    let api_key = ai::adapter::resolve_api_key(vendor, vault, db)?;
    let system = crate::persona::system_prompt(data_dir, db);
    let work_types = WorkTypeConfig::load(db);
    let type_list = work_types.type_names().join("、");
    let activity_hint = {
        let label = crate::tracker::activity_key::activity_label_for_segment(segment);
        if label.is_empty() || label == segment.app_name {
            String::new()
        } else {
            format!("，当前活动内容：{label}")
        }
    };
    let prompt = crate::prompts::render(
        data_dir,
        "vision-screenshot",
        &[
            ("app_name", &segment.app_name),
            ("window_title", &segment.window_title),
            ("type_list", &type_list),
            ("activity_hint", &activity_hint),
        ],
    );
    Ok(Some(VisionPrepared {
        config,
        catalog,
        api_key,
        system,
        prompt,
        work_types,
    }))
}

pub fn execute_prepared(prepared: &VisionPrepared, jpeg: &[u8]) -> Result<VisionInsight, String> {
    let def = prepared
        .catalog
        .vendor(&prepared.config.vision_vendor_id)
        .ok_or("未配置视觉供应商")?;
    let model = prepared.config.resolve_model(
        &prepared.config.vision_vendor_id,
        &prepared.config.vision_model,
        ai::ModelKind::Vision,
    );
    let b64 = base64::engine::general_purpose::STANDARD.encode(jpeg);
    let data_url = format!("data:image/jpeg;base64,{b64}");

    let result = runtime::block_on(async {
        ai::adapters::chat_vision(
            def,
            &prepared.api_key,
            &model,
            Some(&prepared.system),
            &prepared.prompt,
            &data_url,
        )
        .await
    })?;
    let v = ai::adapter::parse_vision_json(&result, &prepared.work_types)?;
    Ok(VisionInsight {
        category: v.category,
        summary: v.summary,
        confidence: v.confidence,
    })
}

fn is_quota_or_rate_limit(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("accountquotaexceeded")
        || lower.contains("toomanyrequests")
        || lower.contains("rate limit")
        || lower.contains("usage quota")
        || msg.contains("超出")
        || msg.contains("配额")
}

pub fn execute_or_fallback(
    prepared: &VisionPrepared,
    jpeg: &[u8],
    segment: &Segment,
) -> VisionInsight {
    match execute_prepared(prepared, jpeg) {
        Ok(v) => v,
        Err(e) => {
            let msg = e.to_string();
            if is_quota_or_rate_limit(&msg) {
                crate::log::info("vision AI skipped (quota/rate limit)");
            } else {
                crate::log::warn(format!("vision AI failed: {msg}"));
            }
            fallback_insight(segment)
        }
    }
}

pub fn analyze_screenshot(
    jpeg: &[u8],
    segment: &Segment,
    settings: &AnalysisSettings,
    vault: &VaultState,
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<VisionInsight, String> {
    match prepare_screenshot(segment, settings, vault, db, data_dir)? {
        Some(prep) => Ok(execute_or_fallback(&prep, jpeg, segment)),
        None => Ok(fallback_insight(segment)),
    }
}

pub fn fallback_insight(segment: &Segment) -> VisionInsight {
    let label = crate::tracker::activity_key::activity_label_for_segment(segment);
    let summary = if label.is_empty() || label == segment.app_name {
        format!("正在使用 {}", segment.app_name)
    } else {
        format!("在 {} · {}", segment.app_name, label)
    };
    VisionInsight {
        category: "其他".into(),
        summary,
        confidence: 0.55,
    }
}
