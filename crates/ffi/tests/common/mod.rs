use memedock_domain::{
    asset::{Asset, ImageFormat},
    change::{Operation, OperationKind},
    identity::{ContentHash, OperationId},
    sticker::Sticker,
    version::{ByteSize, TimestampMs},
};
use memedock_ffi::{
    LibraryConfiguration, StickerQuery, StickerSort, default_resource_configuration, new_request_id,
};
use memedock_storage::{LibraryDatabase, files::FsBlobStore};
use sha2::{Digest, Sha256};
use std::{error::Error, num::NonZeroU32, path::Path};
pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;
pub fn config(root: &Path) -> TestResult<LibraryConfiguration> {
    Ok(LibraryConfiguration {
        data_dir: root.join("data").to_str().ok_or("data path")?.into(),
        cache_dir: root.join("cache").to_str().ok_or("cache path")?.into(),
        export_dir: root.join("share").to_str().ok_or("share path")?.into(),
        limits: default_resource_configuration()?,
    })
}
pub fn query() -> StickerQuery {
    StickerQuery {
        request_id: new_request_id(),
        text: String::new(),
        collection_id: None,
        tag_ids: vec![],
        starred: None,
        deleted: false,
        sort: StickerSort::Recent,
        page_size: 1,
        cursor: None,
    }
}
/// Metadata/hash fixture only; this stage does not decode or import images.
pub async fn seed(config: &LibraryConfiguration, bytes: &[u8]) -> TestResult<(Asset, Sticker)> {
    let hash = ContentHash::from_bytes(Sha256::digest(bytes).into());
    let asset = Asset::new(
        hash,
        ByteSize::new(i64::try_from(bytes.len())?)?,
        ImageFormat::Png,
        (
            NonZeroU32::new(10).ok_or("width")?,
            NonZeroU32::new(20).ok_or("height")?,
        ),
        false,
        TimestampMs::new(100),
    );
    let sticker = Sticker::new(
        &asset,
        "测试猫猫".into(),
        "猫.png".into(),
        TimestampMs::new(100),
    );
    let files = FsBlobStore::open(&config.data_dir)?;
    let mut reader = bytes;
    files.publish(
        files.stage_from(&mut reader, bytes.len() as u64, || false)?,
        || false,
    )?;
    let db = LibraryDatabase::open(Path::new(&config.data_dir).join("library.sqlite")).await?;
    let mut tx = db.begin_write().await?;
    tx.insert_asset(&asset).await?;
    tx.save_sticker(&sticker).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::CreateSticker {
            asset: asset.clone(),
            title: sticker.title().into(),
            original_name: sticker.original_name().into(),
            note: String::new(),
            starred: false,
        })?,
        TimestampMs::new(100),
    )
    .await?;
    tx.commit().await?;
    db.close().await?;
    Ok((asset, sticker))
}
