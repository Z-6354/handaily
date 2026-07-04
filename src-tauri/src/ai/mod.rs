//! AI 供应商适配层：JSON 目录 + 适配器工厂 + 用户配置

pub mod adapter;
pub mod adapters;
pub mod catalog;
pub mod config;
pub mod providers;
pub mod json_util;
pub mod response;
pub mod runtime;
pub mod urls;

pub use adapter::{
    chat_text, chat_vision, is_text_ai_ready, load_catalog, PreparedTextChat, PreparedThinkingChat,
    VisionResult,
};
pub use config::{AiConfig, AiModelEntry, AiVendor, ModelKind, vendors_config_path};
pub use catalog::{seed_user_vendors, VendorCatalog, VendorDefinition};
