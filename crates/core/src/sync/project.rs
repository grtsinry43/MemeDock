use super::catalog;
use crate::Result;
use memedock_domain::{
    archive::ArchiveData,
    change::OperationKind,
    collection::Collection,
    lifecycle::{EntityId, Lifecycle, Tombstone},
    ordering::SortKey,
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    tag::Tag,
    version::{Revision, TimestampMs},
};

fn deleted_lifecycle(t: &Tombstone) -> Lifecycle {
    Lifecycle::from_state(
        t.generation(),
        t.revision(),
        t.deleted_at(),
        t.deleted_at(),
        Some(t.deleted_at()),
    )
    .with_purged_at(t.purged_at())
}

pub(super) fn overlay(data: &mut ArchiveData, op: &OperationKind, at: TimestampMs) -> Result<()> {
    match op {
        OperationKind::PurgeEntity {
            entity, generation, ..
        } => {
            let life = match entity {
                EntityId::Sticker(id) => data
                    .stickers
                    .iter()
                    .find(|v| v.id() == *id)
                    .map(|v| v.lifecycle()),
                EntityId::Collection(id) => data
                    .collections
                    .iter()
                    .find(|v| v.id() == *id)
                    .map(|v| v.lifecycle()),
                EntityId::Tag(id) => data
                    .tags
                    .iter()
                    .find(|v| v.id() == *id)
                    .map(|v| v.lifecycle()),
            };
            if let Some(life) = life {
                let mut life = life.clone();
                life.purge(*generation, life.revision(), at, life.revision())?;
                catalog::tombstone(
                    data,
                    Tombstone::new(
                        *entity,
                        *generation,
                        life.revision(),
                        life.deleted_at()
                            .ok_or(memedock_domain::error::DomainError::InvalidTransition)?,
                    )
                    .with_purged_at(Some(at)),
                )?;
            }
        }
        OperationKind::ReimportSticker {
            generation,
            rebuild,
        } => {
            let id = rebuild.asset.sticker_id();
            if let Some(old) = data.stickers.iter().find(|v| v.id() == id) {
                if old.lifecycle().generation() != *generation {
                    return Err(memedock_domain::error::DomainError::StaleGeneration.into());
                }
                let life = old.lifecycle().reimport(at, old.lifecycle().revision())?;
                catalog::asset(data, rebuild.asset.clone());
                catalog::sticker(
                    data,
                    Sticker::from_state(
                        id,
                        rebuild.title.clone(),
                        rebuild.original_name.clone(),
                        rebuild.note.clone(),
                        rebuild.starred,
                        life,
                    ),
                );
            }
        }
        OperationKind::CreateSticker {
            asset,
            title,
            original_name,
            note,
            starred,
        } => {
            if data.stickers.iter().any(|v| v.id() == asset.sticker_id()) {
                return Ok(());
            }
            catalog::asset(data, asset.clone());
            catalog::sticker(
                data,
                Sticker::from_state(
                    asset.sticker_id(),
                    title.clone(),
                    original_name.clone(),
                    note.clone(),
                    *starred,
                    catalog::initial(at),
                ),
            );
        }
        OperationKind::CreateCollection {
            collection_id,
            name,
            before_id,
        } => {
            if data.collections.iter().any(|v| v.id() == *collection_id) {
                return Ok(());
            }
            let sort = collection_key(data, *before_id, Some(*collection_id))?;
            catalog::collection(
                data,
                Collection::new(*collection_id, name.clone(), sort, at),
            );
        }
        OperationKind::CreateTag { tag_id, name } => {
            if !data.tags.iter().any(|v| v.id() == *tag_id) {
                catalog::tag(data, Tag::new(*tag_id, name.clone(), at));
            }
        }
        OperationKind::PatchSticker {
            sticker_id,
            generation,
            patch,
        } => {
            if let Some(v) = data.stickers.iter_mut().find(|v| v.id() == *sticker_id) {
                v.patch(*generation, patch, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::PatchCollection {
            collection_id,
            generation,
            patch,
        } => {
            if let Some(v) = data
                .collections
                .iter_mut()
                .find(|v| v.id() == *collection_id)
            {
                v.patch(*generation, patch, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::PatchTag {
            tag_id,
            generation,
            patch,
        } => {
            if let Some(v) = data.tags.iter_mut().find(|v| v.id() == *tag_id) {
                v.patch(*generation, patch, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::DeleteSticker {
            sticker_id,
            generation,
        } => {
            if let Some(v) = data.stickers.iter_mut().find(|v| v.id() == *sticker_id) {
                v.delete(*generation, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::DeleteCollection {
            collection_id,
            generation,
        } => {
            if let Some(v) = data
                .collections
                .iter_mut()
                .find(|v| v.id() == *collection_id)
            {
                v.delete(*generation, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::DeleteTag { tag_id, generation } => {
            if let Some(v) = data.tags.iter_mut().find(|v| v.id() == *tag_id) {
                v.delete(*generation, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::RestoreSticker {
            sticker_id,
            expected_deleted_revision,
            rebuild,
        } => {
            if !data.stickers.iter().any(|v| v.id() == *sticker_id)
                && let Some(rebuild) = rebuild
                && let Some(t) = data
                    .tombstones
                    .iter()
                    .find(|t| t.entity() == EntityId::Sticker(*sticker_id))
            {
                let life = deleted_lifecycle(t);
                catalog::asset(data, rebuild.asset.clone());
                catalog::sticker(
                    data,
                    Sticker::from_state(
                        *sticker_id,
                        rebuild.title.clone(),
                        rebuild.original_name.clone(),
                        rebuild.note.clone(),
                        rebuild.starred,
                        life,
                    ),
                );
            }
            if let Some(v) = data.stickers.iter_mut().find(|v| v.id() == *sticker_id) {
                v.restore(*expected_deleted_revision, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::RestoreCollection {
            collection_id,
            expected_deleted_revision,
            rebuild,
        } => {
            if !data.collections.iter().any(|v| v.id() == *collection_id)
                && let Some(rebuild) = rebuild
                && let Some(t) = data
                    .tombstones
                    .iter()
                    .find(|t| t.entity() == EntityId::Collection(*collection_id))
            {
                let life = deleted_lifecycle(t);
                let key = collection_key(data, rebuild.before_id, Some(*collection_id))?;
                catalog::collection(
                    data,
                    Collection::from_state(*collection_id, rebuild.name.clone(), key, life),
                );
            }
            if let Some(v) = data
                .collections
                .iter_mut()
                .find(|v| v.id() == *collection_id)
            {
                v.restore(*expected_deleted_revision, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::RestoreTag {
            tag_id,
            expected_deleted_revision,
            rebuild,
        } => {
            if !data.tags.iter().any(|v| v.id() == *tag_id)
                && let Some(name) = rebuild
                && let Some(t) = data
                    .tombstones
                    .iter()
                    .find(|t| t.entity() == EntityId::Tag(*tag_id))
            {
                let life = deleted_lifecycle(t);
                let tag = Tag::new(*tag_id, name.clone(), life.created_at());
                catalog::tag(
                    data,
                    Tag::from_state(*tag_id, name.clone(), tag.normalized_name(), life)?,
                );
            }
            if let Some(v) = data.tags.iter_mut().find(|v| v.id() == *tag_id) {
                v.restore(*expected_deleted_revision, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::MoveCollection {
            collection_id,
            generation,
            before_id,
        } => {
            let key = collection_key(data, *before_id, Some(*collection_id))?;
            if let Some(v) = data
                .collections
                .iter_mut()
                .find(|v| v.id() == *collection_id)
            {
                v.set_sort_key(*generation, key, at, v.lifecycle().revision())?;
            }
        }
        OperationKind::SetStickerCollection {
            sticker_id,
            sticker_generation,
            collection,
        } => {
            let Some(s) = data.stickers.iter().find(|v| v.id() == *sticker_id) else {
                return Ok(());
            };
            s.lifecycle().ensure_active(*sticker_generation)?;
            if let Some((id, g)) = collection {
                let Some(c) = data.collections.iter().find(|v| v.id() == *id) else {
                    return Ok(());
                };
                c.lifecycle().ensure_active(*g)?;
                let last = data
                    .collection_items
                    .iter()
                    .filter(|r| {
                        r.collection_id() == *id && r.present() && r.sticker_id() != *sticker_id
                    })
                    .max_by(|a, b| a.sort_key().cmp(b.sort_key()));
                let sort = SortKey::between(last.map(|r| r.sort_key()), None)?;
                let item = CollectionItem::new(c, s, sort, true, Revision::LOCAL, at)?;
                catalog::membership(data, item);
            } else if let Some(v) = data
                .collection_items
                .iter_mut()
                .find(|v| v.sticker_id() == *sticker_id)
            {
                *v = CollectionItem::from_state(
                    (v.collection_id(), v.collection_generation()),
                    (v.sticker_id(), v.sticker_generation()),
                    false,
                    v.sort_key().clone(),
                    v.revision(),
                    at,
                );
            }
        }
        OperationKind::SetTagMembership {
            sticker_id,
            tag_id,
            sticker_generation,
            tag_generation,
            present,
        } => {
            let Some(s) = data.stickers.iter().find(|v| v.id() == *sticker_id) else {
                return Ok(());
            };
            let Some(t) = data.tags.iter().find(|v| v.id() == *tag_id) else {
                return Ok(());
            };
            s.lifecycle().ensure_active(*sticker_generation)?;
            t.lifecycle().ensure_active(*tag_generation)?;
            let r = StickerTag::new(s, t, *present, Revision::LOCAL, at)?;
            catalog::tagging(data, r);
        }
        OperationKind::MoveCollectionItem {
            collection_id,
            sticker_id,
            collection_generation,
            sticker_generation,
            before_id,
        } => {
            let Some(c) = data.collections.iter().find(|v| v.id() == *collection_id) else {
                return Ok(());
            };
            let Some(s) = data.stickers.iter().find(|v| v.id() == *sticker_id) else {
                return Ok(());
            };
            c.lifecycle().ensure_active(*collection_generation)?;
            s.lifecycle().ensure_active(*sticker_generation)?;
            let mut items: Vec<_> = data
                .collection_items
                .iter()
                .filter(|v| {
                    v.collection_id() == *collection_id
                        && v.present()
                        && v.sticker_id() != *sticker_id
                })
                .collect();
            items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
            let right = before_id.and_then(|id| items.iter().position(|v| v.sticker_id() == id));
            let left = right.map_or_else(
                || items.last().copied(),
                |n| n.checked_sub(1).map(|n| items[n]),
            );
            let sort = SortKey::between(
                left.map(|r| r.sort_key()),
                right.map(|n| items[n].sort_key()),
            )?;
            if let Some(v) = data
                .collection_items
                .iter_mut()
                .find(|v| v.sticker_id() == *sticker_id)
            {
                v.move_to(c, s, sort, v.revision(), at)?;
            }
        }
        OperationKind::ImportArchive { state, .. } => {
            catalog::merge(data, state)?;
        }
    }
    Ok(())
}
fn collection_key(
    data: &ArchiveData,
    before: Option<memedock_domain::identity::CollectionId>,
    exclude: Option<memedock_domain::identity::CollectionId>,
) -> Result<SortKey> {
    let mut items: Vec<_> = data
        .collections
        .iter()
        .filter(|v| v.lifecycle().is_active() && Some(v.id()) != exclude)
        .collect();
    items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
    let right = before.and_then(|id| items.iter().position(|v| v.id() == id));
    let left = right.map_or_else(
        || items.last().copied(),
        |n| n.checked_sub(1).map(|n| items[n]),
    );
    Ok(SortKey::between(
        left.map(|v| v.sort_key()),
        right.map(|n| items[n].sort_key()),
    )?)
}
