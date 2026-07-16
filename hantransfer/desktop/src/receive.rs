use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use uuid::Uuid;

use crate::importer::{FileMetadata, TransferType};
use crate::settings::SettingsStore;
use crate::transfer::{
    cleanup_temp, finalize_received_file, TransferProgress, TransferRegistry, TransferStatus,
};
use crate::trust::TrustGate;
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct PendingReceive {
    pub transfer_id: Uuid,
    pub device_id: Uuid,
    pub meta: FileMetadata,
    pub temp_path: PathBuf,
    pub computed_hash: String,
    pub received_at: String,
}

#[derive(Clone, Default)]
pub struct ReceiveQueue {
    inner: Arc<Mutex<HashMap<Uuid, PendingReceive>>>,
}

impl ReceiveQueue {
    pub fn insert(&self, item: PendingReceive) {
        self.inner
            .lock()
            .expect("receive queue lock")
            .insert(item.transfer_id, item);
    }

    pub fn remove(&self, transfer_id: &Uuid) -> Option<PendingReceive> {
        self.inner.lock().expect("receive queue lock").remove(transfer_id)
    }

    pub fn list(&self) -> Vec<PendingReceive> {
        let mut items: Vec<_> = self.inner.lock().expect("receive queue lock").values().cloned().collect();
        items.sort_by(|a, b| a.received_at.cmp(&b.received_at));
        items
    }

    pub fn accept(
        &self,
        config: &Config,
        settings: &SettingsStore,
        registry: &TransferRegistry,
        trust: &TrustGate,
        transfer_id: &Uuid,
    ) -> Result<PathBuf, String> {
        let item = self
            .remove(transfer_id)
            .ok_or_else(|| format!("transfer {transfer_id} not pending"))?;
        let inbox = settings.inbox_dir();
        finalize_received_file(
            config,
            &inbox,
            item.transfer_id,
            &item.meta,
            &item.temp_path,
            item.computed_hash.clone(),
            registry,
        )
        .map_err(|e| {
            self.insert(item);
            e
        })
        .map(|path| {
            let _ = trust;
            path
        })
    }

    pub fn reject(&self, registry: &TransferRegistry, transfer_id: &Uuid) -> Result<(), String> {
        let item = self
            .remove(transfer_id)
            .ok_or_else(|| format!("transfer {transfer_id} not pending"))?;
        cleanup_temp(&item.temp_path);
        registry.set(TransferProgress {
            transfer_id: *transfer_id,
            status: TransferStatus::Rejected,
            bytes_received: item.meta.size,
            total: item.meta.size,
            filename: Some(item.meta.filename),
            device_name: None,
            path: None,
            error: Some("rejected by user".into()),
        });
        Ok(())
    }

    pub fn accept_all(
        &self,
        config: &Config,
        settings: &SettingsStore,
        registry: &TransferRegistry,
        trust: &TrustGate,
    ) -> Vec<Result<PathBuf, String>> {
        let ids: Vec<Uuid> = self.list().into_iter().map(|p| p.transfer_id).collect();
        ids.into_iter()
            .map(|id| self.accept(config, settings, registry, trust, &id))
            .collect()
    }
}

#[derive(Serialize)]
pub struct PendingReceiveView {
    pub transfer_id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub filename: String,
    pub size: u64,
    pub transfer_type: String,
    pub received_at: String,
}

pub fn pending_view(item: &PendingReceive, trust: &TrustGate) -> PendingReceiveView {
    let device_name = trust
        .store()
        .list_trusted()
        .into_iter()
        .find(|d| d.device_id == item.device_id)
        .map(|d| d.name)
        .unwrap_or_else(|| item.device_id.to_string());
    PendingReceiveView {
        transfer_id: item.transfer_id,
        device_id: item.device_id,
        device_name,
        filename: item.meta.filename.clone(),
        size: item.meta.size,
        transfer_type: match item.meta.transfer_type {
            TransferType::File => "file".into(),
            TransferType::AzurlaneAsset => "azurlane_asset".into(),
        },
        received_at: item.received_at.clone(),
    }
}

pub fn queue_pending(
    queue: &ReceiveQueue,
    registry: &TransferRegistry,
    transfer_id: Uuid,
    device_id: Uuid,
    meta: &FileMetadata,
    temp_path: PathBuf,
    computed_hash: String,
) {
    registry.set(TransferProgress {
        transfer_id,
        status: TransferStatus::PendingApproval,
        bytes_received: meta.size,
        total: meta.size,
        filename: Some(meta.filename.clone()),
        device_name: None,
        path: None,
        error: None,
    });
    queue.insert(PendingReceive {
        transfer_id,
        device_id,
        meta: meta.clone(),
        temp_path,
        computed_hash,
        received_at: crate::trust::now_iso(),
    });
}
