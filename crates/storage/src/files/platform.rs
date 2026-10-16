//! Publication primitives. Windows has no POSIX directory-fsync equivalent;
//! successful Windows publications use MoveFileExW with WRITE_THROUGH instead.
//! Deletions remain replayable by the existing startup recovery procedures.
use crate::Result;
#[cfg(windows)]
use crate::StorageError;
use std::{fs, path::Path};

pub(crate) fn sync_file(path: &Path) -> Result<()> {
    #[cfg(windows)]
    let file = fs::OpenOptions::new().read(true).write(true).open(path)?;
    #[cfg(not(windows))]
    let file = fs::File::open(path)?;
    file.sync_all()?;
    Ok(())
}

/// Finish directory publication bookkeeping. Unix flushes the directory;
/// Windows validates it, relying on the write-through publication primitives.
pub fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(not(windows))]
    fs::File::open(path)?.sync_all()?;
    #[cfg(windows)]
    {
        // Do not attempt FlushFileBuffers on a read-only directory handle:
        // Windows does not provide that POSIX operation. Validate the directory
        // here; move_no_replace/replace perform write-through publication.
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(StorageError::Integrity("unsafe publication directory"));
        }
    }
    Ok(())
}

pub(crate) fn move_no_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    return atomicwrites::move_atomic(source, destination);
    #[cfg(not(windows))]
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

pub(crate) fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    return atomicwrites::replace_atomic(source, destination);
    #[cfg(not(windows))]
    fs::rename(source, destination)
}
