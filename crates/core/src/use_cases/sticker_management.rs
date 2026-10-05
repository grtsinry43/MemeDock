use crate::{
    CoreError, ErrorCode, Library, Result,
    events::ChangeKind,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::{
    change::{Operation, OperationKind, StickerPatch},
    identity::{CollectionId, OperationId, StickerId, TagId},
    ordering::SortKey,
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    version::{Generation, Revision},
};
use std::collections::BTreeMap;

impl Library {
    pub fn patch_sticker(
        &self,
        id: StickerId,
        generation: Generation,
        patch: StickerPatch,
    ) -> Result<Task<Sticker>> {
        patch.validate()?;
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
                let mut sticker = tx
                    .sticker(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker not found"))?;
                let at = crate::writes::now()?;
                sticker.patch(generation, &patch, at, sticker.lifecycle().revision())?;
                control.begin_commit()?;
                tx.save_sticker(&sticker).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::PatchSticker {
                        sticker_id: id,
                        generation,
                        patch,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::StickerChanged(id))?;
                Ok(sticker)
            },
        )
    }

    pub fn set_sticker_relations(
        &self,
        id: StickerId,
        generation: Generation,
        collections: Option<Vec<(CollectionId, Generation)>>,
        tags: Option<Vec<(TagId, Generation)>>,
    ) -> Result<Task<()>> {
        if collections.is_none() && tags.is_none() {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "empty relation edit",
            ));
        }
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
                let sticker = tx
                    .sticker(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker not found"))?;
                sticker.lifecycle().ensure_active(generation)?;
                let at = crate::writes::now()?;
                if let Some(targets) = collections {
                    let mut desired = BTreeMap::new();
                    for (cid, observed) in targets {
                        let c = tx.collection(cid).await?.ok_or_else(|| {
                            CoreError::new(ErrorCode::NotFound, "collection not found")
                        })?;
                        c.lifecycle().ensure_active(observed)?;
                        desired.insert(cid, c);
                    }
                    let current = tx.sticker_collections(id).await?;
                    for c in current.iter().filter(|c| !desired.contains_key(&c.id())) {
                        let mut row = tx
                            .collection_item(c.id(), id)
                            .await?
                            .ok_or_else(|| CoreError::internal("membership missing"))?;
                        row.set_present(c, &sticker, false, row.revision(), at)?;
                        control.begin_commit()?;
                        tx.save_collection_item(&row).await?;
                        tx.append_change(
                            OperationId::new(),
                            Operation::new(OperationKind::SetCollectionMembership {
                                collection_id: c.id(),
                                sticker_id: id,
                                collection_generation: c.lifecycle().generation(),
                                sticker_generation: generation,
                                present: false,
                            })?,
                            at,
                        )
                        .await?;
                    }
                    for c in desired
                        .values()
                        .filter(|c| !current.iter().any(|old| old.id() == c.id()))
                    {
                        let key =
                            SortKey::between(tx.last_collection_key(c.id()).await?.as_ref(), None)?;
                        let old = tx.collection_item(c.id(), id).await?;
                        let revision = old
                            .as_ref()
                            .map_or(Revision::LOCAL, CollectionItem::revision);
                        let row = CollectionItem::new(c, &sticker, key, true, revision, at)?;
                        control.begin_commit()?;
                        tx.save_collection_item(&row).await?;
                        tx.append_change(
                            OperationId::new(),
                            Operation::new(OperationKind::SetCollectionMembership {
                                collection_id: c.id(),
                                sticker_id: id,
                                collection_generation: c.lifecycle().generation(),
                                sticker_generation: generation,
                                present: true,
                            })?,
                            at,
                        )
                        .await?;
                    }
                }
                if let Some(targets) = tags {
                    let mut desired = BTreeMap::new();
                    for (tid, observed) in targets {
                        let t = tx
                            .tag(tid)
                            .await?
                            .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "tag not found"))?;
                        t.lifecycle().ensure_active(observed)?;
                        desired.insert(tid, t);
                    }
                    let current = tx.sticker_tags(id).await?;
                    for t in current.iter().filter(|t| !desired.contains_key(&t.id())) {
                        let mut row = tx
                            .sticker_tag(id, t.id())
                            .await?
                            .ok_or_else(|| CoreError::internal("membership missing"))?;
                        row.set_present(&sticker, t, false, row.revision(), at)?;
                        control.begin_commit()?;
                        tx.save_sticker_tag(&row).await?;
                        tx.append_change(
                            OperationId::new(),
                            Operation::new(OperationKind::SetTagMembership {
                                sticker_id: id,
                                tag_id: t.id(),
                                sticker_generation: generation,
                                tag_generation: t.lifecycle().generation(),
                                present: false,
                            })?,
                            at,
                        )
                        .await?;
                    }
                    for t in desired
                        .values()
                        .filter(|t| !current.iter().any(|old| old.id() == t.id()))
                    {
                        let old = tx.sticker_tag(id, t.id()).await?;
                        let revision = old.as_ref().map_or(Revision::LOCAL, StickerTag::revision);
                        let row = StickerTag::new(&sticker, t, true, revision, at)?;
                        control.begin_commit()?;
                        tx.save_sticker_tag(&row).await?;
                        tx.append_change(
                            OperationId::new(),
                            Operation::new(OperationKind::SetTagMembership {
                                sticker_id: id,
                                tag_id: t.id(),
                                sticker_generation: generation,
                                tag_generation: t.lifecycle().generation(),
                                present: true,
                            })?,
                            at,
                        )
                        .await?;
                    }
                }
                control.begin_commit()?;
                tx.commit().await?;
                services.events.publish(ChangeKind::StickerChanged(id))?;
                Ok(())
            },
        )
    }
}
