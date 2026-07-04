//! AI 用户配置（SQLite）与供应商目录（JSON）合并

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db;

use super::catalog::{self, VendorCatalog, VendorDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Text,
    Vision,
    Thinking,
}

/// IPC / 前端使用的合并视图（供应商定义来自 JSON，密钥绑定来自 SQLite）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub text_vendor_id: String,
    pub text_model: String,
    pub vision_vendor_id: String,
    pub vision_model: String,
    pub thinking_vendor_id: String,
    pub thinking_model: String,
    pub vendors: Vec<AiVendor>,
    pub custom_models: Vec<AiModelEntry>,
    #[serde(default)]
    pub imported_models: Vec<AiModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiVendor {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_style: String,
    pub vault_entry_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModelEntry {
    pub id: String,
    pub name: String,
    pub vendor_id: String,
    pub kind: ModelKind,
    pub custom: bool,
}

/// 仅持久化用户选择，不含供应商静态定义
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAiState {
    #[serde(default = "default_ollama")]
    text_vendor_id: String,
    #[serde(default)]
    text_model: String,
    #[serde(default = "default_ollama")]
    vision_vendor_id: String,
    #[serde(default)]
    vision_model: String,
    #[serde(default = "default_ollama")]
    thinking_vendor_id: String,
    #[serde(default)]
    thinking_model: String,
    #[serde(default)]
    vendor_vault: HashMap<String, Option<i64>>,
    #[serde(default)]
    custom_models: Vec<AiModelEntry>,
    #[serde(default)]
    imported_models: Vec<AiModelEntry>,
    /// 旧版整包配置，仅用于迁移 vault 绑定
    #[serde(default)]
    vendors: Vec<AiVendor>,
}

fn default_ollama() -> String {
    "ollama".into()
}

const AI_CONFIG_KEY: &str = "ai_config";

impl AiConfig {
    pub fn load(db: &Connection, data_dir: &Path) -> Self {
        let catalog = catalog::load(data_dir);
        let mut stored = StoredAiState::load(db);
        let mut dirty = stored.migrate(&catalog);
        dirty |= apply_vendor_defaults(&mut stored, &catalog);
        let merged = Self::merge(&catalog, &stored);
        if sanitize_selection(&catalog, &merged, &mut stored) {
            dirty = true;
        }
        if dirty {
            let _ = stored.save(db);
        }
        Self::merge(&catalog, &stored)
    }

    pub fn save(&self, db: &Connection) -> Result<(), rusqlite::Error> {
        let existing = StoredAiState::load(db);
        let mut stored = StoredAiState::from_ipc(self);
        // 前端 partial save 未带 imported_models 时保留已有列表，避免误清空
        if stored.imported_models.is_empty() && !existing.imported_models.is_empty() {
            stored.imported_models = existing.imported_models;
        }
        stored.save(db)
    }

    pub fn vendor(&self, id: &str) -> Option<&AiVendor> {
        self.vendors.iter().find(|v| v.id == id)
    }

    pub fn vendor_def<'a>(&self, catalog: &'a VendorCatalog, id: &str) -> Option<&'a VendorDefinition> {
        catalog.vendor(id)
    }

    pub fn models_for(
        &self,
        catalog: &VendorCatalog,
        vendor_id: &str,
        kind: ModelKind,
    ) -> Vec<AiModelEntry> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        if let Some(id) = catalog.default_model_id(vendor_id, kind) {
            seen.insert(id.clone());
            out.push(AiModelEntry {
                id: id.clone(),
                name: id,
                vendor_id: vendor_id.to_string(),
                kind,
                custom: false,
            });
        }
        for m in self
            .custom_models
            .iter()
            .chain(self.imported_models.iter())
            .filter(|m| m.vendor_id == vendor_id && m.kind == kind)
        {
            if seen.insert(m.id.clone()) {
                out.push(m.clone());
            }
        }
        out
    }

    pub fn resolve_model(&self, vendor_id: &str, model_id: &str, _kind: ModelKind) -> String {
        if let Some(m) = self
            .custom_models
            .iter()
            .chain(self.imported_models.iter())
            .find(|m| m.vendor_id == vendor_id && m.id == model_id)
        {
            return m.id.clone();
        }
        model_id.to_string()
    }

    fn merge(catalog: &VendorCatalog, stored: &StoredAiState) -> Self {
        let vendors = catalog
            .vendors
            .iter()
            .map(|def| AiVendor {
                id: def.id.clone(),
                name: def.name.clone(),
                base_url: def.base_url.clone(),
                api_style: api_style_from_adapter(&def.adapter),
                vault_entry_id: stored.vendor_vault.get(&def.id).copied().flatten(),
            })
            .collect();
        Self {
            text_vendor_id: stored.text_vendor_id.clone(),
            text_model: stored.text_model.clone(),
            vision_vendor_id: stored.vision_vendor_id.clone(),
            vision_model: stored.vision_model.clone(),
            thinking_vendor_id: stored.thinking_vendor_id.clone(),
            thinking_model: stored.thinking_model.clone(),
            vendors,
            custom_models: stored.custom_models.clone(),
            imported_models: stored.imported_models.clone(),
        }
    }
}

impl StoredAiState {
    fn load(db: &Connection) -> Self {
        match db::get_setting(db, AI_CONFIG_KEY) {
            Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            None => Self::default(),
        }
    }

    fn save(&self, db: &Connection) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(self).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        })?;
        db::set_setting(db, AI_CONFIG_KEY, &json)
    }

    fn from_ipc(cfg: &AiConfig) -> Self {
        let vendor_vault = cfg
            .vendors
            .iter()
            .map(|v| (v.id.clone(), v.vault_entry_id))
            .collect();
        Self {
            text_vendor_id: cfg.text_vendor_id.clone(),
            text_model: cfg.text_model.clone(),
            vision_vendor_id: cfg.vision_vendor_id.clone(),
            vision_model: cfg.vision_model.clone(),
            thinking_vendor_id: cfg.thinking_vendor_id.clone(),
            thinking_model: cfg.thinking_model.clone(),
            vendor_vault,
            custom_models: cfg.custom_models.clone(),
            imported_models: cfg.imported_models.clone(),
            vendors: vec![],
        }
    }

    fn migrate(&mut self, catalog: &VendorCatalog) -> bool {
        let mut changed = false;

        if self.vendor_vault.is_empty() && !self.vendors.is_empty() {
            for v in &self.vendors {
                self.vendor_vault.insert(v.id.clone(), v.vault_entry_id);
            }
            self.vendors.clear();
            changed = true;
        }

        for removed in &catalog.removed_vendor_ids {
            if self.text_vendor_id == *removed {
                self.text_vendor_id = catalog.defaults.text_vendor_id.clone();
                self.text_model.clear();
                changed = true;
            }
            if self.vision_vendor_id == *removed {
                self.vision_vendor_id = catalog.defaults.vision_vendor_id.clone();
                self.vision_model.clear();
                changed = true;
            }
            if self.thinking_vendor_id == *removed {
                self.thinking_vendor_id = catalog.defaults.text_vendor_id.clone();
                self.thinking_model.clear();
                changed = true;
            }
            let before = self.custom_models.len();
            self.custom_models.retain(|m| m.vendor_id != *removed);
            if self.custom_models.len() != before {
                changed = true;
            }
            let ib = self.imported_models.len();
            self.imported_models.retain(|m| m.vendor_id != *removed);
            if self.imported_models.len() != ib {
                changed = true;
            }
            if self.vendor_vault.remove(removed).is_some() {
                changed = true;
            }
        }

        changed |= self.purge_excluded_models(catalog);

        // 旧版配置无思考模型：默认跟随文本模型供应商
        if self.thinking_model.is_empty() {
            if self.thinking_vendor_id.is_empty()
                || self.thinking_vendor_id == default_ollama()
            {
                self.thinking_vendor_id = self.text_vendor_id.clone();
                changed = true;
            }
        }

        changed
    }

    fn purge_excluded_models(&mut self, catalog: &VendorCatalog) -> bool {
        let mut changed = false;
        let ib = self.imported_models.len();
        self.imported_models.retain(|m| !catalog.is_model_excluded(&m.vendor_id, &m.id));
        if self.imported_models.len() != ib {
            changed = true;
        }
        let cb = self.custom_models.len();
        self.custom_models
            .retain(|m| !catalog.is_model_excluded(&m.vendor_id, &m.id));
        if self.custom_models.len() != cb {
            changed = true;
        }
        if catalog.is_model_excluded(&self.text_vendor_id, &self.text_model) {
            self.text_model.clear();
            changed = true;
        }
        if catalog.is_model_excluded(&self.vision_vendor_id, &self.vision_model) {
            self.vision_model.clear();
            changed = true;
        }
        if catalog.is_model_excluded(&self.thinking_vendor_id, &self.thinking_model) {
            self.thinking_model.clear();
            changed = true;
        }
        changed
    }
}

impl Default for StoredAiState {
    fn default() -> Self {
        Self {
            text_vendor_id: "ollama".into(),
            text_model: String::new(),
            vision_vendor_id: "ollama".into(),
            vision_model: String::new(),
            thinking_vendor_id: "ollama".into(),
            thinking_model: String::new(),
            vendor_vault: HashMap::new(),
            custom_models: vec![],
            imported_models: vec![],
            vendors: vec![],
        }
    }
}

fn apply_vendor_defaults(stored: &mut StoredAiState, catalog: &VendorCatalog) -> bool {
    let mut changed = false;
    if stored.text_model.is_empty() {
        if let Some(id) = catalog.default_model_id(&stored.text_vendor_id, ModelKind::Text) {
            stored.text_model = id;
            changed = true;
        }
    }
    if stored.vision_model.is_empty() {
        if let Some(id) = catalog.default_model_id(&stored.vision_vendor_id, ModelKind::Vision) {
            stored.vision_model = id;
            changed = true;
        }
    }
    if stored.thinking_model.is_empty() {
        if let Some(id) = catalog.default_model_id(&stored.thinking_vendor_id, ModelKind::Thinking) {
            stored.thinking_model = id;
            changed = true;
        } else if !stored.text_model.is_empty() {
            stored.thinking_vendor_id = stored.text_vendor_id.clone();
            stored.thinking_model = stored.text_model.clone();
            changed = true;
        }
    }
    changed
}

fn sanitize_selection(
    catalog: &VendorCatalog,
    merged: &AiConfig,
    stored: &mut StoredAiState,
) -> bool {
    let mut changed = false;
    let text_ok = merged
        .models_for(catalog, &stored.text_vendor_id, ModelKind::Text)
        .iter()
        .any(|m| m.id == stored.text_model);
    if !stored.text_model.is_empty() && !text_ok {
        stored.text_model.clear();
        changed = true;
    }
    let vision_ok = merged
        .models_for(catalog, &stored.vision_vendor_id, ModelKind::Vision)
        .iter()
        .any(|m| m.id == stored.vision_model);
    if !stored.vision_model.is_empty() && !vision_ok {
        stored.vision_model.clear();
        changed = true;
    }
    let thinking_ok = merged
        .models_for(catalog, &stored.thinking_vendor_id, ModelKind::Thinking)
        .iter()
        .any(|m| m.id == stored.thinking_model);
    if !stored.thinking_model.is_empty() && !thinking_ok {
        stored.thinking_model.clear();
        changed = true;
    }
    if stored.text_model.is_empty() {
        changed |= apply_vendor_defaults(stored, catalog);
    }
    if stored.vision_model.is_empty() {
        changed |= apply_vendor_defaults(stored, catalog);
    }
    if stored.thinking_model.is_empty() {
        changed |= apply_vendor_defaults(stored, catalog);
    }
    changed
}

fn api_style_from_adapter(adapter: &str) -> String {
    match adapter {
        "ollama" => "ollama".into(),
        _ => "openai".into(),
    }
}

pub fn vendors_config_path(data_dir: &Path) -> std::path::PathBuf {
    catalog::vendors_path(data_dir)
}
