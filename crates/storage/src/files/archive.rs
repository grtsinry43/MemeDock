use crate::{Result, StorageError};
use memedock_domain::identity::OperationId;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

pub fn archive_directory(root: &Path) -> Result<PathBuf> {
    let base = root.join("archives");
    super::directory(&base)?;
    let directory = base.join(OperationId::new().to_string());
    super::directory(&directory)?;
    Ok(directory)
}
pub fn remove_archive_file(root: &Path, path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or(StorageError::InvalidInput("archive parent"))?;
    if parent.parent() != Some(root.join("archives").as_path())
        || parent
            .file_name()
            .and_then(|v| v.to_str())
            .is_none_or(|v| v.parse::<OperationId>().is_err())
    {
        return Err(StorageError::InvalidInput("foreign archive path"));
    }
    super::directory(&root.join("archives"))?;
    super::directory(parent)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StorageError::InvalidInput("archive is not a regular file"));
    }
    fs::remove_file(path)?;
    super::sync_directory(parent)?;
    if fs::read_dir(parent)?.next().is_none() {
        fs::remove_dir(parent)?;
    }
    File::open(root.join("archives"))?.sync_all()?;
    Ok(())
}

/// Session startup holds the root reservation; no live tokens exist yet.
pub fn discard_abandoned_archives(root: &Path) -> Result<()> {
    let base = root.join("archives");
    super::directory(&base)?;
    for entry in fs::read_dir(&base)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .is_none_or(|v| v.parse::<OperationId>().is_err())
        {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(entry.path())?;
        }
    }
    super::sync_directory(&base)?;
    Ok(())
}

pub fn remove_archive_directory(root: &Path, directory: &Path) -> Result<()> {
    let base = root.join("archives");
    if directory.parent() != Some(base.as_path())
        || directory
            .file_name()
            .and_then(|v| v.to_str())
            .is_none_or(|v| v.parse::<OperationId>().is_err())
    {
        return Err(StorageError::InvalidInput("foreign archive directory"));
    }
    super::directory(&base)?;
    if directory.try_exists()? {
        super::directory(directory)?;
        fs::remove_dir_all(directory)?;
    }
    super::sync_directory(&base)?;
    Ok(())
}
