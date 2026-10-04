mod common;
use common::*;
use memedock_domain::{
    change::{Operation, OperationKind},
    collection::Collection,
    identity::{CollectionId, OperationId, TagId},
    lifecycle::{EntityId, Tombstone},
    ordering::SortKey,
    relation::{CollectionItem, StickerTag},
    tag::{Name, Tag},
    version::{Generation, Revision, TimestampMs},
};
use memedock_storage::{
    LibraryDatabase, StorageError,
    queries::{StickerQuery, StickerSort},
};

#[tokio::test]
async fn search_handles_short_chinese_normalization_and_sql_metacharacters() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (a, s) = fixture(b"a", "猫猫 ＡＢＣ 100% ' OR 1=1 --", 100)?;
    insert(&db, &a, &s).await?;
    let (a, t) = fixture(b"b", "狗狗", 101)?;
    insert(&db, &a, &t).await?;
    for text in ["猫", "猫猫", "abc", "猫 abc", "100%", "' OR 1=1 --"] {
        let page = db
            .stickers(
                &StickerQuery {
                    text: text.into(),
                    ..Default::default()
                },
                10,
                None,
            )
            .await?;
        assert_eq!(page.stickers, vec![s.clone()], "{text}");
    }
    assert!(
        db.stickers(
            &StickerQuery {
                text: "猫 狗".into(),
                ..Default::default()
            },
            10,
            None
        )
        .await?
        .stickers
        .is_empty()
    );
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn keyset_pagination_is_stable_and_query_bound() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let mut expected = Vec::new();
    for byte in 0..5_u8 {
        let (a, s) = fixture(&[byte], "猫", 100)?;
        expected.push(s.id());
        insert(&db, &a, &s).await?;
    }
    expected.sort_unstable_by(|a, b| b.cmp(a));
    let query = StickerQuery::default();
    let mut cursor = None;
    let mut actual = Vec::new();
    loop {
        let page = db.stickers(&query, 2, cursor.as_ref()).await?;
        actual.extend(page.stickers.into_iter().map(|s| s.id()));
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(actual, expected);
    let cursor = db.stickers(&query, 1, None).await?.next.ok_or("cursor")?;
    assert!(matches!(
        db.stickers(
            &StickerQuery {
                text: "狗".into(),
                ..Default::default()
            },
            1,
            Some(&cursor)
        )
        .await,
        Err(StorageError::InvalidInput(_))
    ));
    assert!(db.stickers(&query, 0, None).await.is_err());
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn restore_does_not_reactivate_old_collection_or_tag_generations() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (asset, mut sticker) = fixture(b"restore", "猫", 100)?;
    insert(&db, &asset, &sticker).await?;
    let collection = Collection::new(
        CollectionId::new(),
        Name::new("收藏".into())?,
        SortKey::default(),
        TimestampMs::new(100),
    );
    let tag = Tag::new(
        TagId::new(),
        Name::new("宠物".into())?,
        TimestampMs::new(100),
    );
    let item = CollectionItem::new(
        &collection,
        &sticker,
        SortKey::default(),
        true,
        Revision::LOCAL,
        TimestampMs::new(100),
    )?;
    let relation = StickerTag::new(&sticker, &tag, true, Revision::LOCAL, TimestampMs::new(100))?;
    let mut tx = db.begin_write().await?;
    tx.save_collection(&collection).await?;
    tx.save_tag(&tag).await?;
    tx.save_collection_item(&item).await?;
    tx.save_sticker_tag(&relation).await?;
    for op in [
        OperationKind::CreateCollection {
            collection_id: collection.id(),
            name: collection.name().clone(),
            before_id: None,
        },
        OperationKind::CreateTag {
            tag_id: tag.id(),
            name: tag.name().clone(),
        },
        OperationKind::SetCollectionMembership {
            collection_id: collection.id(),
            sticker_id: sticker.id(),
            collection_generation: Generation::INITIAL,
            sticker_generation: Generation::INITIAL,
            present: true,
        },
        OperationKind::SetTagMembership {
            sticker_id: sticker.id(),
            tag_id: tag.id(),
            sticker_generation: Generation::INITIAL,
            tag_generation: Generation::INITIAL,
            present: true,
        },
    ] {
        tx.append_change(
            OperationId::new(),
            Operation::new(op)?,
            TimestampMs::new(100),
        )
        .await?;
    }
    tx.commit().await?;
    let query = StickerQuery {
        collection: Some(collection.id()),
        sort: StickerSort::CollectionOrder,
        ..Default::default()
    };
    assert_eq!(db.stickers(&query, 10, None).await?.stickers.len(), 1);
    assert_eq!(db.sticker_tags(sticker.id()).await?, vec![tag.clone()]);
    assert_eq!(
        db.stickers(
            &StickerQuery {
                text: "宠".into(),
                ..Default::default()
            },
            10,
            None
        )
        .await?
        .stickers
        .len(),
        1
    );
    sticker.delete(Generation::INITIAL, TimestampMs::new(200), Revision::LOCAL)?;
    let tombstone = Tombstone::new(
        EntityId::Sticker(sticker.id()),
        Generation::INITIAL,
        Revision::LOCAL,
        TimestampMs::new(200),
    );
    let mut tx = db.begin_write().await?;
    tx.save_sticker(&sticker).await?;
    tx.save_tombstone(&tombstone).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::DeleteSticker {
            sticker_id: sticker.id(),
            generation: Generation::INITIAL,
        })?,
        TimestampMs::new(200),
    )
    .await?;
    tx.commit().await?;
    assert_eq!(
        db.stickers(
            &StickerQuery {
                deleted: true,
                ..Default::default()
            },
            10,
            None
        )
        .await?
        .stickers,
        vec![sticker.clone()]
    );
    sticker.restore(Revision::LOCAL, TimestampMs::new(300), Revision::LOCAL)?;
    let mut tx = db.begin_write().await?;
    tx.save_sticker(&sticker).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::RestoreSticker {
            sticker_id: sticker.id(),
            expected_deleted_revision: Revision::LOCAL,
            rebuild: None,
        })?,
        TimestampMs::new(300),
    )
    .await?;
    tx.commit().await?;
    assert!(db.stickers(&query, 10, None).await?.stickers.is_empty());
    assert!(db.sticker_tags(sticker.id()).await?.is_empty());
    assert!(
        db.stickers(
            &StickerQuery {
                text: "宠".into(),
                ..Default::default()
            },
            10,
            None
        )
        .await?
        .stickers
        .is_empty()
    );
    assert_eq!(
        db.collection_item(collection.id(), sticker.id()).await?,
        Some(item)
    );
    assert_eq!(
        db.sticker_tag(sticker.id(), tag.id()).await?,
        Some(relation)
    );
    assert_eq!(
        db.tombstone(EntityId::Sticker(sticker.id())).await?,
        Some(tombstone)
    );
    assert_eq!(db.collections(false).await?, vec![collection.clone()]);
    assert_eq!(db.tags(false).await?, vec![tag.clone()]);
    // Explicit re-add binds to sticker generation 1 and becomes effective.
    let item = CollectionItem::new(
        &collection,
        &sticker,
        SortKey::default(),
        true,
        Revision::LOCAL,
        TimestampMs::new(310),
    )?;
    let relation = StickerTag::new(&sticker, &tag, true, Revision::LOCAL, TimestampMs::new(310))?;
    let mut tx = db.begin_write().await?;
    tx.save_collection_item(&item).await?;
    tx.save_sticker_tag(&relation).await?;
    for op in [
        OperationKind::SetCollectionMembership {
            collection_id: collection.id(),
            sticker_id: sticker.id(),
            collection_generation: Generation::INITIAL,
            sticker_generation: sticker.lifecycle().generation(),
            present: true,
        },
        OperationKind::SetTagMembership {
            sticker_id: sticker.id(),
            tag_id: tag.id(),
            sticker_generation: sticker.lifecycle().generation(),
            tag_generation: Generation::INITIAL,
            present: true,
        },
    ] {
        tx.append_change(
            OperationId::new(),
            Operation::new(op)?,
            TimestampMs::new(310),
        )
        .await?;
    }
    tx.commit().await?;
    assert_eq!(db.stickers(&query, 10, None).await?.stickers.len(), 1);
    assert_eq!(db.sticker_tags(sticker.id()).await?.len(), 1);
    // Restoring the other endpoints also invalidates the historical relation.
    let mut collection = collection;
    let mut tag = tag;
    collection.delete(Generation::INITIAL, TimestampMs::new(400), Revision::LOCAL)?;
    tag.delete(Generation::INITIAL, TimestampMs::new(400), Revision::LOCAL)?;
    let mut tx = db.begin_write().await?;
    tx.save_collection(&collection).await?;
    tx.save_tag(&tag).await?;
    for op in [
        OperationKind::DeleteCollection {
            collection_id: collection.id(),
            generation: Generation::INITIAL,
        },
        OperationKind::DeleteTag {
            tag_id: tag.id(),
            generation: Generation::INITIAL,
        },
    ] {
        tx.append_change(
            OperationId::new(),
            Operation::new(op)?,
            TimestampMs::new(400),
        )
        .await?;
    }
    tx.commit().await?;
    assert!(db.stickers(&query, 10, None).await?.stickers.is_empty());
    assert!(db.sticker_tags(sticker.id()).await?.is_empty());
    collection.restore(Revision::LOCAL, TimestampMs::new(500), Revision::LOCAL)?;
    tag.restore(Revision::LOCAL, TimestampMs::new(500), Revision::LOCAL)?;
    let mut tx = db.begin_write().await?;
    tx.save_collection(&collection).await?;
    tx.save_tag(&tag).await?;
    for op in [
        OperationKind::RestoreCollection {
            collection_id: collection.id(),
            expected_deleted_revision: Revision::LOCAL,
            rebuild: None,
        },
        OperationKind::RestoreTag {
            tag_id: tag.id(),
            expected_deleted_revision: Revision::LOCAL,
            rebuild: None,
        },
    ] {
        tx.append_change(
            OperationId::new(),
            Operation::new(op)?,
            TimestampMs::new(500),
        )
        .await?;
    }
    tx.commit().await?;
    assert!(db.stickers(&query, 10, None).await?.stickers.is_empty());
    assert!(db.sticker_tags(sticker.id()).await?.is_empty());
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn collection_and_last_used_pages_keep_binary_order_and_ties() -> TestResult {
    use memedock_domain::local::LocalUsage;
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let collection = Collection::new(
        CollectionId::new(),
        Name::new("排序".into())?,
        SortKey::default(),
        TimestampMs::new(100),
    );
    let mut tx = db.begin_write().await?;
    tx.save_collection(&collection).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::CreateCollection {
            collection_id: collection.id(),
            name: collection.name().clone(),
            before_id: None,
        })?,
        TimestampMs::new(100),
    )
    .await?;
    tx.commit().await?;
    let keys = SortKey::rebalance(4)?;
    let mut expected = Vec::new();
    for (byte, key) in keys.into_iter().enumerate() {
        let (asset, sticker) = fixture(&[byte as u8], "排序", 100)?;
        insert(&db, &asset, &sticker).await?;
        let item = CollectionItem::new(
            &collection,
            &sticker,
            key,
            true,
            Revision::LOCAL,
            TimestampMs::new(100),
        )?;
        let mut tx = db.begin_write().await?;
        tx.save_collection_item(&item).await?;
        tx.append_change(
            OperationId::new(),
            Operation::new(OperationKind::SetCollectionMembership {
                collection_id: collection.id(),
                sticker_id: sticker.id(),
                collection_generation: Generation::INITIAL,
                sticker_generation: Generation::INITIAL,
                present: true,
            })?,
            TimestampMs::new(100),
        )
        .await?;
        // The final two deliberately tie on last-use time; the first is unused.
        if byte > 0 {
            tx.save_local_usage(&LocalUsage::first_use(
                sticker.id(),
                TimestampMs::new(if byte == 1 { 200 } else { 300 }),
            ))
            .await?;
        }
        tx.commit().await?;
        expected.push(sticker.id());
    }
    let query = StickerQuery {
        collection: Some(collection.id()),
        sort: StickerSort::CollectionOrder,
        ..Default::default()
    };
    let mut cursor = None;
    let mut actual = Vec::new();
    loop {
        let page = db.stickers(&query, 1, cursor.as_ref()).await?;
        actual.extend(page.stickers.into_iter().map(|s| s.id()));
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(actual, expected);
    let mut expected_used = expected[2..].to_vec();
    expected_used.sort_unstable_by(|a, b| b.cmp(a));
    expected_used.extend([expected[1], expected[0]]);
    let query = StickerQuery {
        sort: StickerSort::LastUsed,
        ..Default::default()
    };
    let mut actual = Vec::new();
    let mut cursor = None;
    loop {
        let page = db.stickers(&query, 2, cursor.as_ref()).await?;
        actual.extend(page.stickers.into_iter().map(|s| s.id()));
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(actual, expected_used);
    let mut hashes = Vec::new();
    let mut after = None;
    loop {
        let page = db.asset_hashes_after(after, 2).await?;
        if page.is_empty() {
            break;
        }
        after = page.last().copied();
        hashes.extend(page);
    }
    let mut expected_hashes: Vec<_> = expected.into_iter().map(|id| id.content_hash()).collect();
    expected_hashes.sort_unstable();
    assert_eq!(hashes, expected_hashes);
    db.close().await?;
    Ok(())
}
