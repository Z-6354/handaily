//! Agent MCP HTTP API 开关（本地 DB 持久化，默认关闭）

pub const SETTING_KEY: &str = "mcp_api_enabled";

pub fn is_enabled(db: &rusqlite::Connection) -> bool {
    crate::db::get_setting(db, SETTING_KEY).as_deref() == Some("1")
}

pub fn set_enabled(db: &rusqlite::Connection, enabled: bool) -> Result<(), String> {
    crate::db::set_setting(db, SETTING_KEY, if enabled { "1" } else { "0" })
        .map_err(|e| e.to_string())
}

/// 是否应在启动时拉起本地 HTTP 控制面（`HANDAILY_DISABLE_TEST_API=1` 强制关闭）
pub fn should_spawn(db: &rusqlite::Connection) -> bool {
    if std::env::var("HANDAILY_DISABLE_TEST_API").is_ok() {
        return false;
    }
    if cfg!(debug_assertions) {
        return true;
    }
    is_enabled(db)
}
