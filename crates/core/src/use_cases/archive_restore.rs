use crate::{
    ArchiveInput, CoreError, ErrorCode, Library, PreparedArchive, RestoreMode, RestoreSummary,
    Result, archive,
    events::ChangeKind,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::{
    change::{Operation, OperationKind},
    identity::OperationId,
};
use memedock_storage::{LibraryDatabase, files::library_layout::LibraryLayout};
use std::{path::PathBuf, sync::Arc};

pub struct RestoreResult {
    pub summary: RestoreSummary,
    pub checkpoint: Option<PathBuf>,
    pub(crate) replacement: Option<Replacement>,
}
pub(crate) struct Replacement {
    pub id: OperationId,
    pub config: crate::LibraryConfig,
    pub revision: i64,
    pub identity: memedock_storage::db::LibraryIdentity,
    pub artifacts: Arc<crate::artifacts::ArtifactManager>,
}

impl Library {
    pub fn inspect_archive(&self, input: ArchiveInput) -> Result<Task<Arc<PreparedArchive>>> {
        if input.root != self.data_dir() {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "foreign archive input",
            ));
        }
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                let root = input.root;
                let path = input.path;
                let worker = services.clone();
                let check = control.clone();
                let inspect_path = path.clone();
                let inspected = tokio::task::spawn_blocking(move || {
                    archive::validation::inspect(&inspect_path, &worker.config.limits, &check)
                })
                .await?;
                let (data, hash) = match inspected {
                    Ok(value) => value,
                    Err(error) => {
                        let cleanup_root = root.clone();
                        let cleanup_path = path.clone();
                        tokio::task::spawn_blocking(move || {
                            memedock_storage::files::archive::remove_archive_file(
                                &cleanup_root,
                                &cleanup_path,
                            )
                        })
                        .await??;
                        return Err(error);
                    }
                };
                let _permit = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                let current = services
                    .db
                    .archive_data_bounded(services.config.limits.max_archive_metadata_bytes)
                    .await?;
                let (_, summary) = archive::merge::merge(&current, &data)?;
                let target_revision = services.db.archive_target_revision().await?;
                Ok(Arc::new(PreparedArchive {
                    root,
                    path,
                    hash,
                    data,
                    target_revision,
                    summary,
                }))
            },
        )
    }
    pub fn restore_archive(
        &self,
        prepared: Arc<PreparedArchive>,
        mode: RestoreMode,
    ) -> Result<Task<RestoreResult>> {
        if prepared.root != self.data_dir() {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "foreign prepared archive",
            ));
        }
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                if mode == RestoreMode::Replace && services.artifacts.has_active_leases()? {
                    return Err(CoreError::new(
                        ErrorCode::Busy,
                        "active file delivery prevents replacement",
                    ));
                }
                let _budget = services.image_budget.acquire().await?;
                let worker = services.clone();
                let check = control.clone();
                let proof = prepared.clone();
                tokio::task::spawn_blocking(move || -> Result<()> {
                    let (data, hash) =
                        archive::validation::inspect(&proof.path, &worker.config.limits, &check)?;
                    if hash != proof.hash || data != proof.data {
                        return Err(archive::corrupt("archive changed after preview"));
                    }
                    let mut zip = zip::ZipArchive::new(std::fs::File::open(&proof.path)?)
                        .map_err(archive::zip)?;
                    for asset in &data.assets {
                        check.check()?;
                        let mut entry = zip
                            .by_name(&format!("originals/{}", asset.hash()))
                            .map_err(archive::zip)?;
                        let staged = worker.blobs.stage_from(
                            &mut entry,
                            asset.byte_size().get() as u64,
                            || check.is_cancelled(),
                        )?;
                        if staged.hash() != asset.hash() {
                            staged.discard()?;
                            return Err(archive::corrupt("archive original changed"));
                        }
                        let decoded = crate::images::inspect::decode(
                            staged.open_read()?,
                            &worker.config.limits,
                            &check,
                            true,
                        )?;
                        if decoded.format != asset.format()
                            || decoded.dimensions != (asset.width(), asset.height())
                            || decoded.animated != asset.animated()
                        {
                            staged.discard()?;
                            return Err(archive::corrupt("archive image metadata mismatch"));
                        }
                        drop(decoded);
                        worker.blobs.publish(staged, || check.is_cancelled())?;
                    }
                    Ok(())
                })
                .await??;
                let _write = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                let revision = services.db.archive_target_revision().await?;
                if revision != prepared.target_revision {
                    return Err(CoreError::new(
                        ErrorCode::Conflict,
                        "library changed after preview",
                    ));
                }
                let current = services
                    .db
                    .archive_data_bounded(services.config.limits.max_archive_metadata_bytes)
                    .await?;
                let (state, summary) = match mode {
                    RestoreMode::Merge => archive::merge::merge(&current, &prepared.data)?,
                    RestoreMode::Replace => (prepared.data.clone(), prepared.data.summary()?),
                };
                let id = OperationId::new();
                let candidate = if mode == RestoreMode::Replace {
                    let root = services.config.data_dir.clone();
                    Some(
                        tokio::task::spawn_blocking(move || {
                            LibraryLayout::open(&root)?.candidate(id)
                        })
                        .await??,
                    )
                } else {
                    None
                };
                let replacement_db = match &candidate {
                    Some(path) => Some(LibraryDatabase::open(path).await?),
                    None => None,
                };
                let db = replacement_db.as_ref().unwrap_or(&services.db);
                control.check()?;
                let mut tx = db.begin_write().await?;
                tx.import_archive(&state).await?;
                let at = crate::writes::now()?;
                for asset in &state.assets {
                    let mut local = tx
                        .local_asset(asset.hash())
                        .await?
                        .unwrap_or_else(|| memedock_domain::local::LocalAsset::new(asset.hash()));
                    local.verified(at);
                    tx.save_local_asset(&local).await?;
                }
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::ImportArchive {
                        archive_hash: prepared.hash,
                        state: Box::new(state),
                    })?,
                    crate::writes::now()?,
                )
                .await?;
                control.begin_commit()?;
                tx.commit().await?;
                if let Some(db) = replacement_db {
                    db.close().await?;
                }
                if mode == RestoreMode::Merge {
                    services.events.publish(ChangeKind::CollectionsChanged)?;
                    services.events.publish(ChangeKind::TagsChanged)?;
                }
                Ok(RestoreResult {
                    summary,
                    checkpoint: None,
                    replacement: candidate.map(|_| Replacement {
                        id,
                        config: services.config.clone(),
                        revision,
                        identity: services.db.identity(),
                        artifacts: services.artifacts.clone(),
                    }),
                })
            },
        )
    }
    /// Called by the platform coordinator after awaiting the preparation task,
    /// outside this library's runtime. Shutdown drains work before touching files.
    pub async fn complete_restore(&self, mut result: RestoreResult) -> Result<RestoreResult> {
        let Some(replacement) = result.replacement.take() else {
            return Ok(result);
        };
        if replacement.artifacts.has_active_leases()? {
            return Err(CoreError::new(
                ErrorCode::Busy,
                "active file delivery prevents replacement",
            ));
        }
        self.close().await?;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("memedock-replacement".into())
            .spawn(move || {
                let outcome = (|| -> Result<RestoreResult> {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .max_blocking_threads(replacement.config.limits.blocking_threads)
                        .enable_all()
                        .build()?;
                    let _reservation = crate::registry::Reservation::acquire(&replacement.config)?;
                    runtime.block_on(finish_replacement(replacement, result))
                })();
                let _ = sender.send(outcome);
            })?;
        receiver
            .await
            .map_err(|_| CoreError::internal("replacement worker disconnected"))?
    }
}

async fn finish_replacement(
    replacement: Replacement,
    mut result: RestoreResult,
) -> Result<RestoreResult> {
    if replacement.artifacts.has_active_leases()? {
        return Err(CoreError::new(
            ErrorCode::Busy,
            "active file delivery prevents replacement",
        ));
    }
    let root = replacement.config.data_dir.clone();
    let id = replacement.id;
    let source = LibraryDatabase::open(root.join("library.sqlite")).await?;
    if source.identity() != replacement.identity
        || source.archive_target_revision().await? != replacement.revision
    {
        source.close().await?;
        return Err(CoreError::new(
            ErrorCode::Conflict,
            "library changed before replacement",
        ));
    }
    let candidate_path =
        tokio::task::spawn_blocking(move || LibraryLayout::open(&root)?.candidate(id)).await??;
    let candidate = LibraryDatabase::open(&candidate_path).await?;
    let copy = candidate.copy_delivery_state_from(&source).await;
    candidate.close().await?;
    source.close().await?;
    copy?;
    let root = replacement.config.data_dir;
    result.checkpoint =
        Some(tokio::task::spawn_blocking(move || LibraryLayout::open(&root)?.switch(id)).await??);
    Ok(result)
}
