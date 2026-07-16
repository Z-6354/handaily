use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::config::Config;
use crate::paths;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferType {
    File,
    AzurlaneAsset,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FileMetadata {
    pub filename: String,
    pub size: u64,
    pub hash: String,
    #[serde(rename = "type")]
    pub transfer_type: TransferType,
    pub source: String,
    pub category: Option<String>,
    pub relative_path: Option<String>,
}

/// Resolve inbox destination from metadata type.
pub fn inbox_destination(base_inbox: &Path, meta: &FileMetadata) -> PathBuf {
    match meta.transfer_type {
        TransferType::File => base_inbox.to_path_buf(),
        TransferType::AzurlaneAsset => {
            let category = sanitize_category(meta.category.as_deref());
            base_inbox.join("azurlane").join(category)
        }
    }
}

/// Keep category as a single safe folder name (no `..` / separators).
pub fn sanitize_category(raw: Option<&str>) -> String {
    let s = raw.unwrap_or("custom").trim();
    if s.is_empty() || s.contains('/') || s.contains('\\') || s.contains("..") {
        return "custom".into();
    }
    if s == "." || s.starts_with('.') {
        return "custom".into();
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return "custom".into();
    }
    s.to_string()
}

pub fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        paths::ensure_dir(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn after_receive(_config: &Config, meta: &FileMetadata, final_path: &Path) {
    crate::notify::notify_file_received(meta, final_path);
    log_hanimport_hint(meta, final_path);
    maybe_spawn_hanimport_unpack(meta, final_path);
}

fn log_hanimport_hint(meta: &FileMetadata, final_path: &Path) {
    if meta.transfer_type != TransferType::AzurlaneAsset {
        return;
    }
    let Some(input_dir) = final_path.parent() else {
        return;
    };
    tracing::info!(
        filename = %meta.filename,
        path = %final_path.display(),
        category = meta.category.as_deref().unwrap_or("custom"),
        "azurlane_asset received — hanimport: cargo run -p hanimport -- unpack --input {}",
        input_dir.display()
    );
}

fn maybe_spawn_hanimport_unpack(meta: &FileMetadata, final_path: &Path) {
    if meta.transfer_type != TransferType::AzurlaneAsset {
        return;
    }
    if !env_truthy("HANTRANSFER_AUTO_HANIMPORT") {
        return;
    }
    let Some(input_dir) = final_path.parent() else {
        return;
    };
    let root = paths::project_root();
    if !root.join("hanimport/Cargo.toml").is_file() {
        tracing::warn!("HANTRANSFER_AUTO_HANIMPORT=1 but hanimport crate not found");
        return;
    }
    let input = input_dir.to_path_buf();
    std::thread::spawn(move || {
        tracing::info!(input = %input.display(), "spawning hanimport unpack");
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&root).args([
            "run",
            "-p",
            "hanimport",
            "--",
            "unpack",
            "--input",
            &input.display().to_string(),
        ]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        match cmd.status()
        {
            Ok(status) if status.success() => {
                tracing::info!(input = %input.display(), "hanimport unpack finished");
            }
            Ok(status) => {
                tracing::warn!(
                    input = %input.display(),
                    code = ?status.code(),
                    "hanimport unpack failed"
                );
            }
            Err(err) => {
                tracing::warn!(input = %input.display(), error = %err, "hanimport spawn failed");
            }
        }
    });
}

fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azurlane_routes_to_subfolder() {
        let base = PathBuf::from("/inbox");
        let meta = FileMetadata {
            filename: "x.ab".into(),
            size: 1,
            hash: "sha256:00".into(),
            transfer_type: TransferType::AzurlaneAsset,
            source: "phone".into(),
            category: Some("live2d".into()),
            relative_path: None,
        };
        assert_eq!(
            inbox_destination(&base, &meta),
            PathBuf::from("/inbox/azurlane/live2d")
        );
    }

    #[test]
    fn category_traversal_falls_back_to_custom() {
        let base = PathBuf::from("/inbox");
        for bad in ["../../../etc", "..\\..\\x", "/abs", "a/b", "", ".", ".."] {
            let meta = FileMetadata {
                filename: "x.ab".into(),
                size: 1,
                hash: "sha256:00".into(),
                transfer_type: TransferType::AzurlaneAsset,
                source: "phone".into(),
                category: Some(bad.into()),
                relative_path: None,
            };
            assert_eq!(
                inbox_destination(&base, &meta),
                PathBuf::from("/inbox/azurlane/custom"),
                "bad category {bad:?}"
            );
        }
    }
}
