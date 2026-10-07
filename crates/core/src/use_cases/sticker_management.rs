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

pub(crate) enum TagUpdate {
    Replace(Vec<(TagId, Generation)>),
    Add(Vec<(TagId, Generation)>),
    Remove(Vec<(TagId, Generation)>),
}

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
                let previous = sticker.clone();
                sticker.patch(generation, &patch, at, sticker.lifecycle().revision())?;
                if sticker.title() == previous.title()
                    && sticker.note() == previous.note()
                    && sticker.starred() == previous.starred()
                {
                    return Ok(previous);
                }
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

    pub fn set_sticker_organization(
        &self,
        id: StickerId,
        generation: Generation,
        collection: Option<Option<(CollectionId, Generation)>>,
        tags: Option<Vec<(TagId, Generation)>>,
    ) -> Result<Task<bool>> {
        self.edit_organization(id, generation, collection, tags.map(TagUpdate::Replace))
    }

    pub(crate) fn edit_organization(
        &self,
        id: StickerId,
        generation: Generation,
        collection: Option<Option<(CollectionId, Generation)>>,
        tags: Option<TagUpdate>,
    ) -> Result<Task<bool>> {
        self.edit_organization_checked(id, generation, collection, tags, None)
    }

    pub(crate) fn clear_sticker_collection(
        &self,
        id: StickerId,
        generation: Generation,
        expected: Option<(CollectionId, Generation)>,
    ) -> Result<Task<bool>> {
        self.edit_organization_checked(id, generation, Some(None), None, expected)
    }

    fn edit_organization_checked(
        &self,
        id: StickerId,
        generation: Generation,
        collection: Option<Option<(CollectionId, Generation)>>,
        tags: Option<TagUpdate>,
        expected: Option<(CollectionId, Generation)>,
    ) -> Result<Task<bool>> {
        if collection.is_none() && tags.is_none() {
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
                if let Some((cid, observed)) = expected {
                    let current = tx.sticker_collection(id).await?;
                    if !current
                        .as_ref()
                        .is_some_and(|c| c.id() == cid && c.lifecycle().generation() == observed)
                    {
                        return Err(CoreError::new(ErrorCode::Conflict, "assignment changed"));
                    }
                }
                let at = crate::writes::now()?;
                let mut changed = false;
                if let Some(target) = collection {
                    let current = tx.sticker_collection(id).await?;
                    let desired = match target {
                        Some((cid, observed)) => {
                            let value = tx.collection(cid).await?.ok_or_else(|| {
                                CoreError::new(ErrorCode::NotFound, "collection not found")
                            })?;
                            value.lifecycle().ensure_active(observed)?;
                            Some(value)
                        }
                        None => None,
                    };
                    if current.as_ref().map(|c| c.id()) != desired.as_ref().map(|c| c.id()) {
                        if let Some(value) = desired {
                            let key = SortKey::between(
                                tx.last_collection_key(value.id()).await?.as_ref(),
                                None,
                            )?;
                            let row = CollectionItem::new(
                                &value,
                                &sticker,
                                key,
                                true,
                                Revision::LOCAL,
                                at,
                            )?;
                            control.begin_commit()?;
                            tx.save_collection_item(&row).await?;
                            changed = true;
                            tx.append_change(
                                OperationId::new(),
                                Operation::new(OperationKind::SetStickerCollection {
                                    sticker_id: id,
                                    sticker_generation: generation,
                                    collection: Some((value.id(), value.lifecycle().generation())),
                                })?,
                                at,
                            )
                            .await?;
                        } else if let Some(value) = current {
                            let mut row = tx
                                .collection_item(value.id(), id)
                                .await?
                                .ok_or_else(|| CoreError::internal("assignment missing"))?;
                            row.set_present(&value, &sticker, false, row.revision(), at)?;
                            control.begin_commit()?;
                            tx.save_collection_item(&row).await?;
                            changed = true;
                            tx.append_change(
                                OperationId::new(),
                                Operation::new(OperationKind::SetStickerCollection {
                                    sticker_id: id,
                                    sticker_generation: generation,
                                    collection: None,
                                })?,
                                at,
                            )
                            .await?;
                        }
                    }
                }
                if let Some(update) = tags {
                    let current = tx.sticker_tags(id).await?;
                    let adding = matches!(&update, TagUpdate::Add(_));
                    let removing = matches!(&update, TagUpdate::Remove(_));
                    let targets = match update {
                        TagUpdate::Replace(v) | TagUpdate::Add(v) | TagUpdate::Remove(v) => v,
                    };
                    let mut desired = BTreeMap::new();
                    for (tid, observed) in targets {
                        let t = tx
                            .tag(tid)
                            .await?
                            .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "tag not found"))?;
                        t.lifecycle().ensure_active(observed)?;
                        desired.insert(tid, t);
                    }
                    if adding {
                        for value in &current {
                            desired.entry(value.id()).or_insert_with(|| value.clone());
                        }
                    } else if removing {
                        let removed = desired;
                        desired = current
                            .iter()
                            .filter(|v| !removed.contains_key(&v.id()))
                            .map(|v| (v.id(), v.clone()))
                            .collect();
                    }
                    for t in current.iter().filter(|t| !desired.contains_key(&t.id())) {
                        let mut row = tx
                            .sticker_tag(id, t.id())
                            .await?
                            .ok_or_else(|| CoreError::internal("membership missing"))?;
                        row.set_present(&sticker, t, false, row.revision(), at)?;
                        control.begin_commit()?;
                        tx.save_sticker_tag(&row).await?;
                        changed = true;
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
                        changed = true;
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
                if changed {
                    services.events.publish(ChangeKind::StickerChanged(id))?;
                }
                Ok(changed)
            },
        )
    }
}
