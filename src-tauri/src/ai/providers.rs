//! 供应商连接测试与模型拉取（委托适配器工厂）

use super::adapters;
use super::catalog::VendorDefinition;
use super::config::ModelKind;

pub use adapters::RemoteModel;

#[derive(Debug, Clone, serde::Serialize)]
pub struct VendorTestResult {
    pub ok: bool,
    pub message: String,
    #[serde(default)]
    pub imported_text: usize,
    #[serde(default)]
    pub imported_vision: usize,
}

pub async fn fetch_remote_models(
    def: &VendorDefinition,
    kind: ModelKind,
    api_key: Option<&str>,
) -> Result<Vec<RemoteModel>, String> {
    adapters::list_models(def, kind, api_key).await
}

pub async fn test_vendor_connection(
    def: &VendorDefinition,
    api_key: Option<&str>,
) -> VendorTestResult {
    match adapters::test_connection(def, api_key).await {
        Ok(msg) => VendorTestResult {
            ok: true,
            message: msg,
            imported_text: 0,
            imported_vision: 0,
        },
        Err(e) => VendorTestResult {
            ok: false,
            message: e,
            imported_text: 0,
            imported_vision: 0,
        },
    }
}

/// 从目录解析供应商定义（供 IPC 使用）
pub fn vendor_def<'a>(
    catalog: &'a super::catalog::VendorCatalog,
    vendor_id: &str,
) -> Result<&'a VendorDefinition, String> {
    catalog
        .vendor(vendor_id)
        .ok_or_else(|| format!("未知供应商: {vendor_id}"))
}
