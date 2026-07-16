//! [live2d-only] 密码本空实现（纯桌宠不存 API 密钥）

use rusqlite::Connection;

#[derive(Debug, Clone, Default)]
pub struct VaultStatus {
    pub configured: bool,
    pub unlocked: bool,
    pub has_password: bool,
}

#[derive(Default)]
pub struct VaultState;

impl VaultState {
    pub fn new() -> Self {
        Self
    }

    pub fn load_config(&self, _db: &Connection) -> Result<(), String> {
        Ok(())
    }

    pub fn lock(&self) {}

    pub fn status(&self) -> VaultStatus {
        VaultStatus::default()
    }
}
