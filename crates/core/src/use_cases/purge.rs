use crate::{
    CoreError, ErrorCode, Library, Result,
    events::ChangeKind,
    runtime::Services,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::{
    change::{Operation, OperationKind},
    identity::{OperationId, StickerId},
    lifecycle::EntityId,
    version::{Generation, Revision},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TrashTarget {
    pub entity: EntityId,
    pub generation: Generation,
    pub deleted_revision: Revision,
}

impl Library {
    /// A fixed confirmation snapshot, including stickers, collections and tags.
    pub fn trash_snapshot(&self) -> Result<Task<Vec<TrashTarget>>> {
        self.submit(
            Lane::Read,
            Priority::Interactive,
            |services, control| async move {
                let _write = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                control.check()?;
                let data = services
                    .db
                    .archive_data_bounded(services.config.limits.max_archive_metadata_bytes)
                    .await?;
                let entries = data
                    .stickers
                    .iter()
                    .map(|v| (EntityId::Sticker(v.id()), v.lifecycle()))
                    .chain(
                        data.collections
                            .iter()
                            .map(|v| (EntityId::Collection(v.id()), v.lifecycle())),
                    )
                    .chain(
                        data.tags
                            .iter()
                            .map(|v| (EntityId::Tag(v.id()), v.lifecycle())),
                    );
                Ok(entries
                    .filter(|(_, life)| !life.is_active() && !life.is_purged())
                    .map(|(entity, life)| TrashTarget {
                        entity,
                        generation: life.generation(),
                        deleted_revision: life.revision(),
                    })
                    .collect())
            },
        )
    }
    pub fn purge_entity(
        &self,
        entity: EntityId,
        generation: Generation,
        deleted_revision: Revision,
    ) -> Result<Task<()>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            move |services, control| async move {
                {
                    let _write = services
                        .write_permit
                        .acquire()
                        .await
                        .map_err(|_| CoreError::internal("write service closed"))?;
                    control.check()?;
                    let at = crate::writes::now()?;
                    let mut tx = services.db.begin_write().await?;
                    let changed = match entity {
                        EntityId::Sticker(id) => {
                            let mut v = tx.sticker(id).await?.ok_or_else(|| {
                                CoreError::new(ErrorCode::NotFound, "sticker missing")
                            })?;
                            let changed = v.purge(
                                generation,
                                deleted_revision,
                                at,
                                v.lifecycle().revision(),
                            )?;
                            if changed {
                                tx.save_sticker(&v).await?;
                            }
                            changed
                        }
                        EntityId::Collection(id) => {
                            let mut v = tx.collection(id).await?.ok_or_else(|| {
                                CoreError::new(ErrorCode::NotFound, "collection missing")
                            })?;
                            let changed = v.purge(
                                generation,
                                deleted_revision,
                                at,
                                v.lifecycle().revision(),
                            )?;
                            if changed {
                                tx.save_collection(&v).await?;
                            }
                            changed
                        }
                        EntityId::Tag(id) => {
                            let mut v = tx.tag(id).await?.ok_or_else(|| {
                                CoreError::new(ErrorCode::NotFound, "tag missing")
                            })?;
                            let changed = v.purge(
                                generation,
                                deleted_revision,
                                at,
                                v.lifecycle().revision(),
                            )?;
                            if changed {
                                tx.save_tag(&v).await?;
                            }
                            changed
                        }
                    };
                    if changed {
                        control.begin_commit()?;
                        tx.append_change(
                            OperationId::new(),
                            Operation::new(OperationKind::PurgeEntity {
                                entity,
                                generation,
                                expected_deleted_revision: deleted_revision,
                            })?,
                            at,
                        )
                        .await?;
                        tx.commit().await?;
                    }
                }
                match entity {
                    EntityId::Sticker(id) => {
                        services.events.publish(ChangeKind::StickerChanged(id))?
                    }
                    EntityId::Collection(_) => {
                        services.events.publish(ChangeKind::CollectionsChanged)?
                    }
                    EntityId::Tag(_) => services.events.publish(ChangeKind::TagsChanged)?,
                };
                cleanup(&services).await
            },
        )
    }
    pub fn retry_purge_cleanup(&self) -> Result<Task<()>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            |services, _| async move { cleanup(&services).await },
        )
    }
}

/// Each batch is bounded. File publication and cleanup share the writer gate;
/// image workers are drained first, following their image -> writer lock order.
pub(crate) async fn cleanup(services: &Arc<Services>) -> Result<()> {
    loop {
        let jobs = services.db.purge_jobs().await?;
        if jobs.is_empty() {
            let _exports = services
                .artifacts
                .permit
                .acquire()
                .await
                .map_err(|_| CoreError::internal("artifact manager closed"))?;
            super::artifact_maintenance::cleanup(services, crate::writes::now()?.get()).await?;
            return Ok(());
        }
        for (hash, generation) in jobs {
            let id = StickerId::new(hash);
            let _image = super::thumbnail::image_lock(services, id).await?;
            let _write = services
                .write_permit
                .acquire()
                .await
                .map_err(|_| CoreError::internal("write service closed"))?;
            if services.db.sticker(id).await?.is_some_and(|v| {
                v.lifecycle().is_purged() && v.lifecycle().generation() == generation
            }) {
                let blobs = services.blobs.clone();
                let cache = services.derived.clone();
                tokio::task::spawn_blocking(move || -> Result<()> {
                    blobs.remove_original(hash)?;
                    cache.remove_for(hash)?;
                    Ok(())
                })
                .await??;
                let mut tx = services.db.begin_write().await?;
                tx.save_local_asset(&memedock_domain::local::LocalAsset::new(hash))
                    .await?;
                tx.commit().await?;
            }
            services.db.finish_purge_job(hash, generation).await?;
        }
    }
}
