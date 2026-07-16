//! 角色资源包：zip 导出 / 导入（人设 + 模型 + 台词）

mod export;
mod import;

pub use export::{export_all_packs, ExportedPackInfo, PackExportSummary};
pub use import::{import_from_zip, RosterPackImportProgress, RosterPackImportResult};

use serde::{Deserialize, Serialize};

pub const PACK_FORMAT: &str = "handaily-roster-pack";
pub const PACK_VERSION: u32 = 1;
pub const META_FILENAME: &str = "handaily-roster-pack.json";

/// 主阵营独立分包；其余（未分类、维希教廷、北方联合等）暂归入「其他」
pub const MAIN_FACTIONS: &[&str] = &["皇家", "白鹰", "重樱", "铁血"];

pub const PACK_FULL: &str = "模型-完整角色包";
pub const PACK_OTHER: &str = "模型-其他角色包";
pub const PACK_CHESHIRE: &str = "模型-柴郡角色包";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackMeta {
    pub format: String,
    pub version: u32,
    pub pack_kind: String,
    pub pack_id: String,
    pub pack_label: String,
    pub character_count: u32,
    pub model_count: u32,
    pub exported_at: String,
}

pub fn faction_pack_name(faction: &str) -> String {
    format!("模型-{faction}阵营角色包")
}
