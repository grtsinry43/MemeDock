use crate::{
    CoreError, ErrorCode, Library, Result,
    events::ChangeKind,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::{
    change::{Operation, OperationKind},
    collection::Collection,
    identity::{CollectionId, OperationId, StickerId, TagId},
    sticker::Sticker,
    tag::Tag,
    version::{Generation, Revision},
};

#[derive(Clone, Debug)]
pub struct RestoreSuggestions {
    pub collection: Option<Collection>,
    pub tags: Vec<Tag>,
}

impl Library {
    pub fn delete_sticker(&self, id: StickerId, generation: Generation) -> Result<Task<Sticker>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            move |services, control| async move {
                let _permit = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                control.check()?;
                let mut tx = services.db.begin_write().await?;
                let mut value = tx
                    .sticker(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker missing"))?;
                let at = crate::writes::now()?;
                if !value.delete(generation, at, value.lifecycle().revision())? {
                    return Ok(value);
                }
                control.begin_commit()?;
                tx.save_sticker(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::DeleteSticker {
                        sticker_id: id,
                        generation,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::StickerChanged(id))?;
                Ok(value)
            },
        )
    }
    pub fn restore_suggestions(&self, id: StickerId) -> Result<Task<RestoreSuggestions>> {
        self.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                services
                    .db
                    .sticker(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker missing"))?;
                Ok(RestoreSuggestions {
                    collection: services.db.previous_sticker_collection(id).await?,
                    tags: services.db.previous_sticker_tags(id).await?,
                })
            },
        )
    }
}

impl Library {
    pub fn restore_sticker(
        &self,
        id: StickerId,
        generation: Generation,
        deleted_revision: Revision,
    ) -> Result<Task<Sticker>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                let asset = services.db.asset(id.content_hash()).await?.ok_or_else(|| {
                    CoreError::new(ErrorCode::NotFound, "original metadata missing")
                })?;
                let size = u64::try_from(asset.byte_size().get())
                    .map_err(|_| CoreError::new(ErrorCode::CorruptData, "invalid asset size"))?;
                let blobs = services.blobs.clone();
                let check = control.clone();
                tokio::task::spawn_blocking(move || -> Result<()> {
                    blobs.verify(id.content_hash(), size, || check.is_cancelled())?;
                    Ok(())
                })
                .await??;
                let _permit = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                control.check()?;
                let mut tx = services.db.begin_write().await?;
                let mut value = tx
                    .sticker(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker missing"))?;
                if value.lifecycle().generation() != generation {
                    return Err(CoreError::new(
                        ErrorCode::Conflict,
                        "stale restore generation",
                    ));
                }
                let at = crate::writes::now()?;
                value.restore(deleted_revision, at, value.lifecycle().revision())?;
                control.begin_commit()?;
                tx.save_sticker(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::RestoreSticker {
                        sticker_id: id,
                        expected_deleted_revision: deleted_revision,
                        rebuild: None,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::StickerChanged(id))?;
                Ok(value)
            },
        )
    }
}

impl Library {
    pub fn restore_collection(
        &self,
        id: CollectionId,
        generation: Generation,
        deleted_revision: Revision,
    ) -> Result<Task<Collection>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                let _permit = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                control.check()?;
                let mut tx = services.db.begin_write().await?;
                let mut value = tx
                    .collection(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "collection missing"))?;
                if value.lifecycle().generation() != generation {
                    return Err(CoreError::new(
                        ErrorCode::Conflict,
                        "stale restore generation",
                    ));
                }
                let at = crate::writes::now()?;
                value.restore(deleted_revision, at, value.lifecycle().revision())?;
                control.begin_commit()?;
                tx.save_collection(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::RestoreCollection {
                        collection_id: id,
                        expected_deleted_revision: deleted_revision,
                        rebuild: None,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::CollectionsChanged)?;
                Ok(value)
            },
        )
    }
}

impl Library {
    pub fn restore_tag(
        &self,
        id: TagId,
        generation: Generation,
        deleted_revision: Revision,
    ) -> Result<Task<Tag>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                let _permit = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                control.check()?;
                let mut tx = services.db.begin_write().await?;
                let mut value = tx
                    .tag(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "tag missing"))?;
                if value.lifecycle().generation() != generation {
                    return Err(CoreError::new(
                        ErrorCode::Conflict,
                        "stale restore generation",
                    ));
                }
                let at = crate::writes::now()?;
                value.restore(deleted_revision, at, value.lifecycle().revision())?;
                control.begin_commit()?;
                tx.save_tag(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::RestoreTag {
                        tag_id: id,
                        expected_deleted_revision: deleted_revision,
                        rebuild: None,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::TagsChanged)?;
                Ok(value)
            },
        )
    }
}
