use crate::{CoreError, ErrorCode, Result};
use memedock_domain::{
    archive::ArchiveData,
    asset::Asset,
    collection::Collection,
    lifecycle::{EntityId, Lifecycle, Tombstone},
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    sync::{EventKind, SyncEvent},
    tag::Tag,
    version::{Generation, Revision, TimestampMs},
};
fn put<T>(values: &mut Vec<T>, value: T, same: impl Fn(&T, &T) -> bool) {
    if let Some(slot) = values.iter_mut().find(|v| same(v, &value)) {
        *slot = value;
    } else {
        values.push(value);
    }
}
pub(super) fn asset(data: &mut ArchiveData, value: Asset) {
    put(&mut data.assets, value, |a, b| a.hash() == b.hash());
}
pub(super) fn sticker(data: &mut ArchiveData, value: Sticker) {
    put(&mut data.stickers, value, |a, b| a.id() == b.id());
}
pub(super) fn collection(data: &mut ArchiveData, value: Collection) {
    put(&mut data.collections, value, |a, b| a.id() == b.id());
}
pub(super) fn tag(data: &mut ArchiveData, value: Tag) {
    put(&mut data.tags, value, |a, b| a.id() == b.id());
}
pub(super) fn membership(data: &mut ArchiveData, value: CollectionItem) {
    put(&mut data.collection_items, value, |a, b| {
        a.sticker_id() == b.sticker_id()
    });
}
pub(super) fn tagging(data: &mut ArchiveData, value: StickerTag) {
    put(&mut data.sticker_tags, value, |a, b| {
        a.sticker_id() == b.sticker_id() && a.tag_id() == b.tag_id()
    });
}
fn deleted(old: &Lifecycle, t: &Tombstone) -> Lifecycle {
    Lifecycle::from_state(
        t.generation(),
        t.revision(),
        old.created_at(),
        t.deleted_at(),
        Some(t.deleted_at()),
    )
    .with_purged_at(t.purged_at())
}
pub(super) fn tombstone(data: &mut ArchiveData, t: Tombstone) -> Result<()> {
    match t.entity() {
        EntityId::Sticker(id) => {
            if let Some(v) = data.stickers.iter_mut().find(|v| v.id() == id)
                && v.lifecycle().revision() <= t.revision()
            {
                *v = Sticker::from_state(
                    id,
                    if t.purged_at().is_some() {
                        String::new()
                    } else {
                        v.title().into()
                    },
                    if t.purged_at().is_some() {
                        String::new()
                    } else {
                        v.original_name().into()
                    },
                    if t.purged_at().is_some() {
                        String::new()
                    } else {
                        v.note().into()
                    },
                    t.purged_at().is_none() && v.starred(),
                    deleted(v.lifecycle(), &t),
                );
            }
        }
        EntityId::Collection(id) => {
            if let Some(v) = data.collections.iter_mut().find(|v| v.id() == id)
                && v.lifecycle().revision() <= t.revision()
            {
                *v = Collection::from_state(
                    id,
                    v.name().clone(),
                    v.sort_key().clone(),
                    deleted(v.lifecycle(), &t),
                );
            }
        }
        EntityId::Tag(id) => {
            if let Some(v) = data.tags.iter_mut().find(|v| v.id() == id)
                && v.lifecycle().revision() <= t.revision()
            {
                *v = Tag::from_state(
                    id,
                    v.name().clone(),
                    v.normalized_name(),
                    deleted(v.lifecycle(), &t),
                )?;
            }
        }
    }
    put(&mut data.tombstones, t, |a, b| a.entity() == b.entity());
    Ok(())
}
pub(super) fn event(data: &mut ArchiveData, event: &SyncEvent) -> Result<()> {
    let revision = Revision::from(event.seq());
    let at = event.created_at();
    match event.event() {
        EventKind::StickerCreated { asset: a, entity }
        | EventKind::StickerRestored { asset: a, entity } => {
            asset(data, a.clone());
            sticker(data, entity.clone());
        }
        EventKind::StickerPatched { entity } => sticker(data, entity.clone()),
        EventKind::CollectionCreated { entity }
        | EventKind::CollectionPatched { entity }
        | EventKind::CollectionRestored { entity } => collection(data, entity.clone()),
        EventKind::TagCreated { entity }
        | EventKind::TagPatched { entity }
        | EventKind::TagRestored { entity } => tag(data, entity.clone()),
        EventKind::StickerDeleted { entity }
        | EventKind::CollectionDeleted { entity }
        | EventKind::TagDeleted { entity } => tombstone(data, entity.clone())?,
        EventKind::EntityPurged { entity } => tombstone(data, entity.clone())?,
        EventKind::CollectionMembershipSet { entity }
        | EventKind::CollectionItemMoved { entity } => membership(data, entity.clone()),
        EventKind::TagMembershipSet { entity } => tagging(data, entity.clone()),
        EventKind::CollectionsReordered { items } => {
            for item in items {
                let value = data
                    .collections
                    .iter_mut()
                    .find(|v| v.id() == item.collection_id)
                    .ok_or_else(|| {
                        CoreError::new(ErrorCode::CorruptData, "reordered collection missing")
                    })?;
                value.set_sort_key(
                    value.lifecycle().generation(),
                    item.sort_key.clone(),
                    at,
                    revision,
                )?;
            }
        }
        EventKind::CollectionItemsReordered {
            collection_id,
            items,
        } => {
            for item in items {
                let value = data
                    .collection_items
                    .iter_mut()
                    .find(|v| {
                        v.collection_id() == *collection_id && v.sticker_id() == item.sticker_id
                    })
                    .ok_or_else(|| {
                        CoreError::new(ErrorCode::CorruptData, "reordered item missing")
                    })?;
                *value = CollectionItem::from_state(
                    (value.collection_id(), value.collection_generation()),
                    (value.sticker_id(), value.sticker_generation()),
                    value.present(),
                    item.sort_key.clone(),
                    revision,
                    at,
                );
            }
        }
    }
    Ok(())
}
pub(super) fn merge(view: &mut ArchiveData, remote: &ArchiveData) -> Result<()> {
    for a in &remote.assets {
        asset(view, a.clone());
    }
    for v in &remote.stickers {
        sticker(view, v.clone());
    }
    for v in &remote.collections {
        collection(view, v.clone());
    }
    for v in &remote.tags {
        tag(view, v.clone());
    }
    for v in &remote.collection_items {
        membership(view, v.clone());
    }
    for v in &remote.sticker_tags {
        tagging(view, v.clone());
    }
    for v in &remote.tombstones {
        tombstone(view, v.clone())?;
    }
    clean(view);
    Ok(())
}
pub(super) fn clean(data: &mut ArchiveData) {
    data.tombstones.retain(|t| {
        let life = match t.entity() {
            EntityId::Sticker(id) => data
                .stickers
                .iter()
                .find(|v| v.id() == id)
                .map(|v| v.lifecycle()),
            EntityId::Collection(id) => data
                .collections
                .iter()
                .find(|v| v.id() == id)
                .map(|v| v.lifecycle()),
            EntityId::Tag(id) => data
                .tags
                .iter()
                .find(|v| v.id() == id)
                .map(|v| v.lifecycle()),
        };
        life.is_none_or(|life| {
            t.generation() < life.generation()
                || (t.generation() == life.generation()
                    && !life.is_active()
                    && t.revision() <= life.revision())
        })
    });
    data.collection_items.retain(|r| {
        data.stickers.iter().any(|v| {
            v.id() == r.sticker_id() && v.lifecycle().generation() >= r.sticker_generation()
        }) && data.collections.iter().any(|v| {
            v.id() == r.collection_id() && v.lifecycle().generation() >= r.collection_generation()
        })
    });
    data.sticker_tags.retain(|r| {
        data.stickers.iter().any(|v| {
            v.id() == r.sticker_id() && v.lifecycle().generation() >= r.sticker_generation()
        }) && data
            .tags
            .iter()
            .any(|v| v.id() == r.tag_id() && v.lifecycle().generation() >= r.tag_generation())
    });
}
pub(super) fn initial(at: TimestampMs) -> Lifecycle {
    Lifecycle::from_state(Generation::INITIAL, Revision::LOCAL, at, at, None)
}
