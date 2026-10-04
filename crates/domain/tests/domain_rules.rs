use memedock_domain::{
    asset::{Asset, ImageFormat},
    change::{FieldPatch, NamePatch, Operation, OperationKind, StickerPatch, StickerRebuild},
    collection::Collection,
    error::DomainError,
    identity::{CollectionId, ContentHash, DeviceId, OperationId, StickerId, TagId},
    lifecycle::{EntityId, Lifecycle, Tombstone},
    local::{
        BlobStatus, ChangeStatus, LocalAsset, LocalChange, LocalUsage, ThumbnailStatus, UsageAction,
    },
    ordering::SortKey,
    relation::{CollectionItem, StickerTag},
    sticker::{ImportDisposition, Sticker},
    sync::{EventKind, OrderedSticker, PullPage, SyncEvent},
    tag::{Name, Tag, normalize_text},
    version::{ByteSize, EventSeq, Generation, LocalOrder, Revision, TimestampMs},
};
use std::{error::Error, num::NonZeroU32};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ID: &str = "019a1c00-0000-7000-8000-000000000001";
fn at(value: i64) -> TimestampMs {
    TimestampMs::new(value)
}
fn revision(value: i64) -> Result<Revision, DomainError> {
    Revision::new(value)
}
fn asset() -> TestResult<Asset> {
    Ok(Asset::new(
        HASH.parse()?,
        ByteSize::new(42)?,
        ImageFormat::Png,
        (
            NonZeroU32::new(10).ok_or("width")?,
            NonZeroU32::new(20).ok_or("height")?,
        ),
        false,
        at(100),
    ))
}
fn sticker() -> TestResult<Sticker> {
    Ok(Sticker::new(
        &asset()?,
        "猫猫".into(),
        "cat.png".into(),
        at(100),
    ))
}
fn collection() -> TestResult<Collection> {
    Ok(Collection::new(
        ID.parse()?,
        "常用".parse()?,
        SortKey::default(),
        at(100),
    ))
}
fn tag() -> TestResult<Tag> {
    Ok(Tag::new(ID.parse()?, "猫猫".parse()?, at(100)))
}
fn rename(value: &str) -> TestResult<StickerPatch> {
    Ok(StickerPatch::new(
        FieldPatch::Set(value.to_owned()),
        FieldPatch::Missing,
        FieldPatch::Missing,
    )?)
}
fn operation() -> TestResult<Operation> {
    Ok(Operation::new(OperationKind::PatchSticker {
        sticker_id: HASH.parse()?,
        generation: Generation::INITIAL,
        patch: rename("无语猫")?,
    })?)
}
fn change() -> TestResult<LocalChange> {
    Ok(LocalChange::new(
        LocalOrder::new(1)?,
        ID.parse()?,
        operation()?,
        at(100),
    ))
}
fn event(seq: i64) -> TestResult<SyncEvent> {
    let mut entity = sticker()?;
    entity.patch(
        Generation::INITIAL,
        &rename("更新")?,
        at(200),
        revision(seq)?,
    )?;
    Ok(SyncEvent::new(
        EventSeq::new(seq)?,
        ID.parse()?,
        ID.parse()?,
        at(200),
        EventKind::StickerPatched { entity },
    )?)
}

#[test]
fn identities_validate_actual_kind_and_canonical_hash() -> TestResult {
    let hash: ContentHash = HASH.parse()?;
    assert_eq!(hash.to_string(), HASH);
    assert_eq!(StickerId::new(hash).content_hash(), hash);
    for invalid in [
        "",
        "ab",
        &HASH.to_uppercase(),
        &"z".repeat(64),
        &"猫".repeat(64),
    ] {
        assert!(invalid.parse::<ContentHash>().is_err());
    }
    for invalid in [
        "",
        "00000000-0000-0000-0000-000000000000",
        "67e55044-10b1-426f-9247-bb680e5fe0c8",
        "019a1c00-0000-7000-0000-000000000001",
    ] {
        assert!(invalid.parse::<CollectionId>().is_err());
        assert!(invalid.parse::<TagId>().is_err());
        assert!(invalid.parse::<DeviceId>().is_err());
        assert!(invalid.parse::<OperationId>().is_err());
    }
    assert_eq!(ID.parse::<CollectionId>()?.to_string(), ID);
    assert_eq!(CollectionId::new().as_uuid().get_version_num(), 7);
    Ok(())
}

#[test]
fn counters_bound_sqlite_integers_and_checked_overflow() -> TestResult {
    assert!(Generation::new(-1).is_err());
    assert!(Revision::new(-1).is_err());
    assert!(LocalOrder::new(0).is_err());
    assert!(EventSeq::new(0).is_err());
    assert!(ByteSize::new(-1).is_err());
    assert_eq!(
        Generation::new(i64::MAX)?.checked_next(),
        Err(DomainError::VersionOverflow)
    );
    assert_eq!(Generation::INITIAL.checked_next()?.get(), 1);
    assert_eq!(Revision::from(EventSeq::new(4)?).get(), 4);
    Ok(())
}

#[test]
fn patch_changes_only_supplied_fields_and_keeps_original_name() -> TestResult {
    let mut s = sticker()?;
    let note = StickerPatch::new(
        FieldPatch::Missing,
        FieldPatch::Set("备注".into()),
        FieldPatch::Set(true),
    )?;
    s.patch(Generation::INITIAL, &note, at(200), revision(1)?)?;
    s.patch(
        Generation::INITIAL,
        &rename("新标题")?,
        at(300),
        revision(2)?,
    )?;
    assert_eq!(s.title(), "新标题");
    assert_eq!(s.note(), "备注");
    assert!(s.starred());
    assert_eq!(s.original_name(), "cat.png");
    assert_eq!(s.lifecycle().revision(), revision(2)?);
    // Empty strings intentionally clear non-null text columns.
    s.patch(Generation::INITIAL, &rename("")?, at(400), revision(3)?)?;
    assert_eq!(s.title(), "");
    Ok(())
}

#[test]
fn rejected_patch_is_atomic_and_clocks_do_not_determine_winner() -> TestResult {
    let mut s = sticker()?;
    s.patch(
        Generation::INITIAL,
        &rename("已确认")?,
        at(300),
        revision(4)?,
    )?;
    let before = s.clone();
    assert_eq!(
        s.patch(
            Generation::INITIAL,
            &rename("旧版本")?,
            at(999),
            revision(3)?
        ),
        Err(DomainError::StaleRevision)
    );
    assert_eq!(s, before);
    // A newer server revision with an older clock value is valid.
    s.patch(
        Generation::INITIAL,
        &rename("后来提交")?,
        at(1),
        revision(5)?,
    )?;
    assert_eq!(s.title(), "后来提交");
    assert_eq!(s.lifecycle().updated_at(), at(1));
    Ok(())
}

#[test]
fn deletion_requires_explicit_restore_and_blocks_old_generation() -> TestResult {
    let mut s = sticker()?;
    assert!(s.delete(Generation::INITIAL, at(200), revision(1)?)?);
    let deleted = s.clone();
    assert!(!s.delete(Generation::INITIAL, at(300), revision(2)?)?);
    assert_eq!(s, deleted);
    assert_eq!(
        s.patch(
            Generation::INITIAL,
            &rename("旧更新")?,
            at(300),
            revision(2)?
        ),
        Err(DomainError::EntityDeleted)
    );
    assert_eq!(
        s.restore(revision(0)?, at(300), revision(2)?),
        Err(DomainError::StaleRevision)
    );
    assert_eq!(s, deleted);
    s.restore(revision(1)?, at(400), revision(2)?)?;
    assert_eq!(s.lifecycle().generation().get(), 1);
    assert!(s.lifecycle().is_active());
    assert_eq!(
        s.patch(
            Generation::INITIAL,
            &rename("旧周期")?,
            at(500),
            revision(3)?
        ),
        Err(DomainError::StaleGeneration)
    );
    assert_eq!(
        s.restore(revision(2)?, at(600), revision(3)?),
        Err(DomainError::EntityActive)
    );
    Ok(())
}

#[test]
fn local_restore_and_generation_overflow_preserve_state() -> TestResult {
    let mut state = Lifecycle::new(at(100));
    state.delete(Generation::INITIAL, at(200), Revision::LOCAL)?;
    state.restore(Revision::LOCAL, at(300), Revision::LOCAL)?;
    assert_eq!(state.generation().get(), 1);
    assert_eq!(state.revision(), Revision::LOCAL);
    let mut overflow = Lifecycle::from_state(
        Generation::new(i64::MAX)?,
        revision(4)?,
        at(100),
        at(200),
        Some(at(200)),
    );
    let before = overflow.clone();
    assert_eq!(
        overflow.restore(revision(4)?, at(300), revision(5)?),
        Err(DomainError::VersionOverflow)
    );
    assert_eq!(overflow, before);
    Ok(())
}

#[test]
fn unknown_tombstone_keeps_identity_and_requires_deleted_revision() -> TestResult {
    let t = Tombstone::new(
        EntityId::Sticker(HASH.parse()?),
        Generation::new(3)?,
        revision(12)?,
        at(100),
    );
    assert_eq!(
        t.ensure_restore(revision(11)?),
        Err(DomainError::StaleRevision)
    );
    assert_eq!(t.ensure_restore(revision(12)?)?.get(), 4);
    assert_eq!(t.entity(), EntityId::Sticker(HASH.parse()?));
    Ok(())
}

#[test]
fn duplicate_import_preserves_metadata_and_prompts_for_recovery() -> TestResult {
    let a = asset()?;
    let mut s = sticker()?;
    s.patch(
        Generation::INITIAL,
        &rename("手工名称")?,
        at(200),
        Revision::LOCAL,
    )?;
    let before = s.clone();
    assert_eq!(s.classify_import(&a)?, ImportDisposition::Reuse);
    assert_eq!(s, before);
    s.delete(Generation::INITIAL, at(300), Revision::LOCAL)?;
    assert_eq!(s.classify_import(&a)?, ImportDisposition::RestoreRequired);
    let another = Asset::new(
        ContentHash::from_bytes([255; 32]),
        a.byte_size(),
        a.format(),
        (
            NonZeroU32::new(a.width()).ok_or("width")?,
            NonZeroU32::new(a.height()).ok_or("height")?,
        ),
        false,
        at(100),
    );
    assert_eq!(
        s.classify_import(&another),
        Err(DomainError::IdentityMismatch)
    );
    Ok(())
}

#[test]
fn relationships_hide_deleted_and_restored_generations() -> TestResult {
    let mut c = collection()?;
    let mut s = sticker()?;
    let mut t = tag()?;
    let membership =
        CollectionItem::new(&c, &s, SortKey::default(), true, Revision::LOCAL, at(100))?;
    let tagging = StickerTag::new(&s, &t, true, Revision::LOCAL, at(100))?;
    assert!(membership.is_effective(&c, &s));
    assert!(tagging.is_effective(&s, &t));
    s.delete(Generation::INITIAL, at(200), Revision::LOCAL)?;
    assert!(!membership.is_effective(&c, &s));
    assert!(!tagging.is_effective(&s, &t));
    s.restore(Revision::LOCAL, at(300), Revision::LOCAL)?;
    assert!(!membership.is_effective(&c, &s));
    assert!(!tagging.is_effective(&s, &t));
    let mut fresh =
        CollectionItem::new(&c, &s, SortKey::default(), true, Revision::LOCAL, at(300))?;
    assert!(fresh.is_effective(&c, &s));
    c.delete(Generation::INITIAL, at(400), Revision::LOCAL)?;
    c.restore(Revision::LOCAL, at(500), Revision::LOCAL)?;
    assert!(!fresh.is_effective(&c, &s));
    assert_eq!(
        fresh.set_present(&c, &s, true, Revision::LOCAL, at(600)),
        Err(DomainError::StaleGeneration)
    );
    t.delete(Generation::INITIAL, at(400), Revision::LOCAL)?;
    t.restore(Revision::LOCAL, at(500), Revision::LOCAL)?;
    assert!(!tagging.is_effective(&s, &t));
    Ok(())
}

#[test]
fn relationship_changes_validate_endpoints_and_do_not_revive_old_work() -> TestResult {
    let c = collection()?;
    let s = sticker()?;
    let t = tag()?;
    let mut relation =
        CollectionItem::new(&c, &s, SortKey::default(), true, revision(1)?, at(100))?;
    let before = relation.clone();
    let other = Collection::new(
        CollectionId::new(),
        "其他".parse()?,
        SortKey::default(),
        at(100),
    );
    assert_eq!(
        relation.set_present(&other, &s, false, revision(2)?, at(200)),
        Err(DomainError::IdentityMismatch)
    );
    assert_eq!(
        relation.set_present(&c, &s, false, revision(0)?, at(200)),
        Err(DomainError::StaleRevision)
    );
    assert_eq!(relation, before);
    relation.set_present(&c, &s, false, revision(2)?, at(200))?;
    assert!(!relation.is_effective(&c, &s));
    assert_eq!(
        relation.move_to(&c, &s, SortKey::default(), revision(3)?, at(300)),
        Err(DomainError::InvalidTransition)
    );
    let mut tags = StickerTag::new(&s, &t, true, revision(1)?, at(100))?;
    tags.set_present(&s, &t, false, revision(2)?, at(200))?;
    assert!(!tags.is_effective(&s, &t));
    Ok(())
}

#[test]
fn tag_names_normalize_without_changing_display_or_identity() -> TestResult {
    assert_eq!(normalize_text("  ＣＡＴ\t猫猫\n"), "cat 猫猫");
    assert_eq!(normalize_text("无语 😺"), "无语 😺");
    assert_eq!(normalize_text("猫猫"), "猫猫");
    assert!(" \n\t".parse::<Name>().is_err());
    let mut t = Tag::new(ID.parse()?, "  ＣＡＴ  ".parse()?, at(100));
    assert_eq!(t.name().as_str(), "  ＣＡＴ  ");
    assert_eq!(t.normalized_name(), "cat");
    t.patch(
        Generation::INITIAL,
        &NamePatch::new("狗狗".parse()?),
        at(200),
        revision(1)?,
    )?;
    assert_eq!(t.id(), ID.parse::<TagId>()?);
    assert_eq!(t.normalized_name(), "狗狗");
    assert!(
        Tag::from_state(
            t.id(),
            "狗狗".parse()?,
            "wrong".into(),
            t.lifecycle().clone()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn invalid_move_and_unknown_restore_cannot_misidentify_content() -> TestResult {
    assert!(
        Operation::new(OperationKind::MoveCollectionItem {
            collection_id: ID.parse()?,
            sticker_id: HASH.parse()?,
            collection_generation: Generation::INITIAL,
            sticker_generation: Generation::INITIAL,
            before_id: Some(HASH.parse()?)
        })
        .is_err()
    );
    assert!(
        Operation::new(OperationKind::MoveCollection {
            collection_id: ID.parse()?,
            generation: Generation::INITIAL,
            before_id: Some(ID.parse()?)
        })
        .is_err()
    );
    assert!(
        Operation::new(OperationKind::RestoreSticker {
            sticker_id: StickerId::new(ContentHash::from_bytes([0; 32])),
            expected_deleted_revision: revision(1)?,
            rebuild: Some(StickerRebuild {
                asset: asset()?,
                title: "恢复".into(),
                original_name: "cat.png".into(),
                note: String::new(),
                starred: false
            }),
        })
        .is_err()
    );
    Ok(())
}

#[test]
fn outbox_payload_freezes_on_first_attempt_even_after_retry() -> TestResult {
    let mut item = change()?;
    item.rebase_unsent(operation()?)?;
    item.begin_send()?;
    assert_eq!(item.attempts(), 1);
    assert_eq!(
        item.rebase_unsent(operation()?),
        Err(DomainError::OperationFrozen)
    );
    let frozen = item.operation().clone();
    item.retry(at(300), "network".into())?;
    assert_eq!(item.status(), ChangeStatus::Pending);
    assert_eq!(
        item.rebase_unsent(operation()?),
        Err(DomainError::OperationFrozen)
    );
    item.begin_send()?;
    assert_eq!(item.attempts(), 2);
    assert_eq!(item.operation(), &frozen);
    Ok(())
}

#[test]
fn accepted_push_retains_overlay_until_canonical_state_is_applied() -> TestResult {
    let mut item = change()?;
    item.begin_send()?;
    item.accepted(revision(9)?)?;
    assert!(item.contributes_overlay());
    assert_eq!(item.status(), ChangeStatus::AcceptedWaitingPull);
    let before = item.clone();
    assert_eq!(
        item.confirm_canonical(revision(8)?),
        Err(DomainError::StaleRevision)
    );
    assert_eq!(item, before);
    item.confirm_canonical(revision(9)?)?;
    assert!(!item.contributes_overlay());
    assert_eq!(item.status(), ChangeStatus::Confirmed);
    Ok(())
}

#[test]
fn matching_event_can_confirm_before_push_response_and_wrong_id_cannot() -> TestResult {
    let mut item = change()?;
    item.begin_send()?;
    assert_eq!(
        item.confirm_event(OperationId::new(), EventSeq::new(1)?),
        Err(DomainError::IdentityMismatch)
    );
    item.confirm_event(item.op_id(), EventSeq::new(1)?)?;
    assert_eq!(item.status(), ChangeStatus::Confirmed);
    assert_eq!(
        item.accepted(revision(1)?),
        Err(DomainError::InvalidTransition)
    );
    Ok(())
}

#[test]
fn blocked_changes_and_bootstrap_sealed_history_do_not_contribute_overlay() -> TestResult {
    let mut blocked = change()?;
    blocked.block("entity_deleted".into())?;
    assert!(!blocked.contributes_overlay());
    assert_eq!(blocked.begin_send(), Err(DomainError::InvalidTransition));
    let mut sealed = change()?;
    sealed.seal_local()?;
    assert_eq!(sealed.status(), ChangeStatus::Sealed);
    assert!(!sealed.contributes_overlay());
    let mut sent = change()?;
    sent.begin_send()?;
    sent.retry(at(200), "network".into())?;
    assert_eq!(sent.seal_local(), Err(DomainError::InvalidTransition));
    Ok(())
}

#[test]
fn event_and_pull_page_enforce_revision_order_and_no_cursor_jump() -> TestResult {
    assert!(
        SyncEvent::new(
            EventSeq::new(3)?,
            ID.parse()?,
            ID.parse()?,
            at(100),
            EventKind::StickerPatched { entity: sticker()? }
        )
        .is_err()
    );
    let lib = ID.parse()?;
    let epoch = ID.parse()?;
    let page = PullPage::new(lib, epoch, revision(2)?, vec![event(3)?, event(8)?], false)?;
    assert_eq!(page.next_cursor(), revision(8)?);
    // Gaps are permitted: only ordered returned events determine the cursor.
    page.validate_after(revision(2)?)?;
    assert_eq!(
        page.validate_after(revision(3)?),
        Err(DomainError::StaleRevision)
    );
    assert!(PullPage::new(lib, epoch, revision(2)?, vec![event(8)?, event(3)?], false).is_err());
    assert!(PullPage::new(lib, epoch, revision(2)?, vec![event(3)?, event(3)?], false).is_err());
    assert!(PullPage::new(lib, epoch, revision(2)?, vec![], true).is_err());
    let empty = PullPage::new(lib, epoch, revision(2)?, vec![], false)?;
    assert_eq!(empty.next_cursor(), revision(2)?);
    assert!(empty.validate_after(revision(1)?).is_err());
    Ok(())
}

#[test]
fn reorder_event_rejects_duplicates_and_unsorted_keys() -> TestResult {
    let first = SortKey::default();
    let next = SortKey::between(Some(&first), None)?;
    let id: StickerId = HASH.parse()?;
    let duplicate = vec![
        OrderedSticker {
            sticker_id: id,
            sort_key: first.clone(),
        },
        OrderedSticker {
            sticker_id: id,
            sort_key: next,
        },
    ];
    assert!(
        SyncEvent::new(
            EventSeq::new(1)?,
            ID.parse()?,
            ID.parse()?,
            at(100),
            EventKind::CollectionItemsReordered {
                collection_id: ID.parse()?,
                items: duplicate
            }
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn local_asset_and_usage_separate_resource_readiness_from_business_identity() -> TestResult {
    let mut local = LocalAsset::new(HASH.parse()?);
    assert_eq!(local.begin_thumbnail(), Err(DomainError::InvalidTransition));
    local.begin_download();
    assert_eq!(local.blob_status(), BlobStatus::Downloading);
    local.verified(at(100));
    local.begin_thumbnail()?;
    local.thumbnail_failed("decode_failed".into());
    assert_eq!(local.blob_status(), BlobStatus::Ready);
    assert_eq!(local.thumb_status(), ThumbnailStatus::Failed);
    local.begin_thumbnail()?;
    local.thumbnail_ready()?;
    local.evict_thumbnail();
    assert_eq!(local.blob_status(), BlobStatus::Ready);
    let mut usage = LocalUsage::first_use(HASH.parse()?, at(100));
    assert_eq!(usage.use_count(), 1);
    usage.record(at(200), UsageAction::ShareLaunched)?;
    assert_eq!(usage.use_count(), 2);
    assert_eq!(usage.last_used_at(), at(200));
    Ok(())
}
