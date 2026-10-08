use crate::{
    CoreError, ErrorCode, Library, Result,
    events::ChangeKind,
    images::inspect,
    runtime::Services,
    tasks::{Priority, Task, TaskControl, scheduler::Lane},
};
use memedock_domain::{
    asset::Asset,
    change::{Operation, OperationKind},
    identity::{CollectionId, LibraryId, OperationId},
    local::LocalAsset,
    ordering::SortKey,
    relation::CollectionItem,
    sticker::{ImportDisposition, Sticker},
    version::{ByteSize, Revision},
};
use memedock_storage::files::PendingStaging;
use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct ImportInput {
    pub(crate) pending: PendingStaging,
    pub(crate) library_id: LibraryId,
}
impl ImportInput {
    pub fn path(&self) -> &Path {
        self.pending.path()
    }
}
#[derive(Clone, Debug, Default)]
pub struct ImportOptions {
    pub original_name: String,
    pub title: Option<String>,
    pub collection: Option<CollectionId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportStatus {
    Created,
    Reused,
    RestoreRequired,
}
#[derive(Clone, Debug)]
pub struct ImportOutcome {
    pub sticker: Sticker,
    pub status: ImportStatus,
}

impl Library {
    pub fn create_import_input(&self) -> Result<Task<ImportInput>> {
        let identity = self.identity().library_id;
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                tokio::task::spawn_blocking(move || {
                    Ok(ImportInput {
                        pending: services.blobs.create_staging()?,
                        library_id: identity,
                    })
                })
                .await?
            },
        )
    }
    pub fn discard_import_input(&self, input: ImportInput) -> Result<Task<()>> {
        if input.library_id != self.identity().library_id {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "input belongs to another library",
            ));
        }
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |_, _| async move {
                tokio::task::spawn_blocking(move || Ok(input.pending.discard()?)).await?
            },
        )
    }
    pub fn import_staged(
        &self,
        input: ImportInput,
        options: ImportOptions,
    ) -> Result<Task<ImportOutcome>> {
        if input.library_id != self.identity().library_id {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "input belongs to another library",
            ));
        }
        let library = self.clone();
        self.submit(
            Lane::Import,
            Priority::Interactive,
            move |services, control| async move {
                let outcome = process(services, control, input, options, None).await?;
                if outcome.status != ImportStatus::RestoreRequired {
                    // Retain the real Task until completion; dropping it would cancel
                    // the thumbnail. Busy leaves Missing for a later visible request.
                    if let Ok(task) =
                        library.request_thumbnail(outcome.sticker.id(), Priority::Background)
                    {
                        tokio::spawn(async move {
                            let _result = task.wait().await;
                        });
                    }
                }
                Ok(outcome)
            },
        )
    }
    pub fn import_file(
        &self,
        path: PathBuf,
        options: ImportOptions,
    ) -> Result<Task<ImportOutcome>> {
        let library = self.clone();
        let identity = self.identity().library_id;
        self.submit(
            Lane::Import,
            Priority::Interactive,
            move |services, control| async move {
                let worker = services.clone();
                let cancel = control.clone();
                let pending = tokio::task::spawn_blocking(move || -> Result<PendingStaging> {
                    let pending = worker.blobs.create_staging()?;
                    let result = (|| -> Result<()> {
                        use std::io::{Read, Write};
                        let mut input = std::fs::File::open(path)?;
                        let mut output = std::fs::OpenOptions::new()
                            .write(true)
                            .open(pending.path())?;
                        let mut buffer = [0u8; 64 * 1024];
                        let mut bytes = 0u64;
                        loop {
                            cancel.check()?;
                            let count = input.read(&mut buffer)?;
                            if count == 0 {
                                break;
                            }
                            bytes += count as u64;
                            if bytes > worker.config.limits.max_file_bytes {
                                return Err(CoreError::new(
                                    ErrorCode::ResourceLimit,
                                    "input exceeds file limit",
                                ));
                            }
                            output.write_all(&buffer[..count])?;
                        }
                        output.sync_all()?;
                        Ok(())
                    })();
                    if let Err(error) = result {
                        pending.discard()?;
                        return Err(error);
                    }
                    Ok(pending)
                })
                .await??;
                let outcome = process(
                    services,
                    control,
                    ImportInput {
                        pending,
                        library_id: identity,
                    },
                    options,
                    None,
                )
                .await?;
                if outcome.status != ImportStatus::RestoreRequired
                    && let Ok(task) =
                        library.request_thumbnail(outcome.sticker.id(), Priority::Background)
                {
                    tokio::spawn(async move {
                        let _result = task.wait().await;
                    });
                }
                Ok(outcome)
            },
        )
    }
}
pub(crate) async fn process(
    services: Arc<Services>,
    control: Arc<TaskControl>,
    input: ImportInput,
    options: ImportOptions,
    source: Option<crate::sources::telegram::SourceCommit>,
) -> Result<ImportOutcome> {
    control.stage("validating_image");
    let allowance = services.image_budget.acquire().await?;
    if let Err(error) = control.check() {
        tokio::task::spawn_blocking(move || input.pending.discard()).await??;
        return Err(error);
    }
    let worker = services.clone();
    let cancel = control.clone();
    let validated = tokio::task::spawn_blocking(move || -> Result<_> {
        let _allowance = allowance;
        let result = (|| -> Result<_> {
            if std::fs::symlink_metadata(input.path())?.len() > worker.config.limits.max_file_bytes
            {
                return Err(CoreError::new(
                    ErrorCode::ResourceLimit,
                    "input exceeds file limit",
                ));
            }
            let staged = input
                .pending
                .finish(worker.config.limits.max_file_bytes, || {
                    cancel.is_cancelled()
                })?;
            let decoded =
                inspect::decode(staged.open_read()?, &worker.config.limits, &cancel, true)?;
            let at = crate::writes::now()?;
            let dimensions = (
                NonZeroU32::new(decoded.dimensions.0)
                    .ok_or_else(|| CoreError::new(ErrorCode::InvalidImage, "zero width"))?,
                NonZeroU32::new(decoded.dimensions.1)
                    .ok_or_else(|| CoreError::new(ErrorCode::InvalidImage, "zero height"))?,
            );
            let asset = Asset::new(
                staged.hash(),
                ByteSize::new(i64::try_from(staged.byte_size()).map_err(|_| {
                    CoreError::new(ErrorCode::ResourceLimit, "file size overflow")
                })?)?,
                decoded.format,
                dimensions,
                decoded.animated,
                at,
            );
            drop(decoded);
            cancel.stage("publishing_original");
            worker.blobs.publish(staged, || cancel.is_cancelled())?;
            Ok(asset)
        })();
        if result.is_err() && input.path().try_exists()? {
            input.pending.discard()?;
        }
        result
    })
    .await??;
    let _write = services
        .write_permit
        .acquire()
        .await
        .map_err(|_| CoreError::internal("write service closed"))?;
    control.check()?;
    let mut tx = services.db.begin_write().await?;
    let existing = tx.sticker(validated.sticker_id()).await?;
    if let Some(source) = &source
        && tx
            .source_item(&source.item)
            .await?
            .is_some_and(|old| old.sticker != validated.sticker_id())
    {
        return Err(CoreError::new(
            ErrorCode::Conflict,
            "source item maps to different immutable bytes",
        ));
    }
    if let Some(sticker) = &existing
        && sticker.classify_import(&validated)? == ImportDisposition::RestoreRequired
    {
        if let Some(source) = &source {
            control.begin_commit()?;
            tx.save_source_item(&memedock_domain::source::SourceItem {
                id: source.item.clone(),
                sticker: validated.sticker_id(),
            })
            .await?;
            tx.commit().await?;
        }
        return Ok(ImportOutcome {
            sticker: sticker.clone(),
            status: ImportStatus::RestoreRequired,
        });
    }
    let mut collection = match options.collection {
        Some(id) => {
            let collection = tx.collection(id).await?.ok_or_else(|| {
                CoreError::new(ErrorCode::NotFound, "target collection not found")
            })?;
            if !collection.lifecycle().is_active() {
                return Err(CoreError::new(
                    ErrorCode::EntityDeleted,
                    "target collection deleted",
                ));
            }
            Some(collection)
        }
        None => None,
    };
    let at = crate::writes::now()?;
    let status = if existing.is_some() {
        ImportStatus::Reused
    } else {
        ImportStatus::Created
    };
    if status == ImportStatus::Created
        && let Some(source) = &source
    {
        collection =
            Some(crate::sources::telegram::source_collection(&mut tx, source, at, &control).await?);
    }
    let title = options.title.unwrap_or_else(|| {
        Path::new(&options.original_name)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("未命名图片")
            .to_owned()
    });
    let sticker =
        existing.unwrap_or_else(|| Sticker::new(&validated, title, options.original_name, at));
    control.begin_commit()?;
    tx.insert_asset(&validated).await?;
    if status == ImportStatus::Created {
        tx.save_sticker(&sticker).await?;
        tx.append_change(
            OperationId::new(),
            Operation::new(OperationKind::CreateSticker {
                asset: validated.clone(),
                title: sticker.title().into(),
                original_name: sticker.original_name().into(),
                note: sticker.note().into(),
                starred: sticker.starred(),
            })?,
            at,
        )
        .await?;
    }
    if let Some(collection) = collection.filter(|_| status == ImportStatus::Created) {
        let old = tx.collection_item(collection.id(), sticker.id()).await?;
        if !old
            .as_ref()
            .is_some_and(|item| item.is_effective(&collection, &sticker))
        {
            let key = SortKey::between(
                tx.last_collection_key(collection.id()).await?.as_ref(),
                None,
            )?;
            let item = CollectionItem::new(&collection, &sticker, key, true, Revision::LOCAL, at)?;
            tx.save_collection_item(&item).await?;
            tx.append_change(
                OperationId::new(),
                Operation::new(OperationKind::SetStickerCollection {
                    sticker_id: sticker.id(),
                    sticker_generation: sticker.lifecycle().generation(),
                    collection: Some((collection.id(), collection.lifecycle().generation())),
                })?,
                at,
            )
            .await?;
        }
    }
    let mut local = tx
        .local_asset(validated.hash())
        .await?
        .unwrap_or_else(|| LocalAsset::new(validated.hash()));
    local.verified(at);
    tx.save_local_asset(&local).await?;
    if let Some(source) = &source {
        tx.save_source_item(&memedock_domain::source::SourceItem {
            id: source.item.clone(),
            sticker: sticker.id(),
        })
        .await?;
    }
    tx.commit().await?;
    if source.is_some() && status == ImportStatus::Created {
        services.events.publish(ChangeKind::CollectionsChanged)?;
    }
    services
        .events
        .publish(ChangeKind::StickerChanged(sticker.id()))?;
    Ok(ImportOutcome { sticker, status })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, time::Duration};

    #[tokio::test]
    async fn cancellation_while_waiting_for_decode_budget_discards_input()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let library = Library::open(crate::LibraryConfig::new(
            directory.path().join("data"),
            directory.path().join("cache"),
            directory.path().join("share"),
        ))
        .await?;
        let input = library.create_import_input()?.wait().await?;
        let path = input.path().to_owned();
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2).write_to(&mut png, image::ImageFormat::Png)?;
        std::fs::write(&path, png.into_inner())?;
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = tokio::sync::oneshot::channel();
        let blocker = library.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, _| async move {
                let _budget = services.image_budget.acquire().await?;
                let _ = started.send(());
                wait.await
                    .map_err(|_| CoreError::internal("test release dropped"))?;
                Ok(())
            },
        )?;
        tokio::time::timeout(Duration::from_secs(5), ready).await??;
        let task = library.import_staged(input, ImportOptions::default())?;
        let mut progress = task.progress();
        tokio::time::timeout(Duration::from_secs(5), async {
            while progress.snapshot().stage != "validating_image" {
                progress.next().await;
            }
        })
        .await?;
        assert_eq!(task.cancel(), crate::tasks::CancelResult::Requested);
        release.send(()).map_err(|_| "release dropped")?;
        blocker.wait().await?;
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), task.wait())
                .await?
                .err()
                .ok_or("cancellation expected")?
                .code(),
            ErrorCode::Cancelled
        );
        assert!(!path.exists());
        assert_eq!(library.space_statistics()?.wait().await?.known_assets, 0);
        library.close().await?;
        Ok(())
    }
}
