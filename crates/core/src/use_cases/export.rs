use crate::{
    ArtifactLease, CoreError, ErrorCode, ExportArtifact, Library, Result,
    artifacts::MIN_RETENTION_MS,
    tasks::scheduler::Lane,
    tasks::{Priority, Task},
};
use memedock_domain::identity::{OperationId, StickerId};
use memedock_storage::artifacts::ArtifactRecord;
use std::sync::Arc;

impl Library {
    pub fn export_original(&self, id: StickerId) -> Result<Task<Arc<ArtifactLease>>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                let _permit = services
                    .artifacts
                    .permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("artifact manager closed"))?;
                control.check()?;
                let sticker = services
                    .db
                    .sticker(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker not found"))?;
                if !sticker.lifecycle().is_active() {
                    return Err(CoreError::new(ErrorCode::EntityDeleted, "sticker deleted"));
                }
                let asset = services
                    .db
                    .asset(id.content_hash())
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::CorruptData, "asset missing"))?;
                let until = crate::writes::now()?
                    .get()
                    .checked_add(MIN_RETENTION_MS)
                    .ok_or_else(|| CoreError::internal("retention overflow"))?;
                if let Some(mut record) = services.db.artifact_for(asset.hash()).await? {
                    let store = services.artifacts.store.clone();
                    let candidate = record.clone();
                    let check = control.clone();
                    let verified = tokio::task::spawn_blocking(move || {
                        store.verify(&candidate, || check.is_cancelled())
                    })
                    .await?;
                    match verified {
                        Ok(()) if !record.deleting => {
                            control.begin_commit()?;
                            record.retained_until = record.retained_until.max(until);
                            services.db.save_artifact(&record).await?;
                            return services.artifacts.lease(record);
                        }
                        Err(memedock_storage::StorageError::Cancelled) => {
                            return Err(CoreError::new(ErrorCode::Cancelled, "export cancelled"));
                        }
                        Err(memedock_storage::StorageError::Io(e))
                            if e.kind() != std::io::ErrorKind::NotFound =>
                        {
                            return Err(e.into());
                        }
                        _ => {}
                    }
                    if services.artifacts.active(record.id)? {
                        return Err(CoreError::new(
                            ErrorCode::CorruptData,
                            "active artifact unavailable",
                        ));
                    }
                    // Never change bytes behind an already issued URI.
                    record.deleting = true;
                    services.db.save_artifact(&record).await?;
                }
                let record = ArtifactRecord {
                    id: OperationId::new(),
                    source_hash: asset.hash(),
                    format: asset.format(),
                    byte_size: u64::try_from(asset.byte_size().get())
                        .map_err(|_| CoreError::internal("asset size overflow"))?,
                    animated: asset.animated(),
                    retained_until: until,
                    deleting: false,
                };
                let store = services.artifacts.store.clone();
                let blobs = services.blobs.clone();
                super::artifact_maintenance::cleanup(&services, crate::writes::now()?.get())
                    .await?;
                let usage_store = services.artifacts.store.clone();
                let used = tokio::task::spawn_blocking(move || usage_store.bytes_used()).await??;
                if used
                    .checked_add(record.byte_size)
                    .is_none_or(|bytes| bytes > services.config.limits.export_budget_bytes)
                {
                    return Err(CoreError::new(
                        ErrorCode::ResourceLimit,
                        "sharing output quota exceeded",
                    ));
                }
                let candidate = record.clone();
                let check = control.clone();
                control.stage("copying_original");
                tokio::task::spawn_blocking(move || -> Result<()> {
                    let original = blobs.open_original(candidate.source_hash)?;
                    store.publish(&candidate, original, || check.is_cancelled())?;
                    Ok(())
                })
                .await??;
                control.begin_commit()?;
                services.db.save_artifact(&record).await?;
                services.artifacts.lease(record)
            },
        )
    }
    /// Call immediately before platform handoff. Extends durable retention.
    pub fn prepare_handoff(&self, lease: Arc<ArtifactLease>) -> Result<Task<ExportArtifact>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                if !Arc::ptr_eq(&lease.owner, &services.artifacts) {
                    return Err(CoreError::new(
                        ErrorCode::InvalidInput,
                        "artifact belongs to another library session",
                    ));
                }
                let _permit = services
                    .artifacts
                    .permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("artifact manager closed"))?;
                control.check()?;
                let mut record = services
                    .db
                    .artifact_for(lease.record.source_hash)
                    .await?
                    .filter(|r| r.id == lease.record.id && !r.deleting)
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "artifact unavailable"))?;
                let store = services.artifacts.store.clone();
                let candidate = record.clone();
                let check = control.clone();
                tokio::task::spawn_blocking(move || {
                    store.verify(&candidate, || check.is_cancelled())
                })
                .await??;
                control.begin_commit()?;
                record.retained_until = record.retained_until.max(
                    crate::writes::now()?
                        .get()
                        .checked_add(MIN_RETENTION_MS)
                        .ok_or_else(|| CoreError::internal("retention overflow"))?,
                );
                services.db.save_artifact(&record).await?;
                Ok(ExportArtifact::from_record(
                    &record,
                    services.artifacts.store.path(&record),
                ))
            },
        )
    }
}
