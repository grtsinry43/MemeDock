use crate::{ResourceLimits, Result, images::inspect, tasks::TaskControl};
use memedock_domain::{
    asset::Asset,
    version::{ByteSize, TimestampMs},
};
use memedock_storage::files::StagedFile;
use std::{num::NonZeroU32, sync::Arc};
use tokio::sync::Notify;

/// Blocking server cache fill, using the same bounded PNG thumbnail pipeline.
pub fn prepare_thumbnail(
    blobs: &memedock_storage::files::FsBlobStore,
    cache: &memedock_storage::files::DerivedStore,
    hash: memedock_domain::identity::ContentHash,
) -> Result<std::path::PathBuf> {
    if crate::images::thumbnail::cached(cache, hash)? {
        return Ok(cache.thumbnail_path(hash));
    }
    let control = TaskControl::with_deadline(
        Arc::new(Notify::new()),
        std::time::Instant::now() + std::time::Duration::from_secs(30),
    );
    crate::images::thumbnail::generate(blobs, cache, hash, &ResourceLimits::default(), &control)
}

/// Blocking, bounded full-image validation using the same decoder as local import.
pub fn validate_original(staged: &StagedFile, at: TimestampMs) -> Result<Asset> {
    let limits = ResourceLimits::default();
    let control = TaskControl::with_deadline(
        Arc::new(Notify::new()),
        std::time::Instant::now() + std::time::Duration::from_secs(30),
    );
    let decoded = inspect::decode(staged.open_read()?, &limits, &control, true)?;
    let width = NonZeroU32::new(decoded.dimensions.0)
        .ok_or_else(|| crate::CoreError::internal("zero width"))?;
    let height = NonZeroU32::new(decoded.dimensions.1)
        .ok_or_else(|| crate::CoreError::internal("zero height"))?;
    Ok(Asset::new(
        staged.hash(),
        ByteSize::new(
            i64::try_from(staged.byte_size())
                .map_err(|_| crate::CoreError::internal("asset length"))?,
        )?,
        decoded.format,
        (width, height),
        decoded.animated,
        at,
    ))
}
