mod common;
use common::*;
use memedock_domain::{
    change::{FieldPatch, Operation, OperationKind, StickerPatch},
    identity::OperationId,
    local::{LocalAsset, LocalUsage},
    version::{Generation, Revision, TimestampMs},
};
use memedock_storage::{LibraryDatabase, StorageError, files::FsBlobStore};
use std::collections::HashSet;

#[tokio::test]
async fn transactional_usage_reads_prevent_lost_concurrent_increments() -> TestResult {
    use memedock_domain::local::UsageAction;
    let dir = tempfile::tempdir()?;
    let db = std::sync::Arc::new(LibraryDatabase::open(dir.path().join("library.sqlite")).await?);
    let (asset, sticker) = fixture(b"usage transaction", "使用", 100)?;
    insert(&db, &asset, &sticker).await?;
    let mut jobs = tokio::task::JoinSet::new();
    for index in 0..20 {
        let db = db.clone();
        let id = sticker.id();
        jobs.spawn(async move {
            let mut tx = db.begin_write().await?;
            let at = TimestampMs::new(200 + index);
            let usage = match tx.local_usage(id).await? {
                Some(mut usage) => {
                    usage.record(at, UsageAction::CopyImage)?;
                    usage
                }
                None => LocalUsage::first_use(id, at),
            };
            tx.save_local_usage(&usage).await?;
            tx.commit().await
        });
    }
    while let Some(result) = jobs.join_next().await {
        result??;
    }
    assert_eq!(
        db.local_usage(sticker.id())
            .await?
            .ok_or("usage")?
            .use_count(),
        20
    );
    assert_eq!(db.changes_after(None, 100).await?.len(), 1);
    let db = std::sync::Arc::try_unwrap(db).map_err(|_| "database still shared")?;
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn dropped_and_explicitly_rolled_back_transactions_leave_no_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (asset, sticker) = fixture(b"rollback", "测试", 100)?;
    let mut tx = db.begin_write().await?;
    tx.insert_asset(&asset).await?;
    tx.save_sticker(&sticker).await?;
    tx.append_change(
        OperationId::new(),
        create_operation(&asset, &sticker)?,
        TimestampMs::new(100),
    )
    .await?;
    drop(tx);
    assert!(db.asset(asset.hash()).await?.is_none());
    assert!(db.changes_after(None, 10).await?.is_empty());
    let mut tx = db.begin_write().await?;
    tx.insert_asset(&asset).await?;
    tx.rollback().await?;
    assert!(db.asset(asset.hash()).await?.is_none());
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn business_changes_cannot_commit_without_log() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (asset, _) = fixture(b"unlogged", "", 100)?;
    let mut tx = db.begin_write().await?;
    tx.insert_asset(&asset).await?;
    assert!(matches!(tx.commit().await, Err(StorageError::Conflict(_))));
    assert!(db.asset(asset.hash()).await?.is_none());
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn log_failure_rolls_back_metadata_and_keeps_published_original() -> TestResult {
    let dir = tempfile::tempdir()?;
    let files = FsBlobStore::open(dir.path())?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let bytes = b"original bytes";
    let (asset, mut sticker) = fixture(bytes, "初始", 100)?;
    let staged = files.stage_from(&mut bytes.as_slice(), 1024, || false)?;
    files.publish(staged, || false)?;
    insert(&db, &asset, &sticker).await?;
    let op_id = db.changes_after(None, 1).await?.remove(0).op_id();
    let mut tx = db.begin_write().await?;
    let patch = StickerPatch::new(
        FieldPatch::Set("改变".into()),
        FieldPatch::Missing,
        FieldPatch::Missing,
    )?;
    sticker.patch(
        Generation::INITIAL,
        &patch,
        TimestampMs::new(200),
        Revision::LOCAL,
    )?;
    tx.save_sticker(&sticker).await?;
    assert!(
        tx.append_change(
            op_id,
            create_operation(&asset, &sticker)?,
            TimestampMs::new(200)
        )
        .await
        .is_err()
    );
    tx.rollback().await?;
    assert_eq!(
        db.sticker(sticker.id()).await?.ok_or("sticker")?.title(),
        "初始"
    );
    files.verify(asset.hash(), bytes.len() as u64, || false)?;
    // A new file published before a failed DB transaction remains an orphan.
    let bytes = b"uncommitted bytes";
    let (other, _) = fixture(bytes, "", 100)?;
    files.publish(
        files.stage_from(&mut bytes.as_slice(), 1024, || false)?,
        || false,
    )?;
    let mut tx = db.begin_write().await?;
    tx.insert_asset(&other).await?;
    tx.rollback().await?;
    let scan = files.scan_recovery(&HashSet::from([asset.hash()]))?;
    assert_eq!(scan.orphan_originals, vec![other.hash()]);
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn dedup_preserves_first_asset_and_edited_sticker() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (asset, mut sticker) = fixture(b"same", "原始", 100)?;
    insert(&db, &asset, &sticker).await?;
    let patch = StickerPatch::new(
        FieldPatch::Set("用户修改".into()),
        FieldPatch::Missing,
        FieldPatch::Missing,
    )?;
    sticker.patch(
        Generation::INITIAL,
        &patch,
        TimestampMs::new(200),
        Revision::LOCAL,
    )?;
    let mut tx = db.begin_write().await?;
    tx.save_sticker(&sticker).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::PatchSticker {
            sticker_id: sticker.id(),
            generation: Generation::INITIAL,
            patch,
        })?,
        TimestampMs::new(200),
    )
    .await?;
    tx.commit().await?;
    let (later, _) = fixture(b"same", "导入标题", 300)?;
    let mut tx = db.begin_write().await?;
    assert!(!tx.insert_asset(&later).await?);
    tx.commit().await?;
    assert_eq!(db.asset(asset.hash()).await?, Some(asset));
    assert_eq!(db.sticker(sticker.id()).await?, Some(sticker));
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn local_states_and_outbox_transitions_survive_restart() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("library.sqlite");
    let db = LibraryDatabase::open(&path).await?;
    let (asset, sticker) = fixture(b"state", "状态", 100)?;
    insert(&db, &asset, &sticker).await?;
    let mut state = LocalAsset::new(asset.hash());
    state.verified(TimestampMs::new(100));
    state.begin_thumbnail()?;
    let usage = LocalUsage::first_use(sticker.id(), TimestampMs::new(150));
    let mut change = db.changes_after(None, 1).await?.remove(0);
    change.begin_send()?;
    let mut tx = db.begin_write().await?;
    tx.save_local_asset(&state).await?;
    tx.save_local_usage(&usage).await?;
    tx.save_change(&change).await?;
    tx.commit().await?;
    db.close().await?;
    let db = LibraryDatabase::open(&path).await?;
    assert_eq!(db.local_asset(asset.hash()).await?, Some(state));
    assert_eq!(db.local_usage(sticker.id()).await?, Some(usage));
    assert_eq!(db.changes_after(None, 1).await?, vec![change]);
    assert_eq!(
        db.space_statistics().await?.ready_original_bytes,
        asset.byte_size().get()
    );
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn failed_write_poisoning_prevents_partial_commit_even_with_a_log() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (asset, sticker) = fixture(b"poison", "事务", 100)?;
    let mut tx = db.begin_write().await?;
    tx.insert_asset(&asset).await?;
    tx.save_sticker(&sticker).await?;
    let id = OperationId::new();
    tx.append_change(
        id,
        create_operation(&asset, &sticker)?,
        TimestampMs::new(100),
    )
    .await?;
    assert!(
        tx.append_change(
            id,
            create_operation(&asset, &sticker)?,
            TimestampMs::new(100)
        )
        .await
        .is_err()
    );
    assert!(matches!(tx.commit().await, Err(StorageError::Conflict(_))));
    assert!(db.asset(asset.hash()).await?.is_none());
    assert!(db.changes_after(None, 10).await?.is_empty());
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn frozen_outbox_payload_cannot_be_replaced_and_ack_state_persists() -> TestResult {
    use memedock_domain::{identity::TagId, local::LocalChange, tag::Name};
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (asset, sticker) = fixture(b"freeze", "冻结", 100)?;
    insert(&db, &asset, &sticker).await?;
    let mut change = db.changes_after(None, 1).await?.remove(0);
    change.begin_send()?;
    let mut tx = db.begin_write().await?;
    tx.save_change(&change).await?;
    tx.commit().await?;
    let replacement = Operation::new(OperationKind::CreateTag {
        tag_id: TagId::new(),
        name: Name::new("伪替换".into())?,
    })?;
    let mut forged = LocalChange::new(
        change.local_order(),
        change.op_id(),
        replacement,
        change.created_at(),
    );
    forged.begin_send()?;
    let mut tx = db.begin_write().await?;
    assert!(matches!(
        tx.save_change(&forged).await,
        Err(StorageError::Conflict(_))
    ));
    tx.rollback().await?;
    change.accepted(Revision::new(10)?)?;
    let mut tx = db.begin_write().await?;
    tx.save_change(&change).await?;
    tx.commit().await?;
    assert_eq!(db.changes_after(None, 1).await?, vec![change]);
    db.close().await?;
    Ok(())
}
