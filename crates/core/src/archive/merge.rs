use crate::Result;
use memedock_domain::{
    archive::{ArchiveData, RestoreSummary, entity_key},
    lifecycle::EntityId,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn merge(
    current: &ArchiveData,
    incoming: &ArchiveData,
) -> Result<(ArchiveData, RestoreSummary)> {
    let mut merged = current.clone();
    let mut summary = incoming.summary()?;
    let blocked: BTreeSet<_> = current
        .tombstones
        .iter()
        .map(|v| entity_key(v.entity()))
        .collect();
    let old_stickers: BTreeSet<_> = current.stickers.iter().map(|v| v.id()).collect();
    let old_collections: BTreeSet<_> = current.collections.iter().map(|v| v.id()).collect();
    let old_tags: BTreeSet<_> = current.tags.iter().map(|v| v.id()).collect();
    macro_rules! append {
        ($field:ident, $old:ident, $kind:ident) => {
            for value in &incoming.$field {
                if $old.contains(&value.id())
                    || blocked.contains(&entity_key(EntityId::$kind(value.id())))
                {
                    summary.preserved += 1;
                } else {
                    merged.$field.push(value.clone());
                    summary.added += 1;
                }
            }
        };
    }
    append!(stickers, old_stickers, Sticker);
    append!(collections, old_collections, Collection);
    append!(tags, old_tags, Tag);
    let wanted: BTreeSet<_> = merged
        .stickers
        .iter()
        .map(|v| v.id().content_hash())
        .collect();
    let known: BTreeSet<_> = merged.assets.iter().map(|v| v.hash()).collect();
    for asset in &incoming.assets {
        if wanted.contains(&asset.hash()) && !known.contains(&asset.hash()) {
            merged.assets.push(asset.clone());
        }
    }
    let stickers: BTreeMap<_, _> = merged.stickers.iter().map(|v| (v.id(), v)).collect();
    let collections: BTreeMap<_, _> = merged.collections.iter().map(|v| (v.id(), v)).collect();
    let tags: BTreeMap<_, _> = merged.tags.iter().map(|v| (v.id(), v)).collect();
    let incoming_stickers: BTreeMap<_, _> = incoming.stickers.iter().map(|v| (v.id(), v)).collect();
    let incoming_collections: BTreeMap<_, _> =
        incoming.collections.iter().map(|v| (v.id(), v)).collect();
    let incoming_tags: BTreeMap<_, _> = incoming.tags.iter().map(|v| (v.id(), v)).collect();
    // Existing stickers keep their local assignment, including explicit unassignment.
    for value in &incoming.collection_items {
        let endpoints = collections
            .get(&value.collection_id())
            .zip(stickers.get(&value.sticker_id()));
        if !old_stickers.contains(&value.sticker_id())
            && endpoints.is_some_and(|(collection, sticker)| {
                value.collection_generation() <= collection.lifecycle().generation()
                    && value.sticker_generation() <= sticker.lifecycle().generation()
                    && (!value.is_effective(collection, sticker)
                        || incoming_collections
                            .get(&value.collection_id())
                            .zip(incoming_stickers.get(&value.sticker_id()))
                            .is_some_and(|(c, s)| value.is_effective(c, s)))
            })
        {
            merged.collection_items.push(value.clone());
        } else {
            summary.skipped_relations += 1;
        }
    }
    let pairs: BTreeSet<_> = current
        .sticker_tags
        .iter()
        .map(|v| (v.sticker_id(), v.tag_id()))
        .collect();
    for value in &incoming.sticker_tags {
        let endpoints = stickers
            .get(&value.sticker_id())
            .zip(tags.get(&value.tag_id()));
        let new_endpoint =
            !old_stickers.contains(&value.sticker_id()) || !old_tags.contains(&value.tag_id());
        if !pairs.contains(&(value.sticker_id(), value.tag_id()))
            && new_endpoint
            && endpoints.is_some_and(|(sticker, tag)| {
                value.sticker_generation() <= sticker.lifecycle().generation()
                    && value.tag_generation() <= tag.lifecycle().generation()
                    && (!value.is_effective(sticker, tag)
                        || incoming_stickers
                            .get(&value.sticker_id())
                            .zip(incoming_tags.get(&value.tag_id()))
                            .is_some_and(|(s, t)| value.is_effective(s, t)))
            })
        {
            merged.sticker_tags.push(value.clone());
        } else {
            summary.skipped_relations += 1;
        }
    }
    let known: BTreeSet<_> = current
        .tombstones
        .iter()
        .map(|v| entity_key(v.entity()))
        .collect();
    for value in &incoming.tombstones {
        let exists = match value.entity() {
            EntityId::Sticker(v) => old_stickers.contains(&v),
            EntityId::Collection(v) => old_collections.contains(&v),
            EntityId::Tag(v) => old_tags.contains(&v),
        };
        if !exists && !known.contains(&entity_key(value.entity())) {
            merged.tombstones.push(value.clone());
        }
    }
    // Empty-library restore preserves historical relations for later explicit recovery.
    if current.stickers.is_empty()
        && current.collections.is_empty()
        && current.tags.is_empty()
        && current.tombstones.is_empty()
    {
        merged = incoming.clone();
        summary.skipped_relations = 0;
    }
    merged.validate()?;
    Ok((merged, summary))
}
