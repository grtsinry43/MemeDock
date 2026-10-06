use crate::{CoreError, Result};
use memedock_domain::identity::OperationId;
use memedock_storage::{artifacts::ArtifactRecord, files::ExportStore};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub(crate) const MIN_RETENTION_MS: i64 = 72 * 60 * 60 * 1000;
pub(crate) struct ArtifactManager {
    pub(crate) store: ExportStore,
    pub(crate) permit: tokio::sync::Semaphore,
    live: Mutex<HashMap<OperationId, usize>>,
}
impl ArtifactManager {
    pub(crate) fn has_active_leases(&self) -> Result<bool> {
        Ok(!self
            .live
            .lock()
            .map_err(|_| CoreError::internal("artifact lease lock poisoned"))?
            .is_empty())
    }
    pub(crate) fn new(store: ExportStore) -> Arc<Self> {
        Arc::new(Self {
            store,
            permit: tokio::sync::Semaphore::new(1),
            live: Mutex::new(HashMap::new()),
        })
    }
    pub(crate) fn active(&self, id: OperationId) -> Result<bool> {
        Ok(self
            .live
            .lock()
            .map_err(|_| CoreError::internal("artifact lease lock poisoned"))?
            .contains_key(&id))
    }
    pub(crate) fn lease(self: &Arc<Self>, record: ArtifactRecord) -> Result<Arc<ArtifactLease>> {
        let mut live = self
            .live
            .lock()
            .map_err(|_| CoreError::internal("artifact lease lock poisoned"))?;
        let count = live.entry(record.id).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| CoreError::internal("artifact lease overflow"))?;
        Ok(Arc::new(ArtifactLease {
            metadata: ExportArtifact::from_record(&record, self.store.path(&record)),
            record,
            owner: self.clone(),
        }))
    }
}
#[derive(Clone, Debug)]
pub struct ExportArtifact {
    pub id: OperationId,
    pub path: PathBuf,
    pub mime: String,
    pub file_name: String,
    pub byte_size: u64,
    pub animated: bool,
    pub retained_until: i64,
}
impl ExportArtifact {
    pub(crate) fn from_record(r: &ArtifactRecord, path: PathBuf) -> Self {
        Self {
            id: r.id,
            path,
            mime: r.format.mime().into(),
            file_name: format!("MemeDock-{}.{}", r.source_hash, r.format.extension()),
            byte_size: r.byte_size,
            animated: r.animated,
            retained_until: r.retained_until,
        }
    }
}
pub struct ArtifactLease {
    pub(crate) record: ArtifactRecord,
    pub(crate) owner: Arc<ArtifactManager>,
    metadata: ExportArtifact,
}
impl ArtifactLease {
    pub fn metadata(&self) -> &ExportArtifact {
        &self.metadata
    }
}
impl Drop for ArtifactLease {
    fn drop(&mut self) {
        // No IO or async work in Drop; durable retention outlives this handle.
        if let Ok(mut live) = self.owner.live.lock()
            && let Some(count) = live.get_mut(&self.record.id)
        {
            *count = count.saturating_sub(1);
            if *count == 0 {
                live.remove(&self.record.id);
            }
        }
    }
}
