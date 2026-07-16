use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushEntry {
    pub push_id: Uuid,
    pub target_device_id: Uuid,
    pub filename: String,
    pub size: u64,
    pub hash: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct PushStore {
    root: PathBuf,
}

impl PushStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn from_config(outbox_dir: &Path) -> Self {
        Self::new(outbox_dir.to_path_buf())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure_root(&self) -> Result<(), String> {
        paths::ensure_dir(&self.root).map_err(|e| e.to_string())
    }

    pub fn create(
        &self,
        target_device_id: Uuid,
        filename: &str,
        source: &str,
        bytes: &[u8],
    ) -> Result<PushEntry, String> {
        self.ensure_root()?;
        let push_id = Uuid::new_v4();
        let dir = self.entry_dir(push_id);
        paths::ensure_dir(&dir).map_err(|e| e.to_string())?;
        let hash = format!("sha256:{:x}", Sha256::digest(bytes));
        let file_path = dir.join("file.bin");
        std::fs::write(&file_path, bytes).map_err(|e| e.to_string())?;
        let entry = PushEntry {
            push_id,
            target_device_id,
            filename: sanitize_filename(filename),
            size: bytes.len() as u64,
            hash,
            source: source.to_string(),
            created_at: crate::trust::now_iso(),
        };
        write_meta(&dir, &entry)?;
        Ok(entry)
    }

    pub fn create_from_temp(
        &self,
        target_device_id: Uuid,
        filename: &str,
        source: &str,
        temp_path: &Path,
        hash: String,
    ) -> Result<PushEntry, String> {
        self.ensure_root()?;
        let size = std::fs::metadata(temp_path)
            .map_err(|e| e.to_string())?
            .len();
        let push_id = Uuid::new_v4();
        let dir = self.entry_dir(push_id);
        paths::ensure_dir(&dir).map_err(|e| e.to_string())?;
        let dest = dir.join("file.bin");
        std::fs::rename(temp_path, &dest).or_else(|_| {
            std::fs::copy(temp_path, &dest).map_err(|e| e.to_string())?;
            std::fs::remove_file(temp_path).map_err(|e| e.to_string())
        })?;
        let entry = PushEntry {
            push_id,
            target_device_id,
            filename: sanitize_filename(filename),
            size,
            hash,
            source: source.to_string(),
            created_at: crate::trust::now_iso(),
        };
        write_meta(&dir, &entry)?;
        Ok(entry)
    }

    pub fn list_pending_for(&self, device_id: &Uuid) -> Vec<PushEntry> {
        self.list_all()
            .into_iter()
            .filter(|e| &e.target_device_id == device_id)
            .collect()
    }

    pub fn list_all(&self) -> Vec<PushEntry> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Ok(meta) = read_meta(&path) {
                out.push(meta);
            }
        }
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        out
    }

    pub fn cancel(&self, push_id: &Uuid) -> Result<PushEntry, String> {
        let entry = self
            .get(push_id)
            .ok_or_else(|| format!("push {push_id} not found"))?;
        let dir = self.entry_dir(*push_id);
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(entry)
    }

    pub fn get(&self, push_id: &Uuid) -> Option<PushEntry> {
        read_meta(&self.entry_dir(*push_id)).ok()
    }

    pub fn file_path(&self, push_id: &Uuid) -> PathBuf {
        self.entry_dir(*push_id).join("file.bin")
    }

    pub fn acknowledge(&self, push_id: &Uuid, device_id: &Uuid) -> Result<PushEntry, String> {
        let entry = self
            .get(push_id)
            .ok_or_else(|| format!("push {push_id} not found"))?;
        if entry.target_device_id != *device_id {
            return Err("push not for this device".into());
        }
        let dir = self.entry_dir(*push_id);
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(entry)
    }

    fn entry_dir(&self, push_id: Uuid) -> PathBuf {
        self.root.join(push_id.to_string())
    }
}

fn write_meta(dir: &Path, entry: &PushEntry) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(entry).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meta.json"), raw).map_err(|e| e.to_string())
}

fn read_meta(dir: &Path) -> Result<PushEntry, String> {
    let raw = std::fs::read_to_string(dir.join("meta.json")).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn sanitize_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file.bin");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "file.bin".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn create_list_and_ack() {
        let dir = tempdir().unwrap();
        let store = PushStore::new(dir.path().to_path_buf());
        let target = Uuid::new_v4();
        let entry = store
            .create(target, "photo.jpg", "HAN-PC", b"hello")
            .unwrap();
        assert_eq!(entry.filename, "photo.jpg");
        assert_eq!(store.list_pending_for(&target).len(), 1);
        assert!(store.file_path(&entry.push_id).is_file());
        let removed = store.acknowledge(&entry.push_id, &target).unwrap();
        assert_eq!(removed.push_id, entry.push_id);
        assert!(store.list_pending_for(&target).is_empty());
    }

    #[test]
    fn ack_rejects_wrong_device() {
        let dir = tempdir().unwrap();
        let store = PushStore::new(dir.path().to_path_buf());
        let target = Uuid::new_v4();
        let entry = store.create(target, "a.bin", "pc", b"x").unwrap();
        let other = Uuid::new_v4();
        assert!(store.acknowledge(&entry.push_id, &other).is_err());
    }

    #[test]
    fn cancel_removes_entry() {
        let dir = tempdir().unwrap();
        let store = PushStore::new(dir.path().to_path_buf());
        let target = Uuid::new_v4();
        let entry = store.create(target, "b.bin", "pc", b"data").unwrap();
        assert_eq!(store.list_all().len(), 1);
        store.cancel(&entry.push_id).unwrap();
        assert!(store.list_all().is_empty());
    }
}
