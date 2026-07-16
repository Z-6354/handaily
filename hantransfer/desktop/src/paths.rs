use std::path::{Path, PathBuf};

const MODEL_INBOX: &str = "data/model";
const TRANSFER_INBOX: &str = "data/transfer/inbox";
const TRANSFER_HISTORY: &str = "data/transfer/history";
const TRANSFER_TEMP: &str = "data/transfer/temp";
const TRANSFER_OUTBOX: &str = "data/transfer/outbox";
const MOBILE_WEB: &str = "hantransfer/mobile-web";
const APK_RELEASE: &str = "hantransfer/release";

/// HANDAILY monorepo root (`hantransfer/desktop/` → `../..`).
pub fn project_root() -> PathBuf {
    if let Ok(p) = std::env::var("HANDAILY_ROOT") {
        let path = PathBuf::from(p.trim());
        if path.is_dir() {
            return path;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn model_inbox() -> PathBuf {
    project_root().join(MODEL_INBOX)
}

pub fn transfer_inbox() -> PathBuf {
    project_root().join(TRANSFER_INBOX)
}

pub fn transfer_history() -> PathBuf {
    project_root().join(TRANSFER_HISTORY)
}

pub fn transfer_temp() -> PathBuf {
    project_root().join(TRANSFER_TEMP)
}

pub fn transfer_outbox() -> PathBuf {
    project_root().join(TRANSFER_OUTBOX)
}

pub fn mobile_web_dir() -> PathBuf {
    project_root().join(MOBILE_WEB)
}

pub fn apk_release_dir() -> PathBuf {
    project_root().join(APK_RELEASE)
}

pub fn ensure_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_root_contains_hantransfer() {
        let root = project_root();
        assert!(root.join("hantransfer/proto/api.yaml").is_file());
        assert!(root.join("data/transfer").is_dir());
    }
}
