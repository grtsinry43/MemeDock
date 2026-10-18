use memedock_domain::{
    change::OperationKind,
    collection::Collection,
    error::DomainError,
    identity::{CollectionId, StickerId},
    lifecycle::{EntityId, Lifecycle, Tombstone},
    ordering::SortKey,
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    sync::{EventKind, RejectionCode, ResolutionNote},
    tag::Tag,
    version::{Generation, Revision, TimestampMs},
};
use memedock_storage::{StorageError, server::SyncTransaction};

pub(super) enum ApplyError {
    Storage(StorageError),
    Rejected(RejectionCode),
}
impl From<StorageError> for ApplyError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}
impl From<DomainError> for ApplyError {
    fn from(error: DomainError) -> Self {
        Self::Rejected(match error {
            DomainError::EntityDeleted => RejectionCode::EntityDeleted,
            DomainError::StaleGeneration => RejectionCode::StaleGeneration,
            DomainError::StaleRevision => RejectionCode::StaleRevision,
            _ => RejectionCode::InvalidOperation,
        })
    }
}
type Result<T> = std::result::Result<T, ApplyError>;
fn missing<T>(value: Option<T>) -> Result<T> {
    value.ok_or(ApplyError::Rejected(RejectionCode::EntityMissing))
}
fn reject<T>(code: RejectionCode) -> Result<T> {
    Err(ApplyError::Rejected(code))
}
fn tombstone_key(id: EntityId) -> String {
    match id {
        EntityId::Sticker(id) => format!("sticker:{id}"),
        EntityId::Collection(id) => format!("collection:{id}"),
        EntityId::Tag(id) => format!("tag:{id}"),
    }
}
async fn ensure_new(tx: &SyncTransaction, id: EntityId) -> Result<()> {
    if tx
        .record::<Tombstone>("tombstone", &tombstone_key(id))
        .await?
        .is_some()
    {
        return reject(RejectionCode::EntityDeleted);
    }
    Ok(())
}
async fn key(
    tx: &SyncTransaction,
    kind: &str,
    parent: Option<&str>,
    before: Option<String>,
    exclude: &str,
) -> Result<(SortKey, Option<ResolutionNote>)> {
    let (left, right, missing) = tx
        .neighbors(kind, parent, before.as_deref(), exclude)
        .await?;
    let left = left.map(|s| s.parse::<SortKey>()).transpose()?;
    let right = right.map(|s| s.parse::<SortKey>()).transpose()?;
    Ok((
        SortKey::between(left.as_ref(), right.as_ref())?,
        missing.then_some(ResolutionNote::AnchorMissing),
    ))
}
async fn verified(
    tx: &SyncTransaction,
    asset: &memedock_domain::asset::Asset,
) -> Result<memedock_domain::asset::Asset> {
    let actual = missing(tx.asset(asset.hash()).await?)?;
    if actual.byte_size() != asset.byte_size()
        || actual.format() != asset.format()
        || actual.width() != asset.width()
        || actual.height() != asset.height()
        || actual.animated() != asset.animated()
    {
        return reject(RejectionCode::InvalidOperation);
    }
    Ok(actual)
}

pub(super) async fn apply(
    tx: &SyncTransaction,
    op: &OperationKind,
    revision: Revision,
    at: TimestampMs,
) -> Result<(Option<EventKind>, Option<ResolutionNote>)> {
    let mut note = None;
    let event = match op {
        OperationKind::PurgeEntity {
            entity: id,
            generation,
            expected_deleted_revision,
        } => {
            let (kind, key) = match id {
                EntityId::Sticker(id) => ("sticker", id.to_string()),
                EntityId::Collection(id) => ("collection", id.to_string()),
                EntityId::Tag(id) => ("tag", id.to_string()),
            };
            if tx.record::<serde_json::Value>(kind, &key).await?.is_none() {
                let prior = tx
                    .record::<Tombstone>("tombstone", &tombstone_key(*id))
                    .await?;
                if let Some(prior) = &prior {
                    if prior.generation() != *generation {
                        return reject(RejectionCode::StaleGeneration);
                    }
                    if prior.purged_at().is_some() {
                        return Ok((None, None));
                    }
                    if prior.revision() != *expected_deleted_revision {
                        return reject(RejectionCode::StaleRevision);
                    }
                }
                let entity = Tombstone::new(
                    *id,
                    *generation,
                    revision,
                    prior.map_or(at, |v| v.deleted_at()),
                )
                .with_purged_at(Some(at));
                tx.save("tombstone", &tombstone_key(*id), &entity, None, None)
                    .await?;
                if let EntityId::Sticker(id) = id {
                    tx.queue_purge(*id, *generation).await?;
                }
                return Ok((Some(EventKind::EntityPurged { entity }), None));
            }
            let life = match id {
                EntityId::Sticker(id) => {
                    let mut v = missing(tx.record::<Sticker>("sticker", &id.to_string()).await?)?;
                    if !v.purge(*generation, *expected_deleted_revision, at, revision)? {
                        return Ok((None, None));
                    }
                    tx.save("sticker", &id.to_string(), &v, None, None).await?;
                    tx.queue_purge(*id, *generation).await?;
                    v.lifecycle().clone()
                }
                EntityId::Collection(id) => {
                    let mut v = missing(
                        tx.record::<Collection>("collection", &id.to_string())
                            .await?,
                    )?;
                    if !v.purge(*generation, *expected_deleted_revision, at, revision)? {
                        return Ok((None, None));
                    }
                    tx.save(
                        "collection",
                        &id.to_string(),
                        &v,
                        Some(&v.sort_key().to_string()),
                        None,
                    )
                    .await?;
                    v.lifecycle().clone()
                }
                EntityId::Tag(id) => {
                    let mut v = missing(tx.record::<Tag>("tag", &id.to_string()).await?)?;
                    if !v.purge(*generation, *expected_deleted_revision, at, revision)? {
                        return Ok((None, None));
                    }
                    tx.save("tag", &id.to_string(), &v, None, None).await?;
                    v.lifecycle().clone()
                }
            };
            let entity = Tombstone::new(
                *id,
                *generation,
                revision,
                life.deleted_at().ok_or(DomainError::InvalidTransition)?,
            )
            .with_purged_at(Some(at));
            tx.save("tombstone", &tombstone_key(*id), &entity, None, None)
                .await?;
            EventKind::EntityPurged { entity }
        }
        OperationKind::ReimportSticker {
            generation,
            rebuild,
        } => {
            let id = rebuild.asset.sticker_id();
            let old = tx.record::<Sticker>("sticker", &id.to_string()).await?;
            let old_life = match old {
                Some(v) => v.lifecycle().clone(),
                None => {
                    let t = missing(
                        tx.record::<Tombstone>("tombstone", &tombstone_key(EntityId::Sticker(id)))
                            .await?,
                    )?;
                    Lifecycle::from_state(
                        t.generation(),
                        t.revision(),
                        at,
                        at,
                        Some(t.deleted_at()),
                    )
                    .with_purged_at(t.purged_at())
                }
            };
            if old_life.generation() != *generation {
                return reject(RejectionCode::StaleGeneration);
            }
            let life = old_life.reimport(at, revision)?;
            let asset = verified(tx, &rebuild.asset).await?;
            let entity = Sticker::from_state(
                id,
                rebuild.title.clone(),
                rebuild.original_name.clone(),
                rebuild.note.clone(),
                rebuild.starred,
                life,
            );
            tx.save("sticker", &id.to_string(), &entity, None, None)
                .await?;
            EventKind::StickerCreated { asset, entity }
        }
        OperationKind::ImportArchive { .. } => return reject(RejectionCode::InvalidOperation), // Local audit only; never a replayable network command.
        OperationKind::CreateSticker {
            asset,
            title,
            original_name,
            note,
            starred,
        } => {
            ensure_new(tx, EntityId::Sticker(asset.sticker_id())).await?;
            if tx
                .record::<Sticker>("sticker", &asset.sticker_id().to_string())
                .await?
                .is_some()
            {
                return Ok((None, None));
            }
            let asset = verified(tx, asset).await?;
            let entity = Sticker::from_state(
                asset.sticker_id(),
                title.clone(),
                original_name.clone(),
                note.clone(),
                *starred,
                Lifecycle::from_state(Generation::INITIAL, revision, at, at, None),
            );
            tx.save("sticker", &entity.id().to_string(), &entity, None, None)
                .await?;
            EventKind::StickerCreated { asset, entity }
        }
        OperationKind::PatchSticker {
            sticker_id,
            generation,
            patch,
        } => {
            let mut entity = missing(
                tx.record::<Sticker>("sticker", &sticker_id.to_string())
                    .await?,
            )?;
            entity.lifecycle().ensure_active(*generation)?;
            let mut candidate = entity.clone();
            candidate.patch(*generation, patch, at, revision)?;
            if candidate.title() == entity.title()
                && candidate.note() == entity.note()
                && candidate.starred() == entity.starred()
            {
                return Ok((None, None));
            }
            entity = candidate;
            tx.save("sticker", &sticker_id.to_string(), &entity, None, None)
                .await?;
            EventKind::StickerPatched { entity }
        }
        OperationKind::CreateCollection {
            collection_id,
            name,
            before_id,
        } => {
            ensure_new(tx, EntityId::Collection(*collection_id)).await?;
            if tx
                .record::<Collection>("collection", &collection_id.to_string())
                .await?
                .is_some()
            {
                return Ok((None, None));
            }
            let (sort, n) = key(
                tx,
                "collection",
                None,
                before_id.map(|id| id.to_string()),
                &collection_id.to_string(),
            )
            .await?;
            note = n;
            let entity = Collection::from_state(
                *collection_id,
                name.clone(),
                sort,
                Lifecycle::from_state(Generation::INITIAL, revision, at, at, None),
            );
            tx.save(
                "collection",
                &collection_id.to_string(),
                &entity,
                Some(&entity.sort_key().to_string()),
                None,
            )
            .await?;
            EventKind::CollectionCreated { entity }
        }
        OperationKind::PatchCollection {
            collection_id,
            generation,
            patch,
        } => {
            let mut entity = missing(
                tx.record::<Collection>("collection", &collection_id.to_string())
                    .await?,
            )?;
            entity.lifecycle().ensure_active(*generation)?;
            if entity.name() == patch.name() {
                return Ok((None, None));
            }
            entity.patch(*generation, patch, at, revision)?;
            tx.save(
                "collection",
                &collection_id.to_string(),
                &entity,
                Some(&entity.sort_key().to_string()),
                None,
            )
            .await?;
            EventKind::CollectionPatched { entity }
        }
        OperationKind::MoveCollection {
            collection_id,
            generation,
            before_id,
        } => {
            let mut entity = missing(
                tx.record::<Collection>("collection", &collection_id.to_string())
                    .await?,
            )?;
            entity.lifecycle().ensure_active(*generation)?;
            let (sort, n) = key(
                tx,
                "collection",
                None,
                before_id.map(|id| id.to_string()),
                &collection_id.to_string(),
            )
            .await?;
            note = n;
            if entity.sort_key() == &sort {
                return Ok((None, note));
            }
            entity.set_sort_key(*generation, sort, at, revision)?;
            tx.save(
                "collection",
                &collection_id.to_string(),
                &entity,
                Some(&entity.sort_key().to_string()),
                None,
            )
            .await?;
            EventKind::CollectionPatched { entity }
        }
        OperationKind::CreateTag { tag_id, name } => {
            ensure_new(tx, EntityId::Tag(*tag_id)).await?;
            if tx
                .record::<Tag>("tag", &tag_id.to_string())
                .await?
                .is_some()
            {
                return Ok((None, None));
            }
            let entity = Tag::from_state(
                *tag_id,
                name.clone(),
                name.normalized(),
                Lifecycle::from_state(Generation::INITIAL, revision, at, at, None),
            )?;
            tx.save("tag", &tag_id.to_string(), &entity, None, None)
                .await?;
            EventKind::TagCreated { entity }
        }
        OperationKind::PatchTag {
            tag_id,
            generation,
            patch,
        } => {
            let mut entity = missing(tx.record::<Tag>("tag", &tag_id.to_string()).await?)?;
            entity.lifecycle().ensure_active(*generation)?;
            if entity.name() == patch.name() {
                return Ok((None, None));
            }
            entity.patch(*generation, patch, at, revision)?;
            tx.save("tag", &tag_id.to_string(), &entity, None, None)
                .await?;
            EventKind::TagPatched { entity }
        }
        OperationKind::DeleteSticker {
            sticker_id,
            generation,
        } => {
            let id = EntityId::Sticker(*sticker_id);
            if let Some(mut entity) = tx
                .record::<Sticker>("sticker", &sticker_id.to_string())
                .await?
            {
                if !entity.delete(*generation, at, revision)? {
                    return Ok((None, None));
                }
                tx.save("sticker", &sticker_id.to_string(), &entity, None, None)
                    .await?;
            } else if !check_delete(tx, id, *generation).await? {
                return Ok((None, None));
            }
            let entity = Tombstone::new(id, *generation, revision, at);
            tx.save("tombstone", &tombstone_key(id), &entity, None, None)
                .await?;
            EventKind::StickerDeleted { entity }
        }
        OperationKind::DeleteCollection {
            collection_id,
            generation,
        } => {
            let id = EntityId::Collection(*collection_id);
            if let Some(mut entity) = tx
                .record::<Collection>("collection", &collection_id.to_string())
                .await?
            {
                if !entity.delete(*generation, at, revision)? {
                    return Ok((None, None));
                }
                tx.save(
                    "collection",
                    &collection_id.to_string(),
                    &entity,
                    Some(&entity.sort_key().to_string()),
                    None,
                )
                .await?;
            } else if !check_delete(tx, id, *generation).await? {
                return Ok((None, None));
            }
            let entity = Tombstone::new(id, *generation, revision, at);
            tx.save("tombstone", &tombstone_key(id), &entity, None, None)
                .await?;
            EventKind::CollectionDeleted { entity }
        }
        OperationKind::DeleteTag { tag_id, generation } => {
            let id = EntityId::Tag(*tag_id);
            if let Some(mut entity) = tx.record::<Tag>("tag", &tag_id.to_string()).await? {
                if !entity.delete(*generation, at, revision)? {
                    return Ok((None, None));
                }
                tx.save("tag", &tag_id.to_string(), &entity, None, None)
                    .await?;
            } else if !check_delete(tx, id, *generation).await? {
                return Ok((None, None));
            }
            let entity = Tombstone::new(id, *generation, revision, at);
            tx.save("tombstone", &tombstone_key(id), &entity, None, None)
                .await?;
            EventKind::TagDeleted { entity }
        }
        OperationKind::RestoreSticker {
            sticker_id,
            expected_deleted_revision,
            rebuild,
        } => {
            let tombstone = restore_tombstone(
                tx,
                EntityId::Sticker(*sticker_id),
                *expected_deleted_revision,
            )
            .await?;
            let mut entity = if let Some(entity) = tx
                .record::<Sticker>("sticker", &sticker_id.to_string())
                .await?
            {
                entity
            } else {
                let rebuild = missing(rebuild.as_ref())?;
                verified(tx, &rebuild.asset).await?;
                Sticker::from_state(
                    *sticker_id,
                    rebuild.title.clone(),
                    rebuild.original_name.clone(),
                    rebuild.note.clone(),
                    rebuild.starred,
                    Lifecycle::from_state(
                        tombstone.generation(),
                        tombstone.revision(),
                        at,
                        at,
                        Some(tombstone.deleted_at()),
                    ),
                )
            };
            let asset = missing(tx.asset(sticker_id.content_hash()).await?)?;
            entity.restore(*expected_deleted_revision, at, revision)?;
            tx.save("sticker", &sticker_id.to_string(), &entity, None, None)
                .await?;
            EventKind::StickerRestored { asset, entity }
        }
        OperationKind::RestoreCollection {
            collection_id,
            expected_deleted_revision,
            rebuild,
        } => {
            let tombstone = restore_tombstone(
                tx,
                EntityId::Collection(*collection_id),
                *expected_deleted_revision,
            )
            .await?;
            let mut entity = if let Some(entity) = tx
                .record::<Collection>("collection", &collection_id.to_string())
                .await?
            {
                entity
            } else {
                let rebuild = missing(rebuild.as_ref())?;
                Collection::from_state(
                    *collection_id,
                    rebuild.name.clone(),
                    SortKey::default(),
                    Lifecycle::from_state(
                        tombstone.generation(),
                        tombstone.revision(),
                        at,
                        at,
                        Some(tombstone.deleted_at()),
                    ),
                )
            };
            entity.restore(*expected_deleted_revision, at, revision)?;
            // Resolve fresh intent; old positions can now have other occupants.
            let (sort, n) = key(
                tx,
                "collection",
                None,
                rebuild
                    .as_ref()
                    .and_then(|r| r.before_id)
                    .map(|id| id.to_string()),
                &collection_id.to_string(),
            )
            .await?;
            note = n;
            entity.set_sort_key(entity.lifecycle().generation(), sort, at, revision)?;
            tx.save(
                "collection",
                &collection_id.to_string(),
                &entity,
                Some(&entity.sort_key().to_string()),
                None,
            )
            .await?;
            EventKind::CollectionRestored { entity }
        }
        OperationKind::RestoreTag {
            tag_id,
            expected_deleted_revision,
            rebuild,
        } => {
            let tombstone =
                restore_tombstone(tx, EntityId::Tag(*tag_id), *expected_deleted_revision).await?;
            let mut entity =
                if let Some(entity) = tx.record::<Tag>("tag", &tag_id.to_string()).await? {
                    entity
                } else {
                    let name = missing(rebuild.as_ref())?;
                    Tag::from_state(
                        *tag_id,
                        name.clone(),
                        name.normalized(),
                        Lifecycle::from_state(
                            tombstone.generation(),
                            tombstone.revision(),
                            at,
                            at,
                            Some(tombstone.deleted_at()),
                        ),
                    )?
                };
            entity.restore(*expected_deleted_revision, at, revision)?;
            tx.save("tag", &tag_id.to_string(), &entity, None, None)
                .await?;
            EventKind::TagRestored { entity }
        }
        OperationKind::SetStickerCollection {
            sticker_id,
            sticker_generation,
            collection,
        } => {
            let sticker = active_sticker(tx, *sticker_id, *sticker_generation).await?;
            let old = tx
                .record::<CollectionItem>("collection_item", &sticker_id.to_string())
                .await?;
            if let Some((id, generation)) = collection {
                let target = active_collection(tx, *id, *generation).await?;
                if old
                    .as_ref()
                    .is_some_and(|item| item.is_effective(&target, &sticker))
                {
                    return Ok((None, None));
                }
                let parent = id.to_string();
                let (sort, _) = key(
                    tx,
                    "collection_item",
                    Some(&parent),
                    None,
                    &sticker_id.to_string(),
                )
                .await?;
                let entity = CollectionItem::new(&target, &sticker, sort, true, revision, at)?;
                tx.save(
                    "collection_item",
                    &sticker_id.to_string(),
                    &entity,
                    Some(&entity.sort_key().to_string()),
                    Some(&parent),
                )
                .await?;
                EventKind::CollectionMembershipSet { entity }
            } else {
                let Some(old) = old else {
                    return Ok((None, None));
                };
                if !old.present() || old.sticker_generation() != *sticker_generation {
                    return Ok((None, None));
                }
                let entity = CollectionItem::from_state(
                    (old.collection_id(), old.collection_generation()),
                    (*sticker_id, *sticker_generation),
                    false,
                    old.sort_key().clone(),
                    revision,
                    at,
                );
                tx.save(
                    "collection_item",
                    &sticker_id.to_string(),
                    &entity,
                    Some(&entity.sort_key().to_string()),
                    Some(&entity.collection_id().to_string()),
                )
                .await?;
                EventKind::CollectionMembershipSet { entity }
            }
        }
        OperationKind::MoveCollectionItem {
            collection_id,
            sticker_id,
            collection_generation,
            sticker_generation,
            before_id,
        } => {
            let sticker = active_sticker(tx, *sticker_id, *sticker_generation).await?;
            let collection = active_collection(tx, *collection_id, *collection_generation).await?;
            let mut entity = missing(
                tx.record::<CollectionItem>("collection_item", &sticker_id.to_string())
                    .await?,
            )?;
            if !entity.is_effective(&collection, &sticker) {
                return reject(RejectionCode::InvalidOperation);
            }
            let parent = collection_id.to_string();
            let (sort, n) = key(
                tx,
                "collection_item",
                Some(&parent),
                before_id.map(|id| id.to_string()),
                &sticker_id.to_string(),
            )
            .await?;
            note = n;
            if entity.sort_key() == &sort {
                return Ok((None, note));
            }
            entity.move_to(&collection, &sticker, sort, revision, at)?;
            tx.save(
                "collection_item",
                &sticker_id.to_string(),
                &entity,
                Some(&entity.sort_key().to_string()),
                Some(&parent),
            )
            .await?;
            EventKind::CollectionItemMoved { entity }
        }
        OperationKind::SetTagMembership {
            sticker_id,
            tag_id,
            sticker_generation,
            tag_generation,
            present,
        } => {
            let sticker = active_sticker(tx, *sticker_id, *sticker_generation).await?;
            let tag = missing(tx.record::<Tag>("tag", &tag_id.to_string()).await?)?;
            tag.lifecycle().ensure_active(*tag_generation)?;
            let id = format!("{sticker_id}:{tag_id}");
            let old = tx.record::<StickerTag>("sticker_tag", &id).await?;
            if old.as_ref().is_some_and(|old| {
                old.present() == *present
                    && old.sticker_generation() == *sticker_generation
                    && old.tag_generation() == *tag_generation
            }) || (old.is_none() && !present)
            {
                return Ok((None, None));
            }
            let entity = StickerTag::new(&sticker, &tag, *present, revision, at)?;
            tx.save("sticker_tag", &id, &entity, None, None).await?;
            EventKind::TagMembershipSet { entity }
        }
    };
    Ok((Some(event), note))
}
async fn active_sticker(
    tx: &SyncTransaction,
    id: StickerId,
    generation: Generation,
) -> Result<Sticker> {
    let entity = missing(tx.record::<Sticker>("sticker", &id.to_string()).await?)?;
    entity.lifecycle().ensure_active(generation)?;
    Ok(entity)
}
async fn active_collection(
    tx: &SyncTransaction,
    id: CollectionId,
    generation: Generation,
) -> Result<Collection> {
    let entity = missing(
        tx.record::<Collection>("collection", &id.to_string())
            .await?,
    )?;
    entity.lifecycle().ensure_active(generation)?;
    Ok(entity)
}
async fn restore_tombstone(
    tx: &SyncTransaction,
    id: EntityId,
    expected: Revision,
) -> Result<Tombstone> {
    let entity = missing(
        tx.record::<Tombstone>("tombstone", &tombstone_key(id))
            .await?,
    )?;
    entity.ensure_restore(expected)?;
    Ok(entity)
}
async fn check_delete(tx: &SyncTransaction, id: EntityId, generation: Generation) -> Result<bool> {
    if let Some(old) = tx
        .record::<Tombstone>("tombstone", &tombstone_key(id))
        .await?
    {
        if generation != old.generation() {
            return reject(RejectionCode::StaleGeneration);
        }
        return Ok(false);
    }
    Ok(true)
}
