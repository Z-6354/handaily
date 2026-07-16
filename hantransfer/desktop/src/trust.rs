use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDevice {
    pub device_id: Uuid,
    pub name: String,
    pub platform: String,
    pub ip: Option<String>,
    pub trusted_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TrustFile {
    devices: Vec<TrustedDevice>,
    #[serde(default)]
    rejected_device_ids: Vec<Uuid>,
}

#[derive(Clone)]
pub struct TrustStore {
    path: PathBuf,
    inner: Arc<Mutex<TrustFile>>,
}

impl TrustStore {
    pub fn load_or_create() -> Result<Self, String> {
        let path = config::app_config_dir().join("trust.json");
        let inner = if path.is_file() {
            let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            TrustFile::default()
        };
        Ok(Self {
            path,
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    pub fn is_trusted(&self, device_id: &Uuid) -> bool {
        self.inner
            .lock()
            .expect("trust lock")
            .devices
            .iter()
            .any(|d| &d.device_id == device_id)
    }

    pub fn is_rejected(&self, device_id: &Uuid) -> bool {
        self.inner
            .lock()
            .expect("trust lock")
            .rejected_device_ids
            .contains(device_id)
    }

    pub fn list_trusted(&self) -> Vec<TrustedDevice> {
        self.inner.lock().expect("trust lock").devices.clone()
    }

    pub fn rejected_ids(&self) -> HashSet<Uuid> {
        self.inner
            .lock()
            .expect("trust lock")
            .rejected_device_ids
            .iter()
            .copied()
            .collect()
    }

    pub fn trust_device(&self, device: TrustedDevice) -> Result<(), String> {
        let mut guard = self.inner.lock().expect("trust lock");
        guard.rejected_device_ids.retain(|id| id != &device.device_id);
        if let Some(existing) = guard
            .devices
            .iter_mut()
            .find(|d| d.device_id == device.device_id)
        {
            if device.ip.is_some() {
                existing.ip = device.ip.clone();
            }
            if !device.name.is_empty() {
                existing.name = device.name.clone();
            }
        } else {
            guard.devices.push(device);
        }
        Self::save(&self.path, &guard)
    }

    pub fn record_rejection(&self, device_id: Uuid) -> Result<(), String> {
        let mut guard = self.inner.lock().expect("trust lock");
        if !guard.rejected_device_ids.contains(&device_id) {
            guard.rejected_device_ids.push(device_id);
        }
        Self::save(&self.path, &guard)
    }

    pub fn revoke_device(&self, device_id: &Uuid) -> Result<bool, String> {
        let mut guard = self.inner.lock().expect("trust lock");
        let before = guard.devices.len();
        guard.devices.retain(|d| &d.device_id != device_id);
        if guard.devices.len() == before {
            return Ok(false);
        }
        Self::save(&self.path, &guard)?;
        Ok(true)
    }

    /// Remove a device from the rejected list so it can handshake again.
    pub fn clear_rejection(&self, device_id: &Uuid) -> Result<bool, String> {
        let mut guard = self.inner.lock().expect("trust lock");
        let before = guard.rejected_device_ids.len();
        guard.rejected_device_ids.retain(|id| id != device_id);
        if guard.rejected_device_ids.len() == before {
            return Ok(false);
        }
        Self::save(&self.path, &guard)?;
        Ok(true)
    }

    pub fn list_rejected(&self) -> Vec<Uuid> {
        self.inner
            .lock()
            .expect("trust lock")
            .rejected_device_ids
            .clone()
    }

    pub fn trusted_count(&self) -> usize {
        self.inner.lock().expect("trust lock").devices.len()
    }

    fn save(path: &PathBuf, data: &TrustFile) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let raw = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
        std::fs::write(path, raw).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub device_id: Uuid,
    pub name: String,
    pub platform: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandshakeStatus {
    Trusted,
    Pending,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct PendingHandshake {
    pub request: HandshakeRequest,
    pub client_ip: String,
}

#[derive(Clone)]
pub struct TrustGate {
    store: TrustStore,
    pending: Arc<Mutex<HashMap<Uuid, PendingHandshake>>>,
    rejected: Arc<Mutex<HashSet<Uuid>>>,
    notify: Arc<(Mutex<()>, Condvar)>,
    auto_trust: bool,
}

impl TrustGate {
    pub fn new(store: TrustStore, auto_trust: bool) -> Self {
        let rejected_ids = store.rejected_ids();
        Self {
            store,
            pending: Arc::new(Mutex::new(HashMap::new())),
            rejected: Arc::new(Mutex::new(rejected_ids)),
            notify: Arc::new((Mutex::new(()), Condvar::new())),
            auto_trust,
        }
    }

    pub fn store(&self) -> &TrustStore {
        &self.store
    }

    pub fn evaluate_handshake(
        &self,
        req: &HandshakeRequest,
        client_ip: &str,
    ) -> HandshakeStatus {
        if self.store.is_trusted(&req.device_id) {
            let _ = self.store.trust_device(TrustedDevice {
                device_id: req.device_id,
                name: req.name.clone(),
                platform: req.platform.clone(),
                ip: Some(client_ip.to_string()),
                trusted_at: now_iso(),
            });
            return HandshakeStatus::Trusted;
        }
        if self.rejected.lock().expect("rejected lock").contains(&req.device_id) {
            return HandshakeStatus::Rejected;
        }

        if self.auto_trust {
            let _ = self.store.trust_device(TrustedDevice {
                device_id: req.device_id,
                name: req.name.clone(),
                platform: req.platform.clone(),
                ip: Some(client_ip.to_string()),
                trusted_at: now_iso(),
            });
            return HandshakeStatus::Trusted;
        }

        let mut pending = self.pending.lock().expect("pending lock");
        pending.insert(
            req.device_id,
            PendingHandshake {
                request: req.clone(),
                client_ip: client_ip.to_string(),
            },
        );
        drop(pending);
        let (_lock, cv) = &*self.notify;
        cv.notify_one();
        HandshakeStatus::Pending
    }

    pub fn wait_for_pending_signal(&self) {
        let (_lock, cv) = &*self.notify;
        if let Ok(guard) = _lock.lock() {
            let _ = cv.wait_timeout(guard, std::time::Duration::from_secs(2));
        }
    }

    pub fn wait_for_pending(&self) -> Option<PendingHandshake> {
        self.wait_for_pending_signal();
        self.take_one_pending()
    }

    pub fn take_one_pending(&self) -> Option<PendingHandshake> {
        let mut pending = self.pending.lock().expect("pending lock");
        let key = pending.keys().next().copied()?;
        pending.remove(&key)
    }

    pub fn approve(&self, pending: &PendingHandshake) -> Result<(), String> {
        self.pending
            .lock()
            .expect("pending lock")
            .remove(&pending.request.device_id);
        self.rejected
            .lock()
            .expect("rejected lock")
            .remove(&pending.request.device_id);
        self.store.trust_device(TrustedDevice {
            device_id: pending.request.device_id,
            name: pending.request.name.clone(),
            platform: pending.request.platform.clone(),
            ip: Some(pending.client_ip.clone()),
            trusted_at: now_iso(),
        })
    }

    pub fn reject(&self, pending: &PendingHandshake) {
        self.pending
            .lock()
            .expect("pending lock")
            .remove(&pending.request.device_id);
        self.rejected
            .lock()
            .expect("rejected lock")
            .insert(pending.request.device_id);
        if let Err(err) = self.store.record_rejection(pending.request.device_id) {
            tracing::error!("persist rejection failed: {err}");
        }
    }

    pub fn approve_by_id(&self, device_id: &Uuid) -> Result<(), String> {
        let pending = self
            .pending
            .lock()
            .expect("pending lock")
            .get(device_id)
            .cloned()
            .ok_or_else(|| format!("pending device {device_id} not found"))?;
        self.approve(&pending)
    }

    pub fn reject_by_id(&self, device_id: &Uuid) -> Result<(), String> {
        let pending = self
            .pending
            .lock()
            .expect("pending lock")
            .get(device_id)
            .cloned()
            .ok_or_else(|| format!("pending device {device_id} not found"))?;
        self.reject(&pending);
        Ok(())
    }

    pub fn revoke(&self, device_id: &Uuid) -> Result<bool, String> {
        self.store.revoke_device(device_id)
    }

    pub fn clear_rejection(&self, device_id: &Uuid) -> Result<bool, String> {
        self.rejected
            .lock()
            .expect("rejected lock")
            .remove(device_id);
        self.store.clear_rejection(device_id)
    }

    pub fn list_rejected(&self) -> Vec<Uuid> {
        let mem: HashSet<Uuid> = self
            .rejected
            .lock()
            .expect("rejected lock")
            .iter()
            .copied()
            .collect();
        let mut ids = self.store.list_rejected();
        for id in mem {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids
    }

    pub fn is_trusted(&self, device_id: &Uuid) -> bool {
        self.store.is_trusted(device_id)
    }

    pub fn list_pending(&self) -> Vec<PendingHandshake> {
        self.pending
            .lock()
            .expect("pending lock")
            .values()
            .cloned()
            .collect()
    }
}

pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_store(dir: &std::path::Path) -> TrustStore {
        TrustStore {
            path: dir.join("trust.json"),
            inner: Arc::new(Mutex::new(TrustFile::default())),
        }
    }

    #[test]
    fn auto_trust_grants_immediately() {
        let dir = tempdir().unwrap();
        let gate = TrustGate::new(test_store(dir.path()), true);
        let id = Uuid::new_v4();
        let status = gate.evaluate_handshake(
            &HandshakeRequest {
                device_id: id,
                name: "HAN PHONE".into(),
                platform: "android".into(),
                version: "0.1.0".into(),
            },
            "192.168.1.22",
        );
        assert_eq!(status, HandshakeStatus::Trusted);
        assert!(gate.is_trusted(&id));
    }

    #[test]
    fn manual_trust_stays_pending_until_approve() {
        let dir = tempdir().unwrap();
        let gate = TrustGate::new(test_store(dir.path()), false);
        let id = Uuid::new_v4();
        let req = HandshakeRequest {
            device_id: id,
            name: "HAN PHONE".into(),
            platform: "android".into(),
            version: "0.1.0".into(),
        };
        let status = gate.evaluate_handshake(&req, "192.168.1.22");
        assert_eq!(status, HandshakeStatus::Pending);
        assert!(!gate.is_trusted(&id));

        gate.approve_by_id(&id).unwrap();
        assert!(gate.is_trusted(&id));
    }

    #[test]
    fn reject_by_id_persists_and_clears_pending() {
        let dir = tempdir().unwrap();
        let gate = TrustGate::new(test_store(dir.path()), false);
        let id = Uuid::new_v4();
        gate.evaluate_handshake(
            &HandshakeRequest {
                device_id: id,
                name: "HAN PHONE".into(),
                platform: "android".into(),
                version: "0.1.0".into(),
            },
            "192.168.1.22",
        );
        assert_eq!(gate.list_pending().len(), 1);
        gate.reject_by_id(&id).unwrap();
        assert!(gate.list_pending().is_empty());
        assert!(gate.store().is_rejected(&id));
    }

    #[test]
    fn reject_persists_in_store() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path());
        let gate = TrustGate::new(store, false);
        let id = Uuid::new_v4();
        let pending = PendingHandshake {
            request: HandshakeRequest {
                device_id: id,
                name: "HAN PHONE".into(),
                platform: "android".into(),
                version: "0.1.0".into(),
            },
            client_ip: "192.168.1.22".into(),
        };
        gate.reject(&pending);
        let reloaded = TrustStore {
            path: dir.path().join("trust.json"),
            inner: Arc::new(Mutex::new(
                serde_json::from_str(&std::fs::read_to_string(dir.path().join("trust.json")).unwrap())
                    .unwrap(),
            )),
        };
        assert!(reloaded.is_rejected(&id));
    }

    #[test]
    fn revoke_removes_trust() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path());
        let id = Uuid::new_v4();
        store
            .trust_device(TrustedDevice {
                device_id: id,
                name: "phone".into(),
                platform: "android".into(),
                ip: None,
                trusted_at: now_iso(),
            })
            .unwrap();
        assert!(store.revoke_device(&id).unwrap());
        assert!(!store.is_trusted(&id));
    }

    #[test]
    fn clear_rejection_allows_handshake_again() {
        let dir = tempdir().unwrap();
        let gate = TrustGate::new(test_store(dir.path()), false);
        let id = Uuid::new_v4();
        let req = HandshakeRequest {
            device_id: id,
            name: "HAN PHONE".into(),
            platform: "android".into(),
            version: "0.1.0".into(),
        };
        gate.evaluate_handshake(&req, "192.168.1.22");
        gate.reject_by_id(&id).unwrap();
        assert_eq!(
            gate.evaluate_handshake(&req, "192.168.1.22"),
            HandshakeStatus::Rejected
        );
        assert!(gate.clear_rejection(&id).unwrap());
        assert_eq!(
            gate.evaluate_handshake(&req, "192.168.1.22"),
            HandshakeStatus::Pending
        );
    }

    #[test]
    fn rehandshake_updates_ip() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path());
        let gate = TrustGate::new(store, true);
        let id = Uuid::new_v4();
        gate.evaluate_handshake(
            &HandshakeRequest {
                device_id: id,
                name: "phone".into(),
                platform: "android".into(),
                version: "0.1.0".into(),
            },
            "192.168.1.10",
        );
        gate.evaluate_handshake(
            &HandshakeRequest {
                device_id: id,
                name: "phone".into(),
                platform: "android".into(),
                version: "0.1.0".into(),
            },
            "192.168.1.20",
        );
        let device = gate.store().list_trusted().into_iter().find(|d| d.device_id == id).unwrap();
        assert_eq!(device.ip.as_deref(), Some("192.168.1.20"));
    }
}
