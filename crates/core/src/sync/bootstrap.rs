use memedock_domain::{
    archive::ArchiveData, change::OperationKind, lifecycle::EntityId, version::Generation,
};

pub(super) fn operations(local: &ArchiveData, remote: &ArchiveData) -> Vec<OperationKind> {
    let known = |id: EntityId| remote.tombstones.iter().any(|v| v.entity() == id);
    let new_sticker =
        |id| !remote.stickers.iter().any(|v| v.id() == id) && !known(EntityId::Sticker(id));
    let new_collection =
        |id| !remote.collections.iter().any(|v| v.id() == id) && !known(EntityId::Collection(id));
    let new_tag = |id| !remote.tags.iter().any(|v| v.id() == id) && !known(EntityId::Tag(id));
    let mut ops = Vec::new();
    let mut collections: Vec<_> = local
        .collections
        .iter()
        .filter(|v| v.lifecycle().is_active() && new_collection(v.id()))
        .collect();
    collections.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
    for v in collections {
        ops.push(OperationKind::CreateCollection {
            collection_id: v.id(),
            name: v.name().clone(),
            before_id: None,
        });
    }
    for v in &local.tags {
        if v.lifecycle().is_active() && new_tag(v.id()) {
            ops.push(OperationKind::CreateTag {
                tag_id: v.id(),
                name: v.name().clone(),
            });
        }
    }
    for v in &local.stickers {
        if v.lifecycle().is_active()
            && new_sticker(v.id())
            && let Some(a) = local.assets.iter().find(|a| a.sticker_id() == v.id())
        {
            ops.push(OperationKind::CreateSticker {
                asset: a.clone(),
                title: v.title().into(),
                original_name: v.original_name().into(),
                note: v.note().into(),
                starred: v.starred(),
            });
        }
    }
    let mut items: Vec<_> = local
        .collection_items
        .iter()
        .filter(|r| r.present() && new_sticker(r.sticker_id()))
        .collect();
    items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
    for r in items {
        if let Some(c) = local
            .collections
            .iter()
            .find(|c| c.id() == r.collection_id())
            && let Some(s) = local.stickers.iter().find(|s| s.id() == r.sticker_id())
            && r.is_effective(c, s)
        {
            let generation = if new_collection(c.id()) {
                Some(Generation::INITIAL)
            } else {
                remote
                    .collections
                    .iter()
                    .find(|v| v.id() == c.id() && v.lifecycle().is_active())
                    .map(|v| v.lifecycle().generation())
            };
            if let Some(generation) = generation {
                ops.push(OperationKind::SetStickerCollection {
                    sticker_id: s.id(),
                    sticker_generation: Generation::INITIAL,
                    collection: Some((c.id(), generation)),
                });
            }
        }
    }
    for r in &local.sticker_tags {
        if !r.present() {
            continue;
        }
        let Some(s) = local.stickers.iter().find(|v| v.id() == r.sticker_id()) else {
            continue;
        };
        let Some(t) = local.tags.iter().find(|v| v.id() == r.tag_id()) else {
            continue;
        };
        if !r.is_effective(s, t) {
            continue;
        }
        if remote
            .sticker_tags
            .iter()
            .any(|v| v.sticker_id() == s.id() && v.tag_id() == t.id())
        {
            continue;
        }
        let sg = if new_sticker(s.id()) {
            Some(Generation::INITIAL)
        } else {
            remote
                .stickers
                .iter()
                .find(|v| v.id() == s.id() && v.lifecycle().is_active())
                .map(|v| v.lifecycle().generation())
        };
        let tg = if new_tag(t.id()) {
            Some(Generation::INITIAL)
        } else {
            remote
                .tags
                .iter()
                .find(|v| v.id() == t.id() && v.lifecycle().is_active())
                .map(|v| v.lifecycle().generation())
        };
        if let (Some(sg), Some(tg)) = (sg, tg) {
            ops.push(OperationKind::SetTagMembership {
                sticker_id: s.id(),
                tag_id: t.id(),
                sticker_generation: sg,
                tag_generation: tg,
                present: true,
            });
        }
    }
    ops
}
/// Rebase only never-sent work captured during the first connection. The frozen
/// network envelope is persisted before its first request and is never rewritten.
pub(super) fn rebase(
    mut op: OperationKind,
    baseline: &ArchiveData,
    canonical: &ArchiveData,
) -> OperationKind {
    let sg = |id, observed| {
        canonical
            .stickers
            .iter()
            .find(|v| v.id() == id)
            .filter(|_| {
                baseline
                    .stickers
                    .iter()
                    .any(|v| v.id() == id && v.lifecycle().generation() == observed)
            })
            .map_or(observed, |v| v.lifecycle().generation())
    };
    let cg = |id, observed| {
        canonical
            .collections
            .iter()
            .find(|v| v.id() == id)
            .filter(|_| {
                baseline
                    .collections
                    .iter()
                    .any(|v| v.id() == id && v.lifecycle().generation() == observed)
            })
            .map_or(observed, |v| v.lifecycle().generation())
    };
    let tg = |id, observed| {
        canonical
            .tags
            .iter()
            .find(|v| v.id() == id)
            .filter(|_| {
                baseline
                    .tags
                    .iter()
                    .any(|v| v.id() == id && v.lifecycle().generation() == observed)
            })
            .map_or(observed, |v| v.lifecycle().generation())
    };
    match &mut op {
        OperationKind::PatchSticker {
            sticker_id,
            generation,
            ..
        }
        | OperationKind::DeleteSticker {
            sticker_id,
            generation,
        } => *generation = sg(*sticker_id, *generation),
        OperationKind::PatchCollection {
            collection_id,
            generation,
            ..
        }
        | OperationKind::DeleteCollection {
            collection_id,
            generation,
        }
        | OperationKind::MoveCollection {
            collection_id,
            generation,
            ..
        } => *generation = cg(*collection_id, *generation),
        OperationKind::PatchTag {
            tag_id, generation, ..
        }
        | OperationKind::DeleteTag { tag_id, generation } => *generation = tg(*tag_id, *generation),
        OperationKind::SetStickerCollection {
            sticker_id,
            sticker_generation,
            collection,
        } => {
            *sticker_generation = sg(*sticker_id, *sticker_generation);
            if let Some((id, g)) = collection {
                *g = cg(*id, *g);
            }
        }
        OperationKind::MoveCollectionItem {
            collection_id,
            sticker_id,
            collection_generation,
            sticker_generation,
            ..
        } => {
            *collection_generation = cg(*collection_id, *collection_generation);
            *sticker_generation = sg(*sticker_id, *sticker_generation);
        }
        OperationKind::SetTagMembership {
            sticker_id,
            tag_id,
            sticker_generation,
            tag_generation,
            ..
        } => {
            *sticker_generation = sg(*sticker_id, *sticker_generation);
            *tag_generation = tg(*tag_id, *tag_generation);
        }
        _ => {}
    }
    op
}
