use memedock_domain::{
    asset::{Asset, ImageFormat},
    change::{Operation, OperationKind},
    identity::{ContentHash, OperationId},
    sticker::Sticker,
    version::{ByteSize, TimestampMs},
};
use memedock_storage::LibraryDatabase;
use sha2::{Digest, Sha256};
use std::{error::Error, num::NonZeroU32};

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;
pub fn fixture(bytes: &[u8], title: &str, at: i64) -> TestResult<(Asset, Sticker)> {
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
        TimestampMs::new(at),
    );
    let sticker = Sticker::new(
        &asset,
        title.into(),
        "original.png".into(),
        TimestampMs::new(at),
    );
    Ok((asset, sticker))
}
pub fn create_operation(asset: &Asset, sticker: &Sticker) -> TestResult<Operation> {
    Ok(Operation::new(OperationKind::CreateSticker {
        asset: asset.clone(),
        title: sticker.title().into(),
        original_name: sticker.original_name().into(),
        note: sticker.note().into(),
        starred: sticker.starred(),
    })?)
}
pub async fn insert(db: &LibraryDatabase, asset: &Asset, sticker: &Sticker) -> TestResult {
    let mut tx = db.begin_write().await?;
    tx.insert_asset(asset).await?;
    tx.save_sticker(sticker).await?;
    tx.append_change(
        OperationId::new(),
        create_operation(asset, sticker)?,
        TimestampMs::new(100),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
