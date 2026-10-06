use crate::{
    ArtifactLease, CoreError, ErrorCode, ExportArtifact, Library, Result,
    artifacts::MIN_RETENTION_MS,
    tasks::scheduler::Lane,
    tasks::{Priority, Task},
};
use memedock_domain::{
    export::{AnimationPolicy, ExportOptions, ExportPreset},
    identity::{OperationId, StickerId},
};
use memedock_storage::artifacts::ArtifactRecord;
use std::sync::Arc;

impl Library {
    pub fn export_original(&self, id: StickerId) -> Result<Task<Arc<ArtifactLease>>> {
        self.export(
            id,
            ExportOptions::for_preset(ExportPreset::Original, AnimationPolicy::Preserve),
        )
    }
    pub fn export(
        &self,
        id: StickerId,
        options: ExportOptions,
    ) -> Result<Task<Arc<ArtifactLease>>> {
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
                options.validate_source(asset.animated())?;
                if let Some(mut record) = services
                    .db
                    .artifact_for_recipe(asset.hash(), &options.recipe())
                    .await?
                {
                    let store = services.artifacts.store.clone();
                    let candidate = record.clone();
                    let check = control.clone();
                    let verified = tokio::task::spawn_blocking(move || {
                        store.verify(&candidate, || check.is_cancelled())
                    })
                    .await?;
                    match verified {
                        Ok(())
                            if !record.deleting
                                && record.format == options.output_format(asset.format())
                                && record.animated
                                    == (options.preset() == ExportPreset::Original
                                        && asset.animated()) =>
                        {
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
                let mut record = ArtifactRecord {
                    id: OperationId::new(),
                    source_hash: asset.hash(),
                    output_hash: asset.hash(),
                    recipe: options.recipe(),
                    format: options.output_format(asset.format()),
                    byte_size: u64::try_from(asset.byte_size().get())
                        .map_err(|_| CoreError::internal("asset size overflow"))?,
                    animated: options.preset() == ExportPreset::Original && asset.animated(),
                    retained_until: until,
                    deleting: false,
                };
                let store = services.artifacts.store.clone();
                let blobs = services.blobs.clone();
                super::artifact_maintenance::cleanup(&services, crate::writes::now()?.get())
                    .await?;
                let usage_store = services.artifacts.store.clone();
                let used = tokio::task::spawn_blocking(move || usage_store.bytes_used()).await??;
                let available = services
                    .config
                    .limits
                    .export_budget_bytes
                    .saturating_sub(used);
                if available == 0
                    || (options.preset() == ExportPreset::Original && record.byte_size > available)
                {
                    return Err(CoreError::new(
                        ErrorCode::ResourceLimit,
                        "sharing output quota exceeded",
                    ));
                }
                if options.preset() == ExportPreset::Original {
                    let candidate = record.clone();
                    let check = control.clone();
                    control.stage("copying_original");
                    tokio::task::spawn_blocking(move || -> Result<()> {
                        let original = blobs.open_original(candidate.source_hash)?;
                        store.publish(&candidate, original, || check.is_cancelled())?;
                        Ok(())
                    })
                    .await??;
                } else {
                    control.stage("waiting_export_budget");
                    let allowance = services.image_budget.acquire().await?;
                    let candidate = record.clone();
                    let limits = services.config.limits.clone();
                    let check = control.clone();
                    record = tokio::task::spawn_blocking(move || -> Result<ArtifactRecord> {
                        let _allowance = allowance;
                        blobs.verify(candidate.source_hash, candidate.byte_size, || {
                            check.is_cancelled()
                        })?;
                        let original = blobs.open_original(candidate.source_hash)?;
                        let staged = crate::images::export::encode(
                            original, &blobs, options, &limits, available, &check,
                        )?;
                        let mut output = candidate;
                        output.output_hash = staged.hash();
                        output.byte_size = staged.byte_size();
                        check.stage("publishing_export");
                        let published =
                            staged
                                .open_read()
                                .map_err(CoreError::from)
                                .and_then(|file| {
                                    let verified = crate::images::inspect::decode_export(
                                        file.try_clone()?,
                                        &limits,
                                        &check,
                                    )?;
                                    if verified.format != output.format || verified.animated {
                                        return Err(CoreError::new(
                                            ErrorCode::CorruptData,
                                            "encoded output metadata mismatch",
                                        ));
                                    }
                                    drop(verified);
                                    use std::io::{Seek, SeekFrom};
                                    let mut file = file;
                                    file.seek(SeekFrom::Start(0))?;
                                    store
                                        .publish(&output, file, || check.is_cancelled())
                                        .map_err(CoreError::from)
                                });
                        staged.discard()?;
                        published?;
                        Ok(output)
                    })
                    .await??;
                }
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
                    .artifact_by_id(lease.record.id)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancellation_while_waiting_for_image_budget_publishes_nothing()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let settings = crate::LibraryConfig::new(
            directory.path().join("data"),
            directory.path().join("cache"),
            directory.path().join("share"),
        );
        let library = Library::open(settings.clone()).await?;
        let input = library.create_import_input()?.wait().await?;
        image::DynamicImage::new_rgba8(2, 2)
            .save_with_format(input.path(), image::ImageFormat::Png)?;
        let id = library
            .import_staged(input, crate::ImportOptions::default())?
            .wait()
            .await?
            .sticker
            .id();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = tokio::sync::oneshot::channel();
        let blocker = library.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, _| async move {
                let _budget = services.image_budget.acquire().await?;
                let _ = started.send(());
                wait.await
                    .map_err(|_| CoreError::internal("release dropped"))?;
                Ok(())
            },
        )?;
        tokio::time::timeout(std::time::Duration::from_secs(5), ready).await??;
        let task = library.export(
            id,
            ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::Preserve),
        )?;
        let mut progress = task.progress();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while progress.snapshot().stage != "waiting_export_budget" {
                progress.next().await;
            }
        })
        .await?;
        assert_eq!(task.cancel(), crate::tasks::CancelResult::Requested);
        release.send(()).map_err(|_| "release dropped")?;
        blocker.wait().await?;
        assert_eq!(
            task.wait()
                .await
                .err()
                .ok_or("expected cancellation")?
                .code(),
            ErrorCode::Cancelled
        );
        assert_eq!(std::fs::read_dir(settings.export_dir)?.count(), 0);
        library.close().await?;
        Ok(())
    }
}
