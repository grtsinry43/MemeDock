use crate::{
    CoreError, ErrorCode, Library, Result,
    events::ChangeKind,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::{
    change::{NamePatch, Operation, OperationKind},
    collection::Collection,
    identity::{CollectionId, OperationId, StickerId},
    ordering::SortKey,
    tag::Name,
    version::Generation,
};

impl Library {
    pub fn create_collection(
        &self,
        name: Name,
        before: Option<CollectionId>,
    ) -> Result<Task<Collection>> {
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
                let ordered = collection_order(&mut tx, &control).await?;
                let position = match before {
                    Some(id) => ordered.iter().position(|c| c.id() == id).ok_or_else(|| {
                        CoreError::new(ErrorCode::Conflict, "ordering anchor missing")
                    })?,
                    None => ordered.len(),
                };
                let key = SortKey::between(
                    position.checked_sub(1).map(|i| ordered[i].sort_key()),
                    ordered.get(position).map(Collection::sort_key),
                )?;
                let at = crate::writes::now()?;
                let value = Collection::new(CollectionId::new(), name.clone(), key, at);
                control.begin_commit()?;
                tx.save_collection(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::CreateCollection {
                        collection_id: value.id(),
                        name,
                        before_id: before,
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
    pub fn move_collection(
        &self,
        id: CollectionId,
        generation: Generation,
        before: Option<CollectionId>,
    ) -> Result<Task<Collection>> {
        if before == Some(id) {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "cannot move before itself",
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
                let mut value = tx
                    .collection(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "collection missing"))?;
                value.lifecycle().ensure_active(generation)?;
                let ordered: Vec<_> = collection_order(&mut tx, &control)
                    .await?
                    .into_iter()
                    .filter(|c| c.id() != id)
                    .collect();
                let position = match before {
                    Some(anchor) => {
                        ordered
                            .iter()
                            .position(|c| c.id() == anchor)
                            .ok_or_else(|| {
                                CoreError::new(ErrorCode::Conflict, "ordering anchor missing")
                            })?
                    }
                    None => ordered.len(),
                };
                let key = SortKey::between(
                    position.checked_sub(1).map(|i| ordered[i].sort_key()),
                    ordered.get(position).map(Collection::sort_key),
                )?;
                let at = crate::writes::now()?;
                value.set_sort_key(generation, key, at, value.lifecycle().revision())?;
                control.begin_commit()?;
                tx.save_collection(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::MoveCollection {
                        collection_id: id,
                        generation,
                        before_id: before,
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
    pub fn move_collection_item(
        &self,
        collection: CollectionId,
        collection_generation: Generation,
        sticker: StickerId,
        sticker_generation: Generation,
        before: Option<StickerId>,
    ) -> Result<Task<()>> {
        if before == Some(sticker) {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "cannot move before itself",
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
                let c = tx
                    .collection(collection)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "collection missing"))?;
                let s = tx
                    .sticker(sticker)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker missing"))?;
                c.lifecycle().ensure_active(collection_generation)?;
                s.lifecycle().ensure_active(sticker_generation)?;
                let mut row = tx
                    .collection_item(collection, sticker)
                    .await?
                    .filter(|row| row.is_effective(&c, &s))
                    .ok_or_else(|| {
                        CoreError::new(ErrorCode::Conflict, "membership not effective")
                    })?;
                let ordered: Vec<_> = member_order(&mut tx, &control, &c)
                    .await?
                    .into_iter()
                    .filter(|r| r.sticker_id() != sticker)
                    .collect();
                let position = match before {
                    Some(anchor) => ordered
                        .iter()
                        .position(|r| r.sticker_id() == anchor)
                        .ok_or_else(|| {
                            CoreError::new(ErrorCode::Conflict, "ordering anchor missing")
                        })?,
                    None => ordered.len(),
                };
                let key = SortKey::between(
                    position.checked_sub(1).map(|i| ordered[i].sort_key()),
                    ordered.get(position).map(|r| r.sort_key()),
                )?;
                let at = crate::writes::now()?;
                row.move_to(&c, &s, key, row.revision(), at)?;
                control.begin_commit()?;
                tx.save_collection_item(&row).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::MoveCollectionItem {
                        collection_id: collection,
                        sticker_id: sticker,
                        collection_generation,
                        sticker_generation,
                        before_id: before,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::CollectionsChanged)?;
                services
                    .events
                    .publish(ChangeKind::StickerChanged(sticker))?;
                Ok(())
            },
        )
    }
}
impl Library {
    pub fn rename_collection(
        &self,
        id: CollectionId,
        generation: Generation,
        name: Name,
    ) -> Result<Task<Collection>> {
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
                    .collection(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "collection missing"))?;
                value.lifecycle().ensure_active(generation)?;
                if value.name() == &name {
                    return Ok(value);
                }
                let at = crate::writes::now()?;
                let patch = NamePatch::new(name);
                value.patch(generation, &patch, at, value.lifecycle().revision())?;
                control.begin_commit()?;
                tx.save_collection(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::PatchCollection {
                        collection_id: id,
                        generation,
                        patch,
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
    pub fn delete_collection(
        &self,
        id: CollectionId,
        generation: Generation,
    ) -> Result<Task<Collection>> {
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
                    .collection(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "collection missing"))?;
                let at = crate::writes::now()?;
                if !value.delete(generation, at, value.lifecycle().revision())? {
                    return Ok(value);
                }
                control.begin_commit()?;
                tx.save_collection(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::DeleteCollection {
                        collection_id: id,
                        generation,
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

async fn collection_order(
    tx: &mut memedock_storage::transaction::WriteTransaction,
    control: &crate::tasks::TaskControl,
) -> Result<Vec<Collection>> {
    let mut values = tx.active_collections().await?;
    if values
        .windows(2)
        .any(|pair| pair[0].sort_key() >= pair[1].sort_key())
    {
        let at = crate::writes::now()?;
        let keys = SortKey::rebalance(values.len())?;
        let ids: Vec<_> = values.iter().map(Collection::id).collect();
        control.begin_commit()?;
        for (index, (value, key)) in values.iter_mut().zip(keys).enumerate() {
            let generation = value.lifecycle().generation();
            value.set_sort_key(generation, key, at, value.lifecycle().revision())?;
            tx.save_collection(value).await?;
            tx.append_change(
                OperationId::new(),
                Operation::new(OperationKind::MoveCollection {
                    collection_id: value.id(),
                    generation,
                    before_id: ids.get(index + 1).copied(),
                })?,
                at,
            )
            .await?;
        }
    }
    Ok(values)
}
async fn member_order(
    tx: &mut memedock_storage::transaction::WriteTransaction,
    control: &crate::tasks::TaskControl,
    collection: &Collection,
) -> Result<Vec<memedock_domain::relation::CollectionItem>> {
    let mut values = tx.effective_collection_items(collection.id()).await?;
    if values
        .windows(2)
        .any(|pair| pair[0].sort_key() >= pair[1].sort_key())
    {
        let at = crate::writes::now()?;
        let keys = SortKey::rebalance(values.len())?;
        let ids: Vec<_> = values.iter().map(|value| value.sticker_id()).collect();
        control.begin_commit()?;
        for (index, (value, key)) in values.iter_mut().zip(keys).enumerate() {
            let sticker = tx
                .sticker(value.sticker_id())
                .await?
                .ok_or_else(|| CoreError::internal("member missing"))?;
            value.move_to(collection, &sticker, key, value.revision(), at)?;
            tx.save_collection_item(value).await?;
            tx.append_change(
                OperationId::new(),
                Operation::new(OperationKind::MoveCollectionItem {
                    collection_id: collection.id(),
                    sticker_id: sticker.id(),
                    collection_generation: collection.lifecycle().generation(),
                    sticker_generation: sticker.lifecycle().generation(),
                    before_id: ids.get(index + 1).copied(),
                })?,
                at,
            )
            .await?;
        }
    }
    Ok(values)
}
