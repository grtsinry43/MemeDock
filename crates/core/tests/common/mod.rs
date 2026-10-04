use memedock_core::LibraryConfig;
use memedock_domain::{
    asset::{Asset, ImageFormat},
    change::{Operation, OperationKind},
    identity::{ContentHash, OperationId},
    sticker::Sticker,
    version::{ByteSize, TimestampMs},
};
use memedock_storage::{LibraryDatabase, files::FsBlobStore};
use sha2::{Digest, Sha256};
use std::{error::Error, num::NonZeroU32, path::Path};
pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;
pub fn config(root: &Path) -> LibraryConfig {
    LibraryConfig::new(root.join("library"), root.join("cache"), root.join("share"))
}
/// Storage fixtures only: core image validation/import is a separate stage.
pub async fn seed(config: &LibraryConfig, bytes: &[u8]) -> TestResult<(Asset, Sticker)> {
    let hash = ContentHash::from_bytes(Sha256::digest(bytes).into());
    let asset = Asset::new(
        hash,
        ByteSize::new(bytes.len() as i64)?,
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
        "猫猫".into(),
        "cat.png".into(),
        TimestampMs::new(100),
    );
    let store = FsBlobStore::open(&config.data_dir)?;
    let mut reader = bytes;
    store.publish(
        store.stage_from(&mut reader, bytes.len() as u64, || false)?,
        || false,
    )?;
    let db = LibraryDatabase::open(config.data_dir.join("library.sqlite")).await?;
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
