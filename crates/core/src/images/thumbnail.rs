use crate::{ResourceLimits, Result, images::inspect, tasks::TaskControl};
use memedock_domain::identity::ContentHash;
use memedock_storage::files::{DerivedStore, FsBlobStore};
use std::path::PathBuf;

pub(crate) fn cached(cache: &DerivedStore, hash: ContentHash) -> Result<bool> {
    if !cache.ready(hash)? {
        return Ok(false);
    }
    let path = cache.thumbnail_path(hash);
    let file = std::fs::File::open(path)?;
    let mut reader =
        image::ImageReader::with_format(std::io::BufReader::new(file), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(256);
    limits.max_image_height = Some(256);
    limits.max_alloc = Some(1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode().is_ok())
}

pub(crate) fn generate(
    blobs: &FsBlobStore,
    cache: &DerivedStore,
    hash: ContentHash,
    limits: &ResourceLimits,
    control: &TaskControl,
) -> Result<PathBuf> {
    let decoded = inspect::decode(blobs.open_original(hash)?, limits, control, false)?;
    let thumbnail = if decoded.image.width().max(decoded.image.height()) > 256 {
        decoded.image.thumbnail(256, 256)
    } else {
        decoded.image
    };
    control.check()?;
    let mut encoded_error = None;
    let result = cache.publish(hash, |file| {
        thumbnail
            .write_to(file, image::ImageFormat::Png)
            .map_err(|error| {
                encoded_error = Some(error);
                memedock_storage::StorageError::Integrity("thumbnail encoding failed")
            })
    });
    if let Some(error) = encoded_error {
        return Err(inspect::error(error));
    }
    Ok(result?)
}
