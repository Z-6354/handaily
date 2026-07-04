//! 供应商目录：仓库 `config/vendors.json` 为源，运行时优先读用户数据目录副本

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const EMBEDDED: &str = include_str!("../../../config/vendors.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorCatalog {
    pub version: u32,
    #[serde(default)]
    pub defaults: CatalogDefaults,
    #[serde(default)]
    pub removed_vendor_ids: Vec<String>,
    pub vendors: Vec<VendorDefinition>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogDefaults {
    #[serde(default = "default_text_vendor")]
    pub text_vendor_id: String,
    #[serde(default = "default_vision_vendor")]
    pub vision_vendor_id: String,
}

fn default_text_vendor() -> String {
    "ollama".into()
}
fn default_vision_vendor() -> String {
    "ollama".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorDefinition {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub adapter: String,
    #[serde(default = "default_true")]
    pub requires_api_key: bool,
    #[serde(default)]
    pub test: VendorTestConfig,
    #[serde(default)]
    pub hints: VendorHints,
    #[serde(default)]
    pub default_models: VendorDefaultModels,
    /// 导入时忽略、并从本地已保存列表中移除的模型 ID
    #[serde(default)]
    pub excluded_models: Vec<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VendorTestConfig {
    /// list_models | openai_or_ping
    #[serde(default = "default_test_strategy")]
    pub strategy: String,
    pub ping_url: Option<String>,
    pub plan_label: Option<String>,
}

fn default_test_strategy() -> String {
    "list_models".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VendorHints {
    #[serde(default)]
    pub empty_models: Option<String>,
    #[serde(default)]
    pub auth_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VendorDefaultModels {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub vision: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
}

pub fn config_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("config")
}

pub fn vendors_path(data_dir: &Path) -> PathBuf {
    config_dir(data_dir).join("vendors.json")
}

/// 首次启动：将内置目录复制到用户数据目录（已存在则不覆盖，便于用户自定义）
pub fn seed_user_vendors(data_dir: &Path) -> std::io::Result<()> {
    let dir = config_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let path = vendors_path(data_dir);
    if !path.exists() {
        fs::write(&path, EMBEDDED)?;
    }
    Ok(())
}

pub fn load(data_dir: &Path) -> VendorCatalog {
    let embedded = default_embedded();
    let mut catalog = if let Ok(raw) = fs::read_to_string(vendors_path(data_dir)) {
        serde_json::from_str(&raw).unwrap_or_else(|_| embedded.clone())
    } else {
        embedded.clone()
    };
    for id in &embedded.removed_vendor_ids {
        if !catalog.removed_vendor_ids.iter().any(|x| x == id) {
            catalog.removed_vendor_ids.push(id.clone());
        }
    }
    catalog
        .vendors
        .retain(|v| !catalog.removed_vendor_ids.iter().any(|x| x == &v.id));
    patch_defaults_from_embedded(&mut catalog, &embedded);
    patch_vendor_urls_from_embedded(&mut catalog, &embedded);
    catalog
}

fn patch_vendor_urls_from_embedded(catalog: &mut VendorCatalog, embedded: &VendorCatalog) {
    const OPENCODE_LEGACY: &str = "https://api.opencode.ai/v1";

    for emb in &embedded.vendors {
        let Some(v) = catalog.vendors.iter_mut().find(|v| v.id == emb.id) else {
            continue;
        };
        let user_base = v.base_url.trim_end_matches('/');
        let legacy = OPENCODE_LEGACY.trim_end_matches('/');
        if emb.id == "opencode" && user_base == legacy {
            v.base_url = emb.base_url.clone();
        }
        if v.hints.auth_error.is_none() && emb.hints.auth_error.is_some() {
            v.hints.auth_error = emb.hints.auth_error.clone();
        }
        if v.hints.empty_models.is_none() && emb.hints.empty_models.is_some() {
            v.hints.empty_models = emb.hints.empty_models.clone();
        }
    }
}

fn patch_defaults_from_embedded(catalog: &mut VendorCatalog, embedded: &VendorCatalog) {
    for emb in &embedded.vendors {
        let Some(v) = catalog.vendors.iter_mut().find(|v| v.id == emb.id) else {
            continue;
        };
        if v.default_models.text.is_none() {
            v.default_models.text = emb.default_models.text.clone();
        }
        if v.default_models.vision.is_none() {
            v.default_models.vision = emb.default_models.vision.clone();
        }
        if v.default_models.thinking.is_none() {
            v.default_models.thinking = emb.default_models.thinking.clone();
        }
        if v.excluded_models.is_empty() && !emb.excluded_models.is_empty() {
            v.excluded_models = emb.excluded_models.clone();
        }
    }
}

fn default_embedded() -> VendorCatalog {
    serde_json::from_str(EMBEDDED).expect("embedded vendors.json")
}

impl Default for VendorCatalog {
    fn default() -> Self {
        default_embedded()
    }
}

impl VendorCatalog {
    pub fn vendor(&self, id: &str) -> Option<&VendorDefinition> {
        self.vendors.iter().find(|v| v.id == id)
    }

    pub fn default_model_id(&self, vendor_id: &str, kind: super::config::ModelKind) -> Option<String> {
        let def = self.vendor(vendor_id)?;
        match kind {
            super::config::ModelKind::Text => def.default_models.text.clone(),
            super::config::ModelKind::Vision => def.default_models.vision.clone(),
            super::config::ModelKind::Thinking => def
                .default_models
                .thinking
                .clone()
                .or_else(|| def.default_models.text.clone()),
        }
    }

    pub fn is_model_excluded(&self, vendor_id: &str, model_id: &str) -> bool {
        self.vendor(vendor_id)
            .is_some_and(|v| v.excluded_models.iter().any(|x| x == model_id))
    }

    pub fn vendor_map(&self) -> HashMap<&str, &VendorDefinition> {
        self.vendors.iter().map(|v| (v.id.as_str(), v)).collect()
    }

    pub fn is_removed(&self, id: &str) -> bool {
        self.removed_vendor_ids.iter().any(|x| x == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn embedded_catalog_parses() {
        let c = load(&env::temp_dir());
        assert!(!c.vendors.is_empty());
        assert!(c.vendor("ollama").is_some());
    }

    #[test]
    fn seed_user_copy() {
        let base = env::temp_dir().join(format!("xiaohan-vendors-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        seed_user_vendors(&base).unwrap();
        assert!(vendors_path(&base).exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migrates_legacy_opencode_base_url() {
        let base = env::temp_dir().join(format!("xiaohan-vendors-oc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(config_dir(&base)).unwrap();
        let legacy = r#"{
  "version": 1,
  "defaults": { "text_vendor_id": "ollama", "vision_vendor_id": "ollama" },
  "removed_vendor_ids": [],
  "vendors": [{
    "id": "opencode",
    "name": "OpenCode GO 套餐",
    "base_url": "https://api.opencode.ai/v1",
    "adapter": "openai",
    "requires_api_key": true
  }]
}"#;
        fs::write(vendors_path(&base), legacy).unwrap();
        let c = load(&base);
        let oc = c.vendor("opencode").unwrap();
        assert_eq!(oc.base_url, "https://opencode.ai/zen/go/v1");
        let _ = fs::remove_dir_all(&base);
    }
}
