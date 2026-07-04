//! AI 调用入口：通过适配器工厂路由到具体供应商

use std::path::Path;

use base64::Engine;
use super::adapters;
use super::json_util;
use super::runtime;
use super::catalog::{self, VendorCatalog, VendorDefinition};
use super::config::{AiConfig, AiVendor, ModelKind};
use crate::persona;
use crate::prompts;
use crate::tracker::{activity_key, Segment};
use crate::vault::VaultState;
use crate::work_type::WorkTypeConfig;

#[derive(Debug, Clone)]
pub struct VisionResult {
    pub category: String,
    pub summary: String,
    pub confidence: f32,
}

pub fn chat_vision(
    jpeg: &[u8],
    segment: &Segment,
    config: &AiConfig,
    catalog: &VendorCatalog,
    vault: &VaultState,
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<VisionResult, String> {
    let def = catalog
        .vendor(&config.vision_vendor_id)
        .ok_or("未配置视觉供应商")?;
    let vendor = config
        .vendor(&config.vision_vendor_id)
        .ok_or("未配置视觉供应商")?;
    let model = config.resolve_model(
        &config.vision_vendor_id,
        &config.vision_model,
        ModelKind::Vision,
    );
    if model.trim().is_empty() {
        return Err("请先在设置中导入或手动添加多模态模型".into());
    }
    let api_key = resolve_api_key(vendor, vault, db)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(jpeg);
    let data_url = format!("data:image/jpeg;base64,{b64}");
    let system = persona::system_prompt(data_dir, db);
    let work_types = WorkTypeConfig::load(db);
    let type_list = work_types.type_names().join("、");
    let activity_hint = {
        let label = activity_key::activity_label_for_segment(segment);
        if label.is_empty() || label == segment.app_name {
            String::new()
        } else {
            format!("，当前活动内容：{label}")
        }
    };
    let prompt = prompts::render(
        data_dir,
        "vision-screenshot",
        &[
            ("app_name", &segment.app_name),
            ("window_title", &segment.window_title),
            ("type_list", &type_list),
            ("activity_hint", &activity_hint),
        ],
    );

    runtime::block_on(async {
        let content = adapters::chat_vision(
            def,
            &api_key,
            &model,
            Some(&system),
            &prompt,
            &data_url,
        )
        .await?;
        parse_vision_json(&content, &work_types)
    })
}

pub fn chat_text(
    prompt: &str,
    config: &AiConfig,
    catalog: &VendorCatalog,
    vault: &VaultState,
    db: &rusqlite::Connection,
    data_dir: &Path,
) -> Result<String, String> {
    let prep = PreparedTextChat::prepare(
        config,
        catalog,
        vault,
        db,
        data_dir,
        prompt.to_string(),
    )?;
    match prep {
        Some(p) => p.run_sync(),
        None => Err("请先在设置中导入或手动添加文本模型".into()),
    }
}

/// 在持锁的短临界区内解析密钥/人设，网络 I/O 用 [`PreparedTextChat::run`]
pub struct PreparedTextChat {
    vendor_def: VendorDefinition,
    api_key: String,
    model: String,
    system: String,
    user_prompt: String,
}

impl PreparedTextChat {
    pub fn prepare(
        config: &AiConfig,
        catalog: &VendorCatalog,
        vault: &VaultState,
        db: &rusqlite::Connection,
        data_dir: &Path,
        user_prompt: String,
    ) -> Result<Option<Self>, String> {
        if config.text_model.trim().is_empty() {
            return Ok(None);
        }
        let def = catalog
            .vendor(&config.text_vendor_id)
            .ok_or("未配置文本供应商")?
            .clone();
        let vendor = config
            .vendor(&config.text_vendor_id)
            .ok_or("未配置文本供应商")?;
        let model = config.resolve_model(&config.text_vendor_id, &config.text_model, ModelKind::Text);
        if model.trim().is_empty() {
            return Ok(None);
        }
        let api_key = resolve_api_key(vendor, vault, db)?;
        let system = persona::system_prompt(data_dir, db);
        Ok(Some(Self {
            vendor_def: def,
            api_key,
            model,
            system,
            user_prompt,
        }))
    }

    /// 后台线程 / 非 async 上下文
    pub fn run_sync(self) -> Result<String, String> {
        runtime::block_on(self.run_async())
    }

    /// Tauri async command：勿在 Tokio 内再 block_on
    pub async fn run_async(self) -> Result<String, String> {
        adapters::chat_text(
            &self.vendor_def,
            &self.api_key,
            &self.model,
            Some(&self.system),
            &self.user_prompt,
        )
        .await
    }

    pub fn vendor_id(&self) -> &str {
        &self.vendor_def.id
    }

    pub fn vendor_name(&self) -> &str {
        &self.vendor_def.name
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn system_prompt(&self) -> &str {
        &self.system
    }

    pub fn user_prompt(&self) -> &str {
        &self.user_prompt
    }
}

/// 文本 AI 是否已绑定（模型 + 供应商 + 可用密钥）
pub fn is_text_ai_ready(
    config: &AiConfig,
    catalog: &VendorCatalog,
    vault: &crate::vault::VaultState,
    db: &rusqlite::Connection,
) -> bool {
    PreparedTextChat::prepare(
        config,
        catalog,
        vault,
        db,
        std::path::Path::new("."),
        String::new(),
    )
    .map(|opt| opt.is_some())
    .unwrap_or(false)
}

/// 思考模型：不注入当前人设，用于预处理 / Skill 生成等结构化任务
pub struct PreparedThinkingChat {
    vendor_def: VendorDefinition,
    api_key: String,
    model: String,
    user_prompt: String,
}

impl PreparedThinkingChat {
    pub fn prepare(
        config: &AiConfig,
        catalog: &VendorCatalog,
        vault: &VaultState,
        db: &rusqlite::Connection,
        _data_dir: &Path,
        user_prompt: String,
    ) -> Result<Option<Self>, String> {
        let (vendor_id, model_id) = if !config.thinking_model.trim().is_empty() {
            (
                config.thinking_vendor_id.clone(),
                config.thinking_model.clone(),
            )
        } else if !config.text_model.trim().is_empty() {
            (config.text_vendor_id.clone(), config.text_model.clone())
        } else {
            return Ok(None);
        };
        let def = catalog
            .vendor(&vendor_id)
            .ok_or("未配置思考模型供应商")?
            .clone();
        let vendor = config
            .vendor(&vendor_id)
            .ok_or("未配置思考模型供应商")?;
        let model = config.resolve_model(&vendor_id, &model_id, ModelKind::Thinking);
        if model.trim().is_empty() {
            return Ok(None);
        }
        let api_key = resolve_api_key(vendor, vault, db)?;
        Ok(Some(Self {
            vendor_def: def,
            api_key,
            model,
            user_prompt,
        }))
    }

    pub async fn run_async(self) -> Result<String, String> {
        self.run_async_with_options(adapters::ChatOptions::thinking()).await
    }

    pub async fn run_async_with_options(
        self,
        options: adapters::ChatOptions,
    ) -> Result<String, String> {
        const SYSTEM: &str = "你是小寒日报的角色资料整理助手。严格按用户要求输出格式，不要闲聊。";
        adapters::chat_text_with_options(
            &self.vendor_def,
            &self.api_key,
            &self.model,
            Some(SYSTEM),
            &self.user_prompt,
            options,
        )
        .await
    }
}

pub(crate) fn resolve_api_key(
    vendor: &AiVendor,
    vault: &VaultState,
    db: &rusqlite::Connection,
) -> Result<String, String> {
    if vendor.api_style == "ollama" {
        return Ok(String::new());
    }
    let entry_id = vendor
        .vault_entry_id
        .ok_or_else(|| format!("请为「{}」在设置中配置 API 密钥", vendor.name))?;
    if !vault.is_unlocked() {
        return Err("密码本未解锁".into());
    }
    vault.get_secret(db, entry_id)
}

/// 将视觉模型 category 映射到用户配置的工作类型名
pub fn normalize_vision_category(raw: &str, work_types: &WorkTypeConfig) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return work_types.normalize_type_name("其他");
    }
    if work_types.types.iter().any(|t| t.name == trimmed) {
        return trimmed.to_string();
    }
    let lower = trimmed.to_lowercase();
    const ALIASES: &[(&str, &str)] = &[
        ("development", "开发"),
        ("dev", "开发"),
        ("coding", "开发"),
        ("document", "文档"),
        ("doc", "文档"),
        ("meeting", "会议"),
        ("communication", "沟通"),
        ("comm", "沟通"),
        ("chat", "沟通"),
        ("design", "设计"),
        ("browsing", "文档"),
        ("browser", "文档"),
        ("entertainment", "娱乐"),
        ("game", "游戏"),
        ("general", "其他"),
        ("other", "其他"),
        ("idle", "其他"),
    ];
    for (from, to) in ALIASES {
        if lower == *from || lower.contains(from) {
            return work_types.normalize_type_name(to);
        }
    }
    if trimmed.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
        return work_types.normalize_type_name(trimmed);
    }
    work_types.normalize_type_name("其他")
}

pub(crate) fn parse_vision_json(raw: &str, work_types: &WorkTypeConfig) -> Result<VisionResult, String> {
    let json_str = json_util::extract_json_object(raw);
    let v: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("无法解析视觉 API JSON: {e}"))?;

    let category_raw = v["category"]
        .as_str()
        .or_else(|| v["work_type"].as_str())
        .unwrap_or("其他");
    let summary_raw = v["summary"]
        .as_str()
        .or_else(|| v["description"].as_str())
        .unwrap_or("屏幕活动");
    let summary = truncate_chars(summary_raw.trim(), 80);
    if summary.is_empty() {
        return Err("视觉 API 返回空 summary".into());
    }

    let mut confidence = v["confidence"].as_f64().unwrap_or(0.72) as f32;
    if !confidence.is_finite() {
        confidence = 0.72;
    }
    confidence = confidence.clamp(0.05, 0.99);

    Ok(VisionResult {
        category: normalize_vision_category(category_raw, work_types),
        summary,
        confidence,
    })
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

pub fn load_catalog(data_dir: &Path) -> VendorCatalog {
    catalog::load(data_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vision() {
        let wt = crate::work_type::WorkTypeConfig::default();
        let r = parse_vision_json(
            r#"{"category":"开发","summary":"写代码","confidence":0.9}"#,
            &wt,
        )
        .unwrap();
        assert_eq!(r.category, "开发");
    }

    #[test]
    fn parse_vision_alias_and_fence() {
        let wt = crate::work_type::WorkTypeConfig::default();
        let r = parse_vision_json(
            "```json\n{\"category\":\"development\",\"summary\":\"调试接口\",\"confidence\":1.5}\n```",
            &wt,
        )
        .unwrap();
        assert_eq!(r.category, "开发");
        assert!(r.confidence <= 0.99);
    }
}
