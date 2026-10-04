mod common;
use common::*;
use memedock_domain::{
    collection::Collection,
    identity::{CollectionId, TagId},
    lifecycle::Lifecycle,
    tag::{Name, Tag},
    version::{Generation, Revision, TimestampMs},
};
use memedock_ffi::*;

#[tokio::test]
async fn metadata_conversions_preserve_validated_domain_fields() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path())?;
    let (asset, sticker) = seed(&settings, b"metadata").await?;
    let library = open_library(settings).await?;
    let detail = library
        .sticker_detail(sticker.id().to_string())?
        .await_result()
        .await?;
    assert_eq!(detail.asset, AssetMetadata::from(asset));
    assert_eq!(detail.sticker, StickerMetadata::from(sticker));
    assert_eq!(
        library
            .list_stickers(query())?
            .await_result()
            .await?
            .stickers
            .len(),
        1
    );
    let lifecycle = Lifecycle::from_state(
        Generation::new(4)?,
        Revision::new(50)?,
        TimestampMs::new(-10),
        TimestampMs::new(20),
        Some(TimestampMs::new(30)),
    );
    let tag = Tag::from_state(
        TagId::new(),
        Name::new(" ＣＡＴ ".into())?,
        "cat".into(),
        lifecycle.clone(),
    )?;
    let dto = TagMetadata::from(tag);
    assert_eq!(dto.normalized_name, "cat");
    assert_eq!(dto.lifecycle.generation, 4);
    assert_eq!(dto.lifecycle.deleted_at, Some(30));
    let collection = Collection::from_state(
        CollectionId::new(),
        Name::new("收藏".into())?,
        "80".parse()?,
        lifecycle,
    );
    let dto = CollectionMetadata::from(collection);
    assert_eq!(dto.name, "收藏");
    assert_eq!(dto.lifecycle.revision, 50);
    assert_eq!(dto.lifecycle.created_at, -10);
    library.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn boundary_validation_and_errors_do_not_leak_storage_diagnostics() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path())?;
    seed(&settings, b"errors").await?;
    let library = open_library(settings.clone()).await?;
    let mut bad = query();
    bad.tag_ids.push("not-an-id".into());
    assert_eq!(
        library
            .list_stickers(bad)
            .err()
            .ok_or("invalid tag")?
            .code(),
        ErrorCode::InvalidInput
    );
    let mut bad = query();
    bad.page_size = 0;
    assert_eq!(
        library
            .list_stickers(bad)?
            .await_result()
            .await
            .err()
            .ok_or("invalid page size")?
            .code(),
        ErrorCode::InvalidInput
    );
    assert_eq!(
        library
            .cancel_task("invalid".into())
            .err()
            .ok_or("task id")?
            .code(),
        ErrorCode::InvalidInput
    );
    let missing = memedock_domain::identity::StickerId::new(
        memedock_domain::identity::ContentHash::from_bytes([0; 32]),
    )
    .to_string();
    let error = library
        .sticker_detail(missing)?
        .await_result()
        .await
        .err()
        .ok_or("missing sticker")?;
    assert_eq!(error.code(), ErrorCode::NotFound);
    assert!(!error.to_string().contains(&settings.data_dir));
    let mut bad = settings;
    bad.data_dir.push('\0');
    assert_eq!(
        open_library(bad).await.err().ok_or("null path")?.code(),
        ErrorCode::InvalidInput
    );
    library.shutdown().await?;
    Ok(())
}
