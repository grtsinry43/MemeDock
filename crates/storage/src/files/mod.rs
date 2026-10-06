//! Synchronous file primitives. Core must call these on a bounded blocking
//! executor. All paths live in app-private directories, never public storage.
pub mod archive;
mod blobs;
mod derived;
mod exports;
pub mod library_layout;
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

/// A snapshot of regular files directly in an owned cache directory. Never
/// follows symlinks or descends into unrelated directories.
pub(crate) fn directory_bytes(path: &Path, mut cancelled: impl FnMut() -> bool) -> Result<u64> {
    if cancelled() {
        return Err(StorageError::Cancelled);
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(StorageError::InvalidInput("unsafe cache directory"));
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut size = 0u64;
    for entry in entries {
        if cancelled() {
            return Err(StorageError::Cancelled);
        }
        let metadata = match entry.and_then(|entry| entry.metadata()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.is_file() {
            size = size
                .checked_add(metadata.len())
                .ok_or(StorageError::Integrity("cache usage overflow"))?;
        }
    }
    if cancelled() {
        return Err(StorageError::Cancelled);
    }
    Ok(size)
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
