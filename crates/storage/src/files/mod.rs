//! Synchronous file primitives. Core must call these on a bounded blocking
//! executor. All paths live in app-private directories, never public storage.
mod blobs;
mod derived;
mod exports;
mod recovery;
mod staging;
pub use blobs::{FsBlobStore, PublishDisposition, PublishedBlob};
pub use derived::DerivedStore;
pub use exports::ExportStore;
pub use recovery::RecoveryScan;
pub use staging::{PendingStaging, StagedFile};

use crate::{Result, StorageError};
use std::{
    fs::{self, File},
    path::Path,
};

pub(crate) fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

pub(crate) fn directory(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => {
            if let Some(parent) = path.parent() {
                sync_directory(parent)?;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(StorageError::InvalidInput(
            "storage directory must not be a symlink",
        ));
    }
    Ok(())
}
