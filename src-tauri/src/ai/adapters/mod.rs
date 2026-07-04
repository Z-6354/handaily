//! 供应商适配器工厂与 OpenAI / Ollama 实现

mod ollama;
mod openai;

pub use openai::RemoteModel;

use super::catalog::VendorDefinition;
use super::config::ModelKind;

#[derive(Debug, Clone, Copy)]
pub struct ChatOptions {
    pub max_tokens: u32,
    /// 整次请求超时（含等待模型生成），秒
    pub timeout_secs: u64,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            max_tokens: 1024,
            timeout_secs: 120,
        }
    }
}

impl ChatOptions {
    pub fn thinking() -> Self {
        Self::skill_generate()
    }

    /// 参考文本 → 结构化 JSON（输出可能较长）
    pub fn preprocess() -> Self {
        Self {
            max_tokens: 16_384,
            timeout_secs: 300,
        }
    }

    /// 结构化 JSON → Skill Markdown
    pub fn skill_generate() -> Self {
        Self {
            max_tokens: 8192,
            timeout_secs: 300,
        }
    }

    pub fn is_long_running(&self) -> bool {
        self.timeout_secs >= 180
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Ollama,
    OpenAi,
}

pub fn adapter_for(adapter_id: &str) -> AdapterKind {
    match adapter_id {
        "ollama" => AdapterKind::Ollama,
        _ => AdapterKind::OpenAi,
    }
}

pub async fn list_models(
    def: &VendorDefinition,
    kind: ModelKind,
    api_key: Option<&str>,
) -> Result<Vec<RemoteModel>, String> {
    match adapter_for(&def.adapter) {
        AdapterKind::Ollama => ollama::list_models(def, kind).await,
        AdapterKind::OpenAi => openai::list_models(def, kind, api_key).await,
    }
}

pub async fn test_connection(
    def: &VendorDefinition,
    api_key: Option<&str>,
) -> Result<String, String> {
    match adapter_for(&def.adapter) {
        AdapterKind::Ollama => ollama::test_connection(def).await,
        AdapterKind::OpenAi => openai::test_connection(def, api_key).await,
    }
}

pub async fn chat_text(
    def: &VendorDefinition,
    api_key: &str,
    model: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
) -> Result<String, String> {
    chat_text_with_options(
        def,
        api_key,
        model,
        system_prompt,
        user_prompt,
        ChatOptions::default(),
    )
    .await
}

pub async fn chat_text_with_options(
    def: &VendorDefinition,
    api_key: &str,
    model: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
    options: ChatOptions,
) -> Result<String, String> {
    match adapter_for(&def.adapter) {
        AdapterKind::Ollama => {
            ollama::chat_text_with_options(def, model, system_prompt, user_prompt, options).await
        }
        AdapterKind::OpenAi => {
            openai::chat_text_with_options(def, api_key, model, system_prompt, user_prompt, options)
                .await
        }
    }
}

pub async fn chat_vision(
    def: &VendorDefinition,
    api_key: &str,
    model: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
    image_data_url: &str,
) -> Result<String, String> {
    match adapter_for(&def.adapter) {
        AdapterKind::Ollama => {
            ollama::chat_vision(def, model, system_prompt, user_prompt, image_data_url).await
        }
        AdapterKind::OpenAi => {
            openai::chat_vision(def, api_key, model, system_prompt, user_prompt, image_data_url)
                .await
        }
    }
}
