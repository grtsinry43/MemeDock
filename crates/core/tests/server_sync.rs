use memedock_core::server_sync::push;
use memedock_domain::{
    change::{NamePatch, Operation, OperationKind},
    identity::{CollectionId, DeviceId, OperationId, TagId},
    sync::{EventKind, OperationEnvelope, ReceiptOutcome, RejectionCode},
    version::{Generation, Revision, TimestampMs},
};
use memedock_storage::server::ServerDatabase;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn concurrency_ordering_and_restoration_keep_server_authority() -> Result {
    let dir = tempfile::tempdir()?;
    let db = ServerDatabase::open(dir.path().join("server.sqlite")).await?;
    let device = DeviceId::new();
    db.create_device(&device.to_string(), "test", &"0".repeat(64), 0)
        .await?;
    let info = db.sync_identity().await?;
    let envelope = |kind| -> Result<OperationEnvelope> {
        Ok(OperationEnvelope::new(
            info.library_id,
            info.epoch,
            device,
            OperationId::new(),
            TimestampMs::new(100),
            Operation::new(kind)?,
        ))
    };
    let tag = TagId::new();
    let create = envelope(OperationKind::CreateTag {
        tag_id: tag,
        name: "initial".parse()?,
    })?;
    let (a, b) = tokio::join!(
        push(&db, device, create.clone(), TimestampMs::new(0)),
        push(&db, device, create, TimestampMs::new(1))
    );
    assert_eq!(a?, b?);
    assert_eq!(db.sync_identity().await?.head.get(), 1);
    let first = envelope(OperationKind::PatchTag {
        tag_id: tag,
        generation: Generation::INITIAL,
        patch: NamePatch::new("a".parse()?),
    })?;
    let second = envelope(OperationKind::PatchTag {
        tag_id: tag,
        generation: Generation::INITIAL,
        patch: NamePatch::new("b".parse()?),
    })?;
    let (a, b) = tokio::join!(
        push(&db, device, first, TimestampMs::new(9000)),
        push(&db, device, second, TimestampMs::new(1))
    );
    let seq = |outcome| -> Result<i64> {
        match outcome {
            ReceiptOutcome::Accepted { event_seq, .. } => Ok(event_seq.get()),
            _ => Err("acceptance missing".into()),
        }
    };
    let a = seq(a?.outcome)?;
    let b = seq(b?.outcome)?;
    assert_ne!(a, b);
    let page = db
        .pull_events(device, info.library_id, info.epoch, Revision::LOCAL, 256)
        .await?;
    match page.events().last().ok_or("event missing")?.event() {
        EventKind::TagPatched { entity } => {
            assert_eq!(entity.name().as_str(), if a > b { "a" } else { "b" })
        }
        _ => return Err("wrong event".into()),
    }
    let c1 = CollectionId::new();
    let c2 = CollectionId::new();
    for (id, before) in [(c1, None), (c2, Some(c1))] {
        push(
            &db,
            device,
            envelope(OperationKind::CreateCollection {
                collection_id: id,
                name: "collection".parse()?,
                before_id: before,
            })?,
            TimestampMs::new(0),
        )
        .await?;
    }
    let before = db.begin_sync(device).await?;
    let v1 = before
        .record::<memedock_domain::collection::Collection>("collection", &c1.to_string())
        .await?
        .ok_or("collection missing")?;
    let v2 = before
        .record::<memedock_domain::collection::Collection>("collection", &c2.to_string())
        .await?
        .ok_or("collection missing")?;
    assert!(v2.sort_key() < v1.sort_key());
    drop(before);
    let deleted = seq(push(
        &db,
        device,
        envelope(OperationKind::DeleteCollection {
            collection_id: c1,
            generation: Generation::INITIAL,
        })?,
        TimestampMs::new(0),
    )
    .await?
    .outcome)?;
    let stale = envelope(OperationKind::PatchCollection {
        collection_id: c1,
        generation: Generation::INITIAL,
        patch: NamePatch::new("late".parse()?),
    })?;
    assert!(matches!(
        push(&db, device, stale, TimestampMs::new(0)).await?.outcome,
        ReceiptOutcome::Rejected {
            code: RejectionCode::EntityDeleted
        }
    ));
    let restored = push(
        &db,
        device,
        envelope(OperationKind::RestoreCollection {
            collection_id: c1,
            expected_deleted_revision: Revision::new(deleted)?,
            rebuild: None,
        })?,
        TimestampMs::new(0),
    )
    .await?;
    assert!(matches!(restored.outcome, ReceiptOutcome::Accepted { .. }));
    assert!(matches!(
        push(
            &db,
            device,
            envelope(OperationKind::MoveCollection {
                collection_id: c1,
                generation: Generation::INITIAL,
                before_id: None
            })?,
            TimestampMs::new(0)
        )
        .await?
        .outcome,
        ReceiptOutcome::Rejected {
            code: RejectionCode::StaleGeneration
        }
    ));
    db.close().await?;
    Ok(())
}
