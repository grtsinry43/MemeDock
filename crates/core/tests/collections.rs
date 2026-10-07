mod common;
use common::*;
use memedock_core::{ErrorCode, Library, QueryRequest, StickerQuery, StickerSort};
use memedock_domain::{
    collection::Collection,
    identity::CollectionId,
    lifecycle::Lifecycle,
    ordering::SortKey,
    version::{Revision, TimestampMs},
};
use memedock_domain::{tag::Name, version::Generation};
use memedock_storage::LibraryDatabase;

#[tokio::test]
async fn collided_order_is_repaired_atomically_without_resetting_known_revisions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let config = config(directory.path());
    let (_, sticker) = seed(&config, b"collision original").await?;
    let db = LibraryDatabase::open(config.data_dir.join("library.sqlite")).await?;
    let known = Revision::new(42)?;
    let at = TimestampMs::new(100);
    let life = Lifecycle::from_state(Generation::INITIAL, known, at, at, None);
    let mut values = vec![
        Collection::from_state(
            CollectionId::new(),
            Name::new("A".into())?,
            SortKey::default(),
            life.clone(),
        ),
        Collection::from_state(
            CollectionId::new(),
            Name::new("B".into())?,
            SortKey::default(),
            life,
        ),
    ];
    values.sort_by_key(Collection::id);
    let mut tx = db.begin_write().await?;
    for value in &values {
        tx.save_collection(value).await?;
        tx.append_change(
            memedock_domain::identity::OperationId::new(),
            memedock_domain::change::Operation::new(
                memedock_domain::change::OperationKind::CreateCollection {
                    collection_id: value.id(),
                    name: value.name().clone(),
                    before_id: None,
                },
            )?,
            at,
        )
        .await?;
    }
    tx.commit().await?;
    db.close().await?;
    let library = Library::open(config.clone()).await?;
    let created = library
        .create_collection(Name::new("Middle".into())?, Some(values[1].id()))?
        .wait()
        .await?;
    let ordered = library.collections(false)?.wait().await?;
    assert_eq!(
        ordered.iter().map(Collection::id).collect::<Vec<_>>(),
        vec![values[0].id(), created.id(), values[1].id()]
    );
    assert!(
        ordered
            .windows(2)
            .all(|pair| pair[0].sort_key() < pair[1].sort_key())
    );
    assert_eq!(ordered[0].lifecycle().revision(), known);
    library
        .set_sticker_organization(
            sticker.id(),
            Generation::INITIAL,
            Some(Some((created.id(), Generation::INITIAL))),
            None,
        )?
        .wait()
        .await?;
    library
        .delete_collection(created.id(), Generation::INITIAL)?
        .wait()
        .await?;
    library
        .delete_collection(created.id(), Generation::INITIAL)?
        .wait()
        .await?;
    library.close().await?;
    let db = LibraryDatabase::open(config.data_dir.join("library.sqlite")).await?;
    // Three fixtures, two repair moves, create, membership, one idempotent delete.
    assert_eq!(db.changes_after(None, 100).await?.len(), 8);
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn collection_and_member_order_use_real_anchors_and_reject_stale_targets() -> TestResult {
    let directory = tempfile::tempdir()?;
    let config = config(directory.path());
    let (_, a) = seed(&config, b"first original").await?;
    let (_, b) = seed(&config, b"second original").await?;
    let library = Library::open(config).await?;
    let first = library
        .create_collection(Name::new("First".into())?, None)?
        .wait()
        .await?;
    let second = library
        .create_collection(Name::new("Second".into())?, None)?
        .wait()
        .await?;
    library
        .move_collection(second.id(), Generation::INITIAL, Some(first.id()))?
        .wait()
        .await?;
    assert_eq!(
        library.collections(false)?.wait().await?[0].id(),
        second.id()
    );
    for s in [&a, &b] {
        library
            .set_sticker_organization(
                s.id(),
                Generation::INITIAL,
                Some(Some((first.id(), Generation::INITIAL))),
                None,
            )?
            .wait()
            .await?;
    }
    library
        .move_collection_item(
            first.id(),
            Generation::INITIAL,
            b.id(),
            Generation::INITIAL,
            Some(a.id()),
        )?
        .wait()
        .await?;
    let page = library
        .list_stickers(QueryRequest {
            request_id: memedock_core::RequestId::new(),
            query: StickerQuery {
                collection: Some(first.id()),
                sort: StickerSort::CollectionOrder,
                ..Default::default()
            },
            page_size: 60,
            cursor: None,
        })?
        .wait()
        .await?;
    assert_eq!(page.stickers[0].id(), b.id());
    let error = library
        .move_collection_item(
            first.id(),
            Generation::INITIAL,
            a.id(),
            Generation::new(1)?,
            None,
        )?
        .wait()
        .await
        .err()
        .ok_or("error expected")?;
    assert_eq!(error.code(), ErrorCode::Conflict);
    library
        .delete_collection(first.id(), Generation::INITIAL)?
        .wait()
        .await?;
    assert!(
        library
            .sticker_detail(a.id())?
            .wait()
            .await?
            .sticker
            .lifecycle()
            .deleted_at()
            .is_none()
    );
    assert!(
        library
            .sticker_detail(a.id())?
            .wait()
            .await?
            .collection
            .is_none()
    );
    library.close().await?;
    Ok(())
}
