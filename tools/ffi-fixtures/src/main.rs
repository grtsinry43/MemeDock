use memedock_domain::{
    asset::{Asset, ImageFormat},
    change::{Operation, OperationKind},
    identity::{ContentHash, OperationId},
    sticker::Sticker,
    version::{ByteSize, TimestampMs},
};
use memedock_storage::{LibraryDatabase, files::FsBlobStore};
use sha2::{Digest, Sha256};
use std::{error::Error, num::NonZeroU32, path::PathBuf};

/// Host-only test data provisioning. Never linked into the product cdylib.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: memedock-ffi-fixtures <new-library-directory>")?,
    );
    if path.exists() {
        return Err("fixture directory must not exist".into());
    }
    let files = FsBlobStore::open(&path)?;
    let db = LibraryDatabase::open(path.join("library.sqlite")).await?;
    for (index, bytes) in [
        b"first fixture original".as_slice(),
        b"second fixture original".as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        let hash = ContentHash::from_bytes(Sha256::digest(bytes).into());
        let at = TimestampMs::new(100 + i64::try_from(index)?);
        let asset = Asset::new(
            hash,
            ByteSize::new(i64::try_from(bytes.len())?)?,
            ImageFormat::Png,
            (
                NonZeroU32::new(10).ok_or("width")?,
                NonZeroU32::new(20).ok_or("height")?,
            ),
            false,
            at,
        );
        let sticker = Sticker::new(
            &asset,
            format!("测试猫猫 {index}"),
            format!("fixture-{index}.png"),
            at,
        );
        let mut reader = bytes;
        files.publish(
            files.stage_from(&mut reader, bytes.len() as u64, || false)?,
            || false,
        )?;
        let mut tx = db.begin_write().await?;
        tx.insert_asset(&asset).await?;
        tx.save_sticker(&sticker).await?;
        tx.append_change(
            OperationId::new(),
            Operation::new(OperationKind::CreateSticker {
                asset,
                title: sticker.title().into(),
                original_name: sticker.original_name().into(),
                note: String::new(),
                starred: false,
            })?,
            at,
        )
        .await?;
        tx.commit().await?;
    }
    db.close().await?;
    Ok(())
}
