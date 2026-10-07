use crate::{ResourceLimits, Result, images::inspect, tasks::TaskControl};
use memedock_domain::identity::ContentHash;
use memedock_storage::files::{DerivedStore, FsBlobStore};
use std::path::PathBuf;

pub(crate) const EDGE: u32 = 1280;

pub(crate) fn fitted_size(width: u32, height: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let long = width.max(height);
    if long <= EDGE {
        return Some((width, height));
    }
    let width = u64::from(width);
    let height = u64::from(height);
    let long = u64::from(long);
    let short = u32::try_from(width.min(height) * u64::from(EDGE) / long).ok()?;
    let short = short.max(1);
    if width >= height {
        Some((EDGE, short))
    } else {
        Some((short, EDGE))
    }
}

pub(crate) fn cached(cache: &DerivedStore, hash: ContentHash) -> Result<bool> {
    if !cache.preview_ready(hash)? {
        return Ok(false);
    }
    let path = cache.preview_path(hash);
    let file = std::fs::File::open(path)?;
    let mut reader =
        image::ImageReader::with_format(std::io::BufReader::new(file), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(EDGE);
    limits.max_image_height = Some(EDGE);
    limits.max_alloc = Some(u64::from(EDGE) * u64::from(EDGE) * 4);
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
    let image = match fitted_size(decoded.image.width(), decoded.image.height()) {
        Some((width, height))
            if width != decoded.image.width() || height != decoded.image.height() =>
        {
            decoded.image.thumbnail(width, height)
        }
        Some(_) => decoded.image,
        None => {
            return Err(crate::CoreError::new(
                crate::ErrorCode::InvalidImage,
                "image has no display frame",
            ));
        }
    };
    control.check()?;
    let mut encoded_error = None;
    let destination = cache.preview_path(hash);
    let result = cache.publish(&destination, |file| {
        image
            .write_to(file, image::ImageFormat::Png)
            .map_err(|error| {
                encoded_error = Some(error);
                memedock_storage::StorageError::Integrity("preview encoding failed")
            })
    });
    if let Some(error) = encoded_error {
        return Err(inspect::error(error));
    }
    Ok(result?)
}

#[cfg(test)]
mod tests {
    use super::fitted_size;

    #[test]
    fn preview_shrinks_only_when_the_long_edge_exceeds_1280() {
        assert_eq!(fitted_size(0, 10), None);
        assert_eq!(fitted_size(100, 50), Some((100, 50)));
        assert_eq!(fitted_size(1280, 720), Some((1280, 720)));
        assert_eq!(fitted_size(2000, 1000), Some((1280, 640)));
        assert_eq!(fitted_size(1000, 2000), Some((640, 1280)));
    }
}
