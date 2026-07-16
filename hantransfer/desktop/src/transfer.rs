use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::Config;
use crate::importer::{self, FileMetadata};
use crate::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Receiving,
    Verifying,
    PendingApproval,
    Done,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferProgress {
    pub transfer_id: Uuid,
    pub status: TransferStatus,
    pub bytes_received: u64,
    pub total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    pub path: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Default)]
pub struct TransferRegistry {
    inner: Arc<Mutex<HashMap<Uuid, TransferProgress>>>,
}

impl TransferRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, progress: TransferProgress) {
        self.inner
            .lock()
            .expect("transfer lock")
            .insert(progress.transfer_id, progress);
    }

    pub fn get(&self, id: &Uuid) -> Option<TransferProgress> {
        self.inner.lock().expect("transfer lock").get(id).cloned()
    }

    pub fn list_active(&self) -> Vec<TransferProgress> {
        self.inner
            .lock()
            .expect("transfer lock")
            .values()
            .filter(|p| {
                matches!(
                    p.status,
                    TransferStatus::Receiving
                        | TransferStatus::Verifying
                        | TransferStatus::PendingApproval
                )
            })
            .cloned()
            .collect()
    }

    pub fn update_bytes(&self, id: &Uuid, bytes_received: u64) {
        if let Some(entry) = self.inner.lock().expect("transfer lock").get_mut(id) {
            entry.bytes_received = bytes_received;
            entry.status = TransferStatus::Receiving;
        }
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    format!("sha256:{digest:x}")
}

pub fn sanitize_filename(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') {
        return Err("invalid filename".into());
    }
    Ok(trimmed.to_string())
}

pub fn client_hash_required(hash: &str) -> bool {
    !hash.is_empty() && hash != "sha256:server" && hash != "sha256:pending"
}

pub fn receive_file(
    config: &Config,
    transfer_id: Uuid,
    meta: &FileMetadata,
    file_bytes: &[u8],
    registry: &TransferRegistry,
) -> Result<PathBuf, String> {
    let temp_path = config.temp_dir.join(format!("{transfer_id}.part"));
    fs::write(&temp_path, file_bytes).map_err(|e| e.to_string())?;
    let computed_hash = sha256_hex(file_bytes);
    finalize_received_file(
        config,
        &config.inbox_dir,
        transfer_id,
        meta,
        &temp_path,
        computed_hash,
        registry,
    )
}

pub fn finalize_received_file(
    config: &Config,
    inbox_dir: &Path,
    transfer_id: Uuid,
    meta: &FileMetadata,
    temp_path: &Path,
    computed_hash: String,
    registry: &TransferRegistry,
) -> Result<PathBuf, String> {
    let on_disk = fs::metadata(temp_path).map_err(|e| e.to_string())?;
    let bytes_received = on_disk.len();

    registry.set(TransferProgress {
        transfer_id,
        status: TransferStatus::Receiving,
        bytes_received,
        total: meta.size,
        filename: Some(meta.filename.clone()),
        device_name: None,
        path: None,
        error: None,
    });

    if bytes_received != meta.size {
        cleanup_temp(temp_path);
        return fail(registry, transfer_id, "size mismatch");
    }

    registry.set(TransferProgress {
        transfer_id,
        status: TransferStatus::Verifying,
        bytes_received: meta.size,
        total: meta.size,
        filename: Some(meta.filename.clone()),
        device_name: None,
        path: None,
        error: None,
    });

    if client_hash_required(&meta.hash) && computed_hash != meta.hash {
        cleanup_temp(temp_path);
        return fail(registry, transfer_id, "hash mismatch");
    }

    let filename = sanitize_filename(&meta.filename)?;
    let dest_dir = importer::inbox_destination(inbox_dir, meta);
    importer::ensure_parent(&dest_dir.join(&filename))?;
    paths::ensure_dir(&dest_dir).map_err(|e| e.to_string())?;

    // Same path + same size → keep existing file, drop the temp upload.
    if let Some(existing) = existing_same_size(&dest_dir, &filename, meta.size) {
        cleanup_temp(temp_path);
        let stored_hash = if client_hash_required(&meta.hash) {
            meta.hash.clone()
        } else {
            computed_hash.clone()
        };
        append_history(
            &config.history_dir,
            transfer_id,
            meta,
            &existing,
            &stored_hash,
        )?;
        registry.set(TransferProgress {
            transfer_id,
            status: TransferStatus::Done,
            bytes_received: meta.size,
            total: meta.size,
            filename: Some(meta.filename.clone()),
            device_name: None,
            path: Some(existing.display().to_string()),
            error: Some("skipped_duplicate".into()),
        });
        tracing::info!(
            transfer_id = %transfer_id,
            path = %existing.display(),
            size = meta.size,
            "receive skipped duplicate (same name+size)"
        );
        return Ok(existing);
    }

    let final_path = unique_path(&dest_dir, &filename);
    if let Err(err) = fs::rename(temp_path, &final_path) {
        cleanup_temp(temp_path);
        return fail(registry, transfer_id, &err.to_string());
    }

    let stored_hash = if client_hash_required(&meta.hash) {
        meta.hash.clone()
    } else {
        computed_hash.clone()
    };
    append_history(
        &config.history_dir,
        transfer_id,
        meta,
        &final_path,
        &stored_hash,
    )?;

    registry.set(TransferProgress {
        transfer_id,
        status: TransferStatus::Done,
        bytes_received: meta.size,
        total: meta.size,
        filename: Some(meta.filename.clone()),
        device_name: None,
        path: Some(final_path.display().to_string()),
        error: None,
    });

    importer::after_receive(config, meta, &final_path);

    Ok(final_path)
}

pub fn cleanup_temp(path: &Path) {
    let _ = fs::remove_file(path);
}

pub fn cleanup_stale_temp(temp_dir: &Path, max_age: std::time::Duration) {
    let Ok(entries) = fs::read_dir(temp_dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("part") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if now.duration_since(modified).unwrap_or_default() > max_age {
            let _ = fs::remove_file(&path);
            tracing::info!(path = %path.display(), "removed stale temp file");
        }
    }
}

/// If `dir/filename` already exists with exactly `size` bytes, return that path.
pub fn existing_same_size(dir: &Path, filename: &str, size: u64) -> Option<PathBuf> {
    let candidate = dir.join(filename);
    let meta = fs::metadata(&candidate).ok()?;
    if meta.is_file() && meta.len() == size {
        Some(candidate)
    } else {
        None
    }
}

/// Public helper for preflight checks (upload skip before body transfer).
pub fn inbox_duplicate_path(inbox_dir: &Path, meta: &FileMetadata) -> Option<PathBuf> {
    let filename = sanitize_filename(&meta.filename).ok()?;
    let dest_dir = importer::inbox_destination(inbox_dir, meta);
    existing_same_size(&dest_dir, &filename, meta.size)
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let mut candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    for i in 1..1000 {
        candidate = dir.join(format!("{stem}_{i}{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}_{}", Uuid::new_v4()))
}

fn append_history(
    history_dir: &Path,
    transfer_id: Uuid,
    meta: &FileMetadata,
    final_path: &Path,
    hash: &str,
) -> Result<(), String> {
    paths::ensure_dir(history_dir).map_err(|e| e.to_string())?;
    let file = history_dir.join("transfers.jsonl");
    let line = serde_json::json!({
        "transfer_id": transfer_id,
        "filename": meta.filename,
        "size": meta.size,
        "hash": hash,
        "type": meta.transfer_type,
        "path": final_path.display().to_string(),
        "at": crate::trust::now_iso(),
    });
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())
}

fn fail(registry: &TransferRegistry, transfer_id: Uuid, message: &str) -> Result<PathBuf, String> {
    registry.set(TransferProgress {
        transfer_id,
        status: TransferStatus::Failed,
        bytes_received: 0,
        total: 0,
        filename: None,
        device_name: None,
        path: None,
        error: Some(message.to_string()),
    });
    Err(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::TransferType;
    use tempfile::tempdir;

    #[test]
    fn server_hash_skips_client_mismatch() {
        let dir = tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        let history = dir.path().join("history");
        let temp = dir.path().join("temp");
        for d in [&inbox, &history, &temp] {
            fs::create_dir_all(d).unwrap();
        }

        let config = Config {
            device_name: "HAN-PC".into(),
            device_id: Uuid::new_v4(),
            port: 7822,
            lan_ipv4: None,
            inbox_dir: inbox.clone(),
            history_dir: history,
            temp_dir: temp.clone(),
            outbox_dir: dir.path().join("outbox"),
        };

        let bytes = b"payload";
        let temp_path = temp.join("x.part");
        fs::write(&temp_path, bytes).unwrap();
        let computed = sha256_hex(bytes);
        let meta = FileMetadata {
            filename: "test.bin".into(),
            size: bytes.len() as u64,
            hash: "sha256:server".into(),
            transfer_type: TransferType::File,
            source: "test".into(),
            category: None,
            relative_path: None,
        };

        let registry = TransferRegistry::new();
        let id = Uuid::new_v4();
        let path =
            finalize_received_file(&config, &inbox, id, &meta, &temp_path, computed, &registry).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn hash_format() {
        let h = sha256_hex(b"hello");
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), 7 + 64);
    }

    #[test]
    fn receive_writes_file() {
        let dir = tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        let history = dir.path().join("history");
        let temp = dir.path().join("temp");
        for d in [&inbox, &history, &temp] {
            fs::create_dir_all(d).unwrap();
        }

        let config = Config {
            device_name: "HAN-PC".into(),
            device_id: Uuid::new_v4(),
            port: 7822,
            lan_ipv4: None,
            inbox_dir: inbox.clone(),
            history_dir: history,
            temp_dir: temp,
            outbox_dir: dir.path().join("outbox"),
        };

        let bytes = b"payload";
        let hash = sha256_hex(bytes);
        let meta = FileMetadata {
            filename: "test.bin".into(),
            size: bytes.len() as u64,
            hash,
            transfer_type: TransferType::File,
            source: "test".into(),
            category: None,
            relative_path: None,
        };

        let registry = TransferRegistry::new();
        let id = Uuid::new_v4();
        let path = receive_file(&config, id, &meta, bytes, &registry).unwrap();
        assert!(path.exists());
        assert_eq!(registry.get(&id).unwrap().status, TransferStatus::Done);
    }

    #[test]
    fn same_name_same_size_skips_rewrite() {
        let dir = tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        let history = dir.path().join("history");
        let temp = dir.path().join("temp");
        for d in [&inbox, &history, &temp] {
            fs::create_dir_all(d).unwrap();
        }

        let config = Config {
            device_name: "HAN-PC".into(),
            device_id: Uuid::new_v4(),
            port: 7822,
            lan_ipv4: None,
            inbox_dir: inbox.clone(),
            history_dir: history,
            temp_dir: temp.clone(),
            outbox_dir: dir.path().join("outbox"),
        };

        let bytes = b"payload-v1";
        let dest = inbox.join("dup.bin");
        fs::write(&dest, bytes).unwrap();
        let mtime = fs::metadata(&dest).unwrap().modified().unwrap();

        let temp_path = temp.join("dup.part");
        fs::write(&temp_path, bytes).unwrap();
        let meta = FileMetadata {
            filename: "dup.bin".into(),
            size: bytes.len() as u64,
            hash: "sha256:server".into(),
            transfer_type: TransferType::File,
            source: "test".into(),
            category: None,
            relative_path: None,
        };

        let registry = TransferRegistry::new();
        let id = Uuid::new_v4();
        let path = finalize_received_file(
            &config,
            &inbox,
            id,
            &meta,
            &temp_path,
            sha256_hex(bytes),
            &registry,
        )
        .unwrap();

        assert_eq!(path, dest);
        assert!(!temp_path.exists());
        assert!(!inbox.join("dup_1.bin").exists());
        assert_eq!(fs::read(&dest).unwrap(), bytes);
        assert_eq!(fs::metadata(&dest).unwrap().modified().unwrap(), mtime);
        assert_eq!(registry.get(&id).unwrap().status, TransferStatus::Done);
        assert_eq!(
            registry.get(&id).unwrap().error.as_deref(),
            Some("skipped_duplicate")
        );
    }

    #[test]
    fn same_name_different_size_keeps_unique_copy() {
        let dir = tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        let history = dir.path().join("history");
        let temp = dir.path().join("temp");
        for d in [&inbox, &history, &temp] {
            fs::create_dir_all(d).unwrap();
        }

        let config = Config {
            device_name: "HAN-PC".into(),
            device_id: Uuid::new_v4(),
            port: 7822,
            lan_ipv4: None,
            inbox_dir: inbox.clone(),
            history_dir: history,
            temp_dir: temp.clone(),
            outbox_dir: dir.path().join("outbox"),
        };

        fs::write(inbox.join("dup.bin"), b"old").unwrap();
        let bytes = b"new-payload";
        let temp_path = temp.join("dup.part");
        fs::write(&temp_path, bytes).unwrap();
        let meta = FileMetadata {
            filename: "dup.bin".into(),
            size: bytes.len() as u64,
            hash: "sha256:server".into(),
            transfer_type: TransferType::File,
            source: "test".into(),
            category: None,
            relative_path: None,
        };

        let registry = TransferRegistry::new();
        let id = Uuid::new_v4();
        let path = finalize_received_file(
            &config,
            &inbox,
            id,
            &meta,
            &temp_path,
            sha256_hex(bytes),
            &registry,
        )
        .unwrap();

        assert_eq!(path, inbox.join("dup_1.bin"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(registry.get(&id).unwrap().error, None);
    }
}
