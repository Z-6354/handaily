//! 工作类型配置（可自定义）

use serde::{Deserialize, Serialize};

use crate::db;

const WORK_TYPES_KEY: &str = "work_types_config";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkType {
    pub id: String,
    pub name: String,
    pub color: String,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkTypeConfig {
    pub types: Vec<WorkType>,
}

impl Default for WorkTypeConfig {
    fn default() -> Self {
        Self {
            types: vec![
                wt("dev", "开发", "#4096ff", true),
                wt("meeting", "会议", "#9254de", true),
                wt("comm", "沟通", "#13c2c2", true),
                wt("doc", "文档", "#52c41a", true),
                wt("entertainment", "娱乐", "#faad14", true),
                wt("game", "游戏", "#f5222d", true),
                wt("leisure", "休闲", "#bfbfbf", true),
                wt("other", "其他", "#d9d9d9", true),
            ],
        }
    }
}

fn wt(id: &str, name: &str, color: &str, builtin: bool) -> WorkType {
    WorkType {
        id: id.into(),
        name: name.into(),
        color: color.into(),
        builtin,
    }
}

impl WorkTypeConfig {
    pub fn load(db: &rusqlite::Connection) -> Self {
        db::get_setting(db, WORK_TYPES_KEY)
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, db: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        let json = serde_json::to_string(self).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        })?;
        db::set_setting(db, WORK_TYPES_KEY, &json)
    }

    pub fn type_names(&self) -> Vec<String> {
        self.types.iter().map(|t| t.name.clone()).collect()
    }

    pub fn color_for(&self, name: &str) -> String {
        self.types
            .iter()
            .find(|t| t.name == name || t.id == name)
            .map(|t| t.color.clone())
            .unwrap_or_else(|| "#d9d9d9".into())
    }

    pub fn normalize_type_name(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if self.types.iter().any(|t| t.name == trimmed) {
            return trimmed.to_string();
        }
        // fuzzy match common aliases
        let lower = trimmed.to_lowercase();
        for t in &self.types {
            if t.id == lower || t.name.to_lowercase() == lower {
                return t.name.clone();
            }
        }
        self.types
            .last()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "其他".into())
    }
}
