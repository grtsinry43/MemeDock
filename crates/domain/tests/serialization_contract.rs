use memedock_domain::{
    asset::Asset,
    change::{
        CollectionRebuild, FieldPatch, NamePatch, Operation, OperationKind, StickerPatch,
        StickerRebuild,
    },
    collection::Collection,
    identity::{
        CollectionId, ContentHash, DeviceId, LibraryId, OperationId, StickerId, SyncEpoch, TagId,
    },
    lifecycle::{EntityId, Tombstone},
    local::{LocalChange, LocalUsage},
    ordering::SortKey,
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    sync::{
        Device, EventKind, OperationEnvelope, OperationReceipt, PullPage, ReceiptOutcome,
        RejectionCode, ResolutionNote, SyncEvent,
    },
    tag::{Name, Tag},
    version::{EventSeq, Generation, LocalOrder, ProtocolVersion, Revision, TimestampMs},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ID: &str = "019a1c00-0000-7000-8000-000000000001";
const SECOND_ID: &str = "019a1c00-0000-7000-8000-000000000002";
fn at(value: i64) -> TimestampMs {
    TimestampMs::new(value)
}
fn asset_value() -> Value {
    json!({"hash": HASH, "byte_size": 42, "mime": "image/png", "width": 10,
        "height": 20, "animated": false, "created_at": 100})
}
fn sticker_value() -> Value {
    json!({"id": HASH, "title": "猫猫", "original_name": "cat.png", "note": "",
        "starred": false, "generation": 0, "revision": 0, "created_at": 100, "updated_at": 100, "deleted_at": null})
}
fn asset() -> TestResult<Asset> {
    Ok(serde_json::from_value(asset_value())?)
}
fn patch() -> TestResult<StickerPatch> {
    Ok(serde_json::from_value(json!({"title": "无语猫"}))?)
}
fn operation() -> TestResult<Operation> {
    Ok(Operation::new(OperationKind::PatchSticker {
        sticker_id: HASH.parse()?,
        generation: Generation::INITIAL,
        patch: patch()?,
    })?)
}
fn golden<T>(value: Value) -> TestResult<T>
where
    T: DeserializeOwned + Serialize,
{
    let result = serde_json::from_value::<T>(value.clone())?;
    assert_eq!(serde_json::to_value(&result)?, value);
    Ok(result)
}
fn roundtrip<T>(value: &T) -> TestResult
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let decoded: T = serde_json::from_str(&serde_json::to_string(value)?)?;
    assert_eq!(&decoded, value);
    Ok(())
}

#[test]
fn hash_and_uuid_wire_types_validate_before_entering_domain() -> TestResult {
    assert_eq!(
        serde_json::to_value(HASH.parse::<ContentHash>()?)?,
        json!(HASH)
    );
    assert_eq!(
        serde_json::to_value(ID.parse::<CollectionId>()?)?,
        json!(ID)
    );
    assert_eq!(serde_json::to_value(ID.parse::<TagId>()?)?, json!(ID));
    assert_eq!(serde_json::to_value(ID.parse::<DeviceId>()?)?, json!(ID));
    assert_eq!(serde_json::to_value(ID.parse::<OperationId>()?)?, json!(ID));
    for value in [json!(HASH.to_uppercase()), json!("bad"), json!(vec![0; 32])] {
        assert!(serde_json::from_value::<ContentHash>(value).is_err());
    }
    assert!(
        serde_json::from_value::<CollectionId>(json!("00000000-0000-0000-0000-000000000000"))
            .is_err()
    );
    assert!(
        serde_json::from_value::<CollectionId>(json!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .is_err()
    );
    // The design specifies UUID, not specifically v7, for library/epoch identity.
    let library: LibraryId = "67e55044-10b1-426f-9247-bb680e5fe0c8".parse()?;
    let epoch: SyncEpoch = ID.parse()?;
    roundtrip(&library)?;
    roundtrip(&epoch)?;
    Ok(())
}

#[test]
fn asset_golden_contract_rejects_invalid_dimensions_and_mime() -> TestResult {
    let a: Asset = golden(asset_value())?;
    assert_eq!(a.sticker_id().to_string(), HASH);
    assert_eq!(a.format().extension(), "png");
    for (key, value) in [
        ("width", json!(0)),
        ("height", json!(-1)),
        ("byte_size", json!(-1)),
        ("byte_size", json!(u64::MAX)),
        ("mime", json!("image/heic")),
    ] {
        let mut invalid = asset_value();
        invalid[key] = value;
        assert!(
            serde_json::from_value::<Asset>(invalid).is_err(),
            "accepted invalid {key}"
        );
    }
    Ok(())
}

#[test]
fn entity_golden_contracts_keep_lifecycle_flat_and_tag_normalization_checked() -> TestResult {
    let _: Sticker = golden(sticker_value())?;
    let _: Collection = golden(json!({"id": ID, "name": "常用", "sort_key": "80",
        "generation": 0, "revision": 0, "created_at": 100, "updated_at": 100, "deleted_at": null}))?;
    let _: Tag = golden(json!({"id": ID, "name": "ＣＡＴ", "normalized_name": "cat",
        "generation": 0, "revision": 0, "created_at": 100, "updated_at": 100, "deleted_at": null}))?;
    let mut bad_tag = serde_json::to_value(Tag::new(ID.parse()?, "ＣＡＴ".parse()?, at(100)))?;
    bad_tag["normalized_name"] = json!("not-cat");
    assert!(serde_json::from_value::<Tag>(bad_tag).is_err());
    for (key, value) in [
        ("generation", json!(-1)),
        ("revision", json!(u64::MAX)),
        ("id", json!("invalid")),
    ] {
        let mut bad = sticker_value();
        bad[key] = value;
        assert!(serde_json::from_value::<Sticker>(bad).is_err());
    }
    Ok(())
}

#[test]
fn patch_contract_distinguishes_missing_null_false_and_empty_string() -> TestResult {
    let patch: StickerPatch = golden(json!({"title": "", "starred": false}))?;
    assert!(patch.note().is_missing());
    assert_eq!(patch.title(), &FieldPatch::Set(String::new()));
    assert_eq!(patch.starred(), &FieldPatch::Set(false));
    let missing: FieldPatch<String> = FieldPatch::Missing;
    assert!(serde_json::to_string(&missing).is_err());
    assert_eq!(
        serde_json::from_value::<FieldPatch<String>>(Value::Null)?,
        FieldPatch::Clear
    );
    assert_eq!(
        serde_json::to_value(FieldPatch::<String>::Clear)?,
        Value::Null
    );
    for invalid in [
        json!({}),
        json!({"title": null}),
        json!({"note": null}),
        json!({"starred": null}),
        json!({"starred": "false"}),
        json!({"titel": "typo"}),
    ] {
        assert!(serde_json::from_value::<StickerPatch>(invalid).is_err());
    }
    assert!(serde_json::from_value::<NamePatch>(json!({"name": " "})).is_err());
    assert!(serde_json::from_value::<NamePatch>(json!({"name": null})).is_err());
    Ok(())
}

#[test]
fn operation_envelope_matches_document_shape_and_rejects_wrong_version_or_identity() -> TestResult {
    let value = json!({"protocol_version": 1, "library_id": ID, "sync_epoch": ID,
        "device_id": ID, "op_id": ID, "client_time_ms": 100,
        "operation": {"type": "patch_sticker", "sticker_id": HASH, "generation": 0,
            "patch": {"title": "无语猫"}}});
    let envelope: OperationEnvelope = golden(value.clone())?;
    envelope.ensure_identity(ID.parse()?, ID.parse()?, ID.parse()?)?;
    assert!(
        envelope
            .ensure_identity(SECOND_ID.parse()?, ID.parse()?, ID.parse()?)
            .is_err()
    );
    for invalid_version in [0, 2, u32::MAX] {
        let mut invalid = value.clone();
        invalid["protocol_version"] = json!(invalid_version);
        assert!(serde_json::from_value::<OperationEnvelope>(invalid).is_err());
    }
    let mut invalid = value;
    invalid["operation"]["patch"]["extra"] = json!("typo");
    assert!(serde_json::from_value::<OperationEnvelope>(invalid).is_err());
    assert!(serde_json::from_value::<ProtocolVersion>(json!(2)).is_err());
    Ok(())
}

#[test]
fn all_operation_variants_roundtrip_without_untyped_payloads() -> TestResult {
    let sid: StickerId = HASH.parse()?;
    let cid: CollectionId = ID.parse()?;
    let tid: TagId = ID.parse()?;
    let generation = Generation::INITIAL;
    let revision = Revision::new(1)?;
    let name: Name = "猫猫".parse()?;
    let rebuild = StickerRebuild {
        asset: asset()?,
        title: "恢复".into(),
        original_name: "cat.png".into(),
        note: "备注".into(),
        starred: false,
    };
    let kinds = vec![
        OperationKind::CreateSticker {
            asset: asset()?,
            title: "猫猫".into(),
            original_name: "cat.png".into(),
            note: "备注".into(),
            starred: true,
        },
        OperationKind::PatchSticker {
            sticker_id: sid,
            generation,
            patch: patch()?,
        },
        OperationKind::DeleteSticker {
            sticker_id: sid,
            generation,
        },
        OperationKind::RestoreSticker {
            sticker_id: sid,
            expected_deleted_revision: revision,
            rebuild: Some(rebuild),
        },
        OperationKind::RestoreSticker {
            sticker_id: sid,
            expected_deleted_revision: revision,
            rebuild: None,
        },
        OperationKind::CreateCollection {
            collection_id: cid,
            name: name.clone(),
            before_id: None,
        },
        OperationKind::PatchCollection {
            collection_id: cid,
            generation,
            patch: NamePatch::new(name.clone()),
        },
        OperationKind::MoveCollection {
            collection_id: cid,
            generation,
            before_id: Some(SECOND_ID.parse()?),
        },
        OperationKind::DeleteCollection {
            collection_id: cid,
            generation,
        },
        OperationKind::RestoreCollection {
            collection_id: cid,
            expected_deleted_revision: revision,
            rebuild: Some(CollectionRebuild {
                name: name.clone(),
                before_id: None,
            }),
        },
        OperationKind::CreateTag {
            tag_id: tid,
            name: name.clone(),
        },
        OperationKind::PatchTag {
            tag_id: tid,
            generation,
            patch: NamePatch::new(name.clone()),
        },
        OperationKind::DeleteTag {
            tag_id: tid,
            generation,
        },
        OperationKind::RestoreTag {
            tag_id: tid,
            expected_deleted_revision: revision,
            rebuild: Some(name),
        },
        OperationKind::SetStickerCollection {
            sticker_id: sid,
            sticker_generation: generation,
            collection: Some((cid, generation)),
        },
        OperationKind::MoveCollectionItem {
            collection_id: cid,
            sticker_id: sid,
            collection_generation: generation,
            sticker_generation: generation,
            before_id: None,
        },
        OperationKind::SetTagMembership {
            sticker_id: sid,
            tag_id: tid,
            sticker_generation: generation,
            tag_generation: generation,
            present: false,
        },
    ];
    for kind in kinds {
        roundtrip(&Operation::new(kind)?)?;
    }
    Ok(())
}

#[test]
fn invalid_operation_cannot_bypass_cross_field_validation_via_json() {
    let invalid = json!({"type": "move_collection_item", "collection_id": ID, "sticker_id": HASH,
        "collection_generation": 0, "sticker_generation": 0, "before_id": HASH});
    assert!(serde_json::from_value::<Operation>(invalid).is_err());
    let invalid = json!({"type": "restore_sticker", "sticker_id": "00".repeat(32),
        "expected_deleted_revision": 1, "rebuild": {"asset": asset_value(), "title": "猫猫",
        "original_name": "cat.png", "note": "", "starred": false}});
    assert!(serde_json::from_value::<Operation>(invalid).is_err());
    assert!(serde_json::from_value::<Operation>(json!({"type": "upsert_sticker"})).is_err());
}

#[test]
fn relations_and_unknown_tombstones_have_explicit_endpoint_versions() -> TestResult {
    let _: CollectionItem = golden(json!({"collection_id": ID, "sticker_id": HASH,
        "collection_generation": 2, "sticker_generation": 3, "present": false,
        "sort_key": "80", "revision": 9, "updated_at": 100}))?;
    let _: StickerTag = golden(json!({"sticker_id": HASH, "tag_id": ID,
        "sticker_generation": 3, "tag_generation": 2, "present": true, "revision": 9, "updated_at": 100}))?;
    let tomb: Tombstone = golden(json!({"entity_kind": "sticker", "entity_id": HASH,
        "generation": 2, "revision": 9, "deleted_at": 100}))?;
    assert_eq!(tomb.entity(), EntityId::Sticker(HASH.parse()?));
    assert!(
        serde_json::from_value::<Tombstone>(json!({"entity_kind": "collection", "entity_id": HASH,
        "generation": 2, "revision": 9, "deleted_at": 100}))
        .is_err()
    );
    Ok(())
}

#[test]
fn outbox_contract_rejects_impossible_states_and_survives_restart() -> TestResult {
    let mut item = LocalChange::new(LocalOrder::new(1)?, ID.parse()?, operation()?, at(100));
    roundtrip(&item)?;
    item.begin_send()?;
    roundtrip(&item)?;
    item.accepted(Revision::new(9)?)?;
    roundtrip(&item)?;
    let mut bad = serde_json::to_value(&item)?;
    bad["attempts"] = json!(0);
    assert!(serde_json::from_value::<LocalChange>(bad).is_err());
    let mut bad = serde_json::to_value(&item)?;
    bad["accepted_revision"] = Value::Null;
    assert!(serde_json::from_value::<LocalChange>(bad).is_err());
    let mut bad = serde_json::to_value(&item)?;
    bad["schema_version"] = json!(2);
    assert!(serde_json::from_value::<LocalChange>(bad).is_err());
    item.confirm_canonical(Revision::new(9)?)?;
    roundtrip(&item)?;
    let mut bad = serde_json::to_value(&item)?;
    bad["retry_after"] = json!(200);
    assert!(serde_json::from_value::<LocalChange>(bad).is_err());
    let mut bad_usage = serde_json::to_value(LocalUsage::first_use(HASH.parse()?, at(100)))?;
    bad_usage["use_count"] = json!(-1);
    assert!(serde_json::from_value::<LocalUsage>(bad_usage).is_err());
    Ok(())
}

#[test]
fn pull_event_contract_binds_entity_revision_to_event_and_last_returned_cursor() -> TestResult {
    let mut entity = sticker_value();
    entity["revision"] = json!(9);
    let event = json!({"seq": 9, "device_id": ID, "op_id": ID, "created_at": 100,
        "type": "sticker_patched", "entity": entity});
    let _: SyncEvent = golden(event.clone())?;
    let page = json!({"protocol_version": 1, "library_id": ID, "sync_epoch": ID,
        "next_cursor": 9, "has_more": false, "events": [event.clone()]});
    let _: PullPage = golden(page.clone())?;
    let mut bad = page;
    bad["next_cursor"] = json!(100);
    assert!(serde_json::from_value::<PullPage>(bad).is_err());
    let mut bad = event;
    bad["seq"] = json!(8);
    assert!(serde_json::from_value::<SyncEvent>(bad).is_err());
    Ok(())
}

#[test]
fn every_canonical_entity_event_roundtrips() -> TestResult {
    let a = asset()?;
    let s = Sticker::from_state(
        a.sticker_id(),
        "猫猫".into(),
        "cat.png".into(),
        "".into(),
        false,
        memedock_domain::lifecycle::Lifecycle::from_state(
            Generation::INITIAL,
            Revision::new(1)?,
            at(100),
            at(100),
            None,
        ),
    );
    let c = Collection::from_state(
        ID.parse()?,
        "常用".parse()?,
        SortKey::default(),
        s.lifecycle().clone(),
    );
    let t = Tag::from_state(
        ID.parse()?,
        "猫猫".parse()?,
        "猫猫".into(),
        s.lifecycle().clone(),
    )?;
    let membership =
        CollectionItem::new(&c, &s, SortKey::default(), true, Revision::new(1)?, at(100))?;
    let tagging = StickerTag::new(&s, &t, true, Revision::new(1)?, at(100))?;
    let one = Revision::new(1)?;
    let stamp = |entity| Tombstone::new(entity, Generation::INITIAL, one, at(100));
    let kinds = vec![
        EventKind::StickerCreated {
            asset: a.clone(),
            entity: s.clone(),
        },
        EventKind::StickerPatched { entity: s.clone() },
        EventKind::StickerRestored {
            asset: a,
            entity: s.clone(),
        },
        EventKind::StickerDeleted {
            entity: stamp(EntityId::Sticker(s.id())),
        },
        EventKind::CollectionCreated { entity: c.clone() },
        EventKind::CollectionPatched { entity: c.clone() },
        EventKind::CollectionRestored { entity: c.clone() },
        EventKind::CollectionDeleted {
            entity: stamp(EntityId::Collection(c.id())),
        },
        EventKind::TagCreated { entity: t.clone() },
        EventKind::TagPatched { entity: t.clone() },
        EventKind::TagRestored { entity: t.clone() },
        EventKind::TagDeleted {
            entity: stamp(EntityId::Tag(t.id())),
        },
        EventKind::CollectionMembershipSet {
            entity: membership.clone(),
        },
        EventKind::CollectionItemMoved { entity: membership },
        EventKind::TagMembershipSet { entity: tagging },
        EventKind::CollectionItemsReordered {
            collection_id: c.id(),
            items: vec![],
        },
        EventKind::CollectionsReordered { items: vec![] },
    ];
    for kind in kinds {
        roundtrip(&SyncEvent::new(
            EventSeq::new(1)?,
            ID.parse()?,
            ID.parse()?,
            at(100),
            kind,
        )?)?;
    }
    Ok(())
}

#[test]
fn receipts_and_device_metadata_are_typed_and_exclude_credentials() -> TestResult {
    let mut device = Device::new(ID.parse()?, "手机".parse()?, at(100));
    assert!(!device.is_revoked());
    device.revoke(at(200));
    roundtrip(&device)?;
    let value = serde_json::to_value(&device)?;
    assert!(value.get("token").is_none());
    assert!(value.get("token_hash").is_none());
    for outcome in [
        ReceiptOutcome::Accepted {
            event_seq: EventSeq::new(1)?,
            note: Some(ResolutionNote::AnchorMissing),
        },
        ReceiptOutcome::NoOp {
            canonical_revision: Revision::new(1)?,
        },
        ReceiptOutcome::Rejected {
            code: RejectionCode::EntityDeleted,
        },
    ] {
        roundtrip(&OperationReceipt {
            device_id: ID.parse()?,
            op_id: ID.parse()?,
            outcome,
        })?;
    }
    Ok(())
}
