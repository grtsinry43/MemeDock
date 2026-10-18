//! An explicit backup restore is a new edit, unlike the remote-first initial merge.
use super::{bootstrap, project};
use crate::Result;
use memedock_domain::{
    archive::ArchiveData,
    change::{
        CollectionRebuild, FieldPatch, NamePatch, OperationKind, StickerPatch, StickerRebuild,
    },
    lifecycle::EntityId,
    version::TimestampMs,
};

pub(super) fn operations(
    desired: &ArchiveData,
    remote: &ArchiveData,
    at: TimestampMs,
) -> Result<Vec<OperationKind>> {
    let mut desired = desired.clone();
    for t in remote.tombstones.iter().filter(|t| t.purged_at().is_some()) {
        super::catalog::tombstone(&mut desired, t.clone())?;
    }
    for v in remote.stickers.iter().filter(|v| v.lifecycle().is_purged()) {
        if let Some(slot) = desired.stickers.iter_mut().find(|s| s.id() == v.id()) {
            *slot = v.clone();
        }
    }
    for v in remote
        .collections
        .iter()
        .filter(|v| v.lifecycle().is_purged())
    {
        if let Some(slot) = desired.collections.iter_mut().find(|s| s.id() == v.id()) {
            *slot = v.clone();
        }
    }
    for v in remote.tags.iter().filter(|v| v.lifecycle().is_purged()) {
        if let Some(slot) = desired.tags.iter_mut().find(|s| s.id() == v.id()) {
            *slot = v.clone();
        }
    }
    let desired = &desired;
    let mut current = remote.clone();
    let mut ops = Vec::new();
    // Creates precede references. Existing records are handled separately below.
    for op in bootstrap::operations(desired, remote) {
        if matches!(
            op,
            OperationKind::CreateSticker { .. }
                | OperationKind::CreateCollection { .. }
                | OperationKind::CreateTag { .. }
        ) {
            append(&mut current, &mut ops, op, at)?;
        }
    }
    // A compacted remote deletion may retain only a tombstone. Rebuild metadata
    // from the verified archive, while retaining the server's deletion guard.
    for tombstone in &remote.tombstones {
        if tombstone.purged_at().is_some() {
            continue;
        }
        let op = match tombstone.entity() {
            EntityId::Sticker(id) if !current.stickers.iter().any(|v| v.id() == id) => desired
                .stickers
                .iter()
                .find(|v| v.id() == id && v.lifecycle().is_active())
                .and_then(|v| {
                    desired
                        .assets
                        .iter()
                        .find(|a| a.sticker_id() == id)
                        .map(|asset| OperationKind::RestoreSticker {
                            sticker_id: id,
                            expected_deleted_revision: tombstone.revision(),
                            rebuild: Some(StickerRebuild {
                                asset: asset.clone(),
                                title: v.title().into(),
                                original_name: v.original_name().into(),
                                note: v.note().into(),
                                starred: v.starred(),
                            }),
                        })
                }),
            EntityId::Collection(id) if !current.collections.iter().any(|v| v.id() == id) => {
                desired
                    .collections
                    .iter()
                    .find(|v| v.id() == id && v.lifecycle().is_active())
                    .map(|v| OperationKind::RestoreCollection {
                        collection_id: id,
                        expected_deleted_revision: tombstone.revision(),
                        rebuild: Some(CollectionRebuild {
                            name: v.name().clone(),
                            before_id: None,
                        }),
                    })
            }
            EntityId::Tag(id) if !current.tags.iter().any(|v| v.id() == id) => desired
                .tags
                .iter()
                .find(|v| v.id() == id && v.lifecycle().is_active())
                .map(|v| OperationKind::RestoreTag {
                    tag_id: id,
                    expected_deleted_revision: tombstone.revision(),
                    rebuild: Some(v.name().clone()),
                }),
            _ => None,
        };
        if let Some(op) = op {
            append(&mut current, &mut ops, op, at)?;
        }
    }
    for wanted in &desired.stickers {
        if remote
            .stickers
            .iter()
            .any(|v| v.id() == wanted.id() && v.lifecycle().is_purged())
        {
            continue;
        }
        let Some(old) = current
            .stickers
            .iter()
            .find(|v| v.id() == wanted.id())
            .cloned()
        else {
            continue;
        };
        if !wanted.lifecycle().is_active() {
            if old.lifecycle().is_active() {
                append(
                    &mut current,
                    &mut ops,
                    OperationKind::DeleteSticker {
                        sticker_id: old.id(),
                        generation: old.lifecycle().generation(),
                    },
                    at,
                )?;
            }
            continue;
        }
        if !old.lifecycle().is_active() {
            append(
                &mut current,
                &mut ops,
                OperationKind::RestoreSticker {
                    sticker_id: old.id(),
                    expected_deleted_revision: old.lifecycle().revision(),
                    rebuild: None,
                },
                at,
            )?;
        }
        let generation = if old.lifecycle().is_active() {
            old.lifecycle().generation()
        } else {
            old.lifecycle().generation().checked_next()?
        };
        if wanted.title() != old.title()
            || wanted.note() != old.note()
            || wanted.starred() != old.starred()
        {
            append(
                &mut current,
                &mut ops,
                OperationKind::PatchSticker {
                    sticker_id: old.id(),
                    generation,
                    patch: StickerPatch::new(
                        FieldPatch::Set(wanted.title().into()),
                        FieldPatch::Set(wanted.note().into()),
                        FieldPatch::Set(wanted.starred()),
                    )?,
                },
                at,
            )?;
        }
    }
    for wanted in &desired.collections {
        if remote
            .collections
            .iter()
            .any(|v| v.id() == wanted.id() && v.lifecycle().is_purged())
        {
            continue;
        }
        let Some(old) = current
            .collections
            .iter()
            .find(|v| v.id() == wanted.id())
            .cloned()
        else {
            continue;
        };
        if !wanted.lifecycle().is_active() {
            if old.lifecycle().is_active() {
                append(
                    &mut current,
                    &mut ops,
                    OperationKind::DeleteCollection {
                        collection_id: old.id(),
                        generation: old.lifecycle().generation(),
                    },
                    at,
                )?;
            }
            continue;
        }
        if !old.lifecycle().is_active() {
            append(
                &mut current,
                &mut ops,
                OperationKind::RestoreCollection {
                    collection_id: old.id(),
                    expected_deleted_revision: old.lifecycle().revision(),
                    rebuild: None,
                },
                at,
            )?;
        }
        let generation = if old.lifecycle().is_active() {
            old.lifecycle().generation()
        } else {
            old.lifecycle().generation().checked_next()?
        };
        if wanted.name() != old.name() {
            append(
                &mut current,
                &mut ops,
                OperationKind::PatchCollection {
                    collection_id: old.id(),
                    generation,
                    patch: NamePatch::new(wanted.name().clone()),
                },
                at,
            )?;
        }
    }
    for wanted in &desired.tags {
        if remote
            .tags
            .iter()
            .any(|v| v.id() == wanted.id() && v.lifecycle().is_purged())
        {
            continue;
        }
        let Some(old) = current.tags.iter().find(|v| v.id() == wanted.id()).cloned() else {
            continue;
        };
        if !wanted.lifecycle().is_active() {
            if old.lifecycle().is_active() {
                append(
                    &mut current,
                    &mut ops,
                    OperationKind::DeleteTag {
                        tag_id: old.id(),
                        generation: old.lifecycle().generation(),
                    },
                    at,
                )?;
            }
            continue;
        }
        if !old.lifecycle().is_active() {
            append(
                &mut current,
                &mut ops,
                OperationKind::RestoreTag {
                    tag_id: old.id(),
                    expected_deleted_revision: old.lifecycle().revision(),
                    rebuild: None,
                },
                at,
            )?;
        }
        let generation = if old.lifecycle().is_active() {
            old.lifecycle().generation()
        } else {
            old.lifecycle().generation().checked_next()?
        };
        if wanted.name() != old.name() {
            append(
                &mut current,
                &mut ops,
                OperationKind::PatchTag {
                    tag_id: old.id(),
                    generation,
                    patch: NamePatch::new(wanted.name().clone()),
                },
                at,
            )?;
        }
    }
    let purges: Vec<_> = desired
        .stickers
        .iter()
        .map(|v| (EntityId::Sticker(v.id()), v.lifecycle()))
        .chain(
            desired
                .collections
                .iter()
                .map(|v| (EntityId::Collection(v.id()), v.lifecycle())),
        )
        .chain(
            desired
                .tags
                .iter()
                .map(|v| (EntityId::Tag(v.id()), v.lifecycle())),
        )
        .filter(|(_, life)| life.is_purged())
        .map(|(id, _)| id)
        .collect();
    for entity in purges {
        let life = match entity {
            EntityId::Sticker(id) => current
                .stickers
                .iter()
                .find(|v| v.id() == id)
                .map(|v| v.lifecycle()),
            EntityId::Collection(id) => current
                .collections
                .iter()
                .find(|v| v.id() == id)
                .map(|v| v.lifecycle()),
            EntityId::Tag(id) => current
                .tags
                .iter()
                .find(|v| v.id() == id)
                .map(|v| v.lifecycle()),
        };
        if let Some(life) = life.filter(|v| !v.is_active() && !v.is_purged()) {
            let op = OperationKind::PurgeEntity {
                entity,
                generation: life.generation(),
                expected_deleted_revision: life.revision(),
            };
            append(&mut current, &mut ops, op, at)?;
        }
    }
    for wanted in desired
        .stickers
        .iter()
        .filter(|v| v.lifecycle().is_active())
    {
        let Some(sticker) = current
            .stickers
            .iter()
            .find(|v| v.id() == wanted.id() && v.lifecycle().is_active())
            .cloned()
        else {
            continue;
        };
        let target = desired
            .collection_items
            .iter()
            .find(|r| {
                r.sticker_id() == wanted.id()
                    && desired
                        .collections
                        .iter()
                        .any(|c| r.is_effective(c, wanted))
            })
            .and_then(|r| {
                current
                    .collections
                    .iter()
                    .find(|c| c.id() == r.collection_id() && c.lifecycle().is_active())
            })
            .map(|c| (c.id(), c.lifecycle().generation()));
        let old = current
            .collection_items
            .iter()
            .find(|r| {
                r.sticker_id() == wanted.id()
                    && current
                        .collections
                        .iter()
                        .any(|c| r.is_effective(c, &sticker))
            })
            .map(|r| (r.collection_id(), r.collection_generation()));
        if old != target {
            append(
                &mut current,
                &mut ops,
                OperationKind::SetStickerCollection {
                    sticker_id: sticker.id(),
                    sticker_generation: sticker.lifecycle().generation(),
                    collection: target,
                },
                at,
            )?;
        }
        for tag in desired.tags.iter().filter(|v| v.lifecycle().is_active()) {
            let Some(remote_tag) = current
                .tags
                .iter()
                .find(|t| t.id() == tag.id() && t.lifecycle().is_active())
                .cloned()
            else {
                continue;
            };
            let present = desired
                .sticker_tags
                .iter()
                .any(|r| r.is_effective(wanted, tag));
            let was_present = current
                .sticker_tags
                .iter()
                .any(|r| r.is_effective(&sticker, &remote_tag));
            if present != was_present {
                append(
                    &mut current,
                    &mut ops,
                    OperationKind::SetTagMembership {
                        sticker_id: sticker.id(),
                        tag_id: tag.id(),
                        sticker_generation: sticker.lifecycle().generation(),
                        tag_generation: remote_tag.lifecycle().generation(),
                        present,
                    },
                    at,
                )?;
            }
        }
    }
    let mut ordered: Vec<_> = desired
        .collections
        .iter()
        .filter(|v| v.lifecycle().is_active())
        .collect();
    ordered.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
    let mut before = None;
    for wanted in ordered.into_iter().rev() {
        let Some(collection) = current
            .collections
            .iter()
            .find(|c| c.id() == wanted.id() && c.lifecycle().is_active())
            .cloned()
        else {
            continue;
        };
        append(
            &mut current,
            &mut ops,
            OperationKind::MoveCollection {
                collection_id: collection.id(),
                generation: collection.lifecycle().generation(),
                before_id: before,
            },
            at,
        )?;
        before = Some(collection.id());
        let mut items: Vec<_> = desired
            .collection_items
            .iter()
            .filter(|r| desired.stickers.iter().any(|s| r.is_effective(wanted, s)))
            .collect();
        items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
        let mut before_item = None;
        for item in items.into_iter().rev() {
            let Some(sticker) = current
                .stickers
                .iter()
                .find(|s| s.id() == item.sticker_id() && s.lifecycle().is_active())
                .cloned()
            else {
                continue;
            };
            append(
                &mut current,
                &mut ops,
                OperationKind::MoveCollectionItem {
                    collection_id: collection.id(),
                    sticker_id: sticker.id(),
                    collection_generation: collection.lifecycle().generation(),
                    sticker_generation: sticker.lifecycle().generation(),
                    before_id: before_item,
                },
                at,
            )?;
            before_item = Some(sticker.id());
        }
    }
    Ok(ops)
}

fn append(
    current: &mut ArchiveData,
    ops: &mut Vec<OperationKind>,
    op: OperationKind,
    at: TimestampMs,
) -> Result<()> {
    project::overlay(current, &op, at)?;
    ops.push(op);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use memedock_domain::{
        collection::Collection,
        identity::CollectionId,
        lifecycle::Tombstone,
        ordering::SortKey,
        tag::Name,
        version::{Generation, Revision},
    };
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn restore_then_patch_uses_the_new_server_generation() -> TestResult {
        let at = TimestampMs::new(100);
        let id = CollectionId::new();
        let original = Collection::new(id, Name::new("before".into())?, SortKey::default(), at);
        let mut remote = ArchiveData {
            collections: vec![original.clone()],
            ..Default::default()
        };
        remote.collections[0].delete(Generation::INITIAL, at, Revision::new(7)?)?;
        let mut desired = ArchiveData {
            collections: vec![original],
            ..Default::default()
        };
        desired.collections[0].patch(
            Generation::INITIAL,
            &NamePatch::new(Name::new("restored name".into())?),
            at,
            Revision::LOCAL,
        )?;
        let ops = operations(&desired, &remote, at)?;
        assert!(matches!(
            ops.first(),
            Some(OperationKind::RestoreCollection { .. })
        ));
        for op in &ops {
            project::overlay(&mut remote, op, at)?;
        }
        assert!(remote.collections[0].lifecycle().is_active());
        assert_eq!(
            remote.collections[0].lifecycle().generation(),
            Generation::INITIAL.checked_next()?
        );
        assert_eq!(remote.collections[0].name().as_str(), "restored name");
        Ok(())
    }

    #[test]
    fn compacted_deletion_is_rebuilt_under_its_exact_revision() -> TestResult {
        let at = TimestampMs::new(100);
        let id = CollectionId::new();
        let desired = ArchiveData {
            collections: vec![Collection::new(
                id,
                Name::new("recovered".into())?,
                SortKey::default(),
                at,
            )],
            ..Default::default()
        };
        let mut remote = ArchiveData {
            tombstones: vec![Tombstone::new(
                EntityId::Collection(id),
                Generation::new(3)?,
                Revision::new(8)?,
                at,
            )],
            ..Default::default()
        };
        let ops = operations(&desired, &remote, at)?;
        assert!(
            matches!(ops.first(), Some(OperationKind::RestoreCollection { expected_deleted_revision, rebuild: Some(_), .. }) if expected_deleted_revision.get() == 8)
        );
        for op in &ops {
            project::overlay(&mut remote, op, at)?;
        }
        assert_eq!(remote.collections[0].lifecycle().generation().get(), 4);
        assert_eq!(remote.collections[0].name().as_str(), "recovered");
        super::super::catalog::clean(&mut remote);
        remote.validate()?;
        Ok(())
    }
}
