use super::{FsBlobStore, sync_directory};
use crate::{Result, StorageError};
use memedock_domain::identity::ContentHash;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Ownership token for a fully copied, flushed original candidate. Dropping it
/// leaves a recoverable staging file; discard explicitly to observe IO errors.
pub struct StagedFile {
    pub(super) path: PathBuf,
    pub(super) root: PathBuf,
    pub(super) hash: ContentHash,
    pub(super) byte_size: u64,
}

/// Exclusive ownership of an app-private input slot. Foreign callers may write
/// only while they own this token, and must close their writer before finishing.
/// Abandoned tokens leave a recoverable file; dropping never blocks a UI thread.
pub struct PendingStaging {
    path: PathBuf,
    root: PathBuf,
}
impl PendingStaging {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn finish(&self, max_bytes: u64, cancelled: impl FnMut() -> bool) -> Result<StagedFile> {
        let (hash, byte_size) = hash_file(&self.path, max_bytes, cancelled)?;
        File::open(&self.path)?.sync_all()?;
        sync_directory(&self.root.join("staging"))?;
        Ok(StagedFile {
            path: self.path.clone(),
            root: self.root.clone(),
            hash,
            byte_size,
        })
    }
    pub fn discard(self) -> Result<()> {
        fs::remove_file(&self.path)?;
        sync_directory(&self.root.join("staging"))
    }
}
impl StagedFile {
    pub fn hash(&self) -> ContentHash {
        self.hash
    }
    pub fn byte_size(&self) -> u64 {
        self.byte_size
    }
    /// Core uses this read-only descriptor for actual image validation.
    pub fn open_read(&self) -> Result<File> {
        Ok(File::open(&self.path)?)
    }
    pub fn discard(self) -> Result<()> {
        fs::remove_file(&self.path)?;
        sync_directory(
            self.path
                .parent()
                .ok_or(StorageError::Integrity("staging parent"))?,
        )
    }
}
impl FsBlobStore {
    pub fn create_staging(&self) -> Result<PendingStaging> {
        let directory = self.root.join("staging");
        super::directory(&directory)?;
        let path = directory.join(format!("{}.part", uuid::Uuid::now_v7()));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?
            .sync_all()?;
        sync_directory(&directory)?;
        Ok(PendingStaging {
            path,
            root: self.root.clone(),
        })
    }
    /// Copy untrusted input into controlled staging, checking actual bytes and
    /// cooperative cancellation between bounded reads. No entire-file buffer.
    pub fn stage_from(
        &self,
        reader: &mut impl Read,
        max_bytes: u64,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<StagedFile> {
        if max_bytes == 0 || max_bytes > i64::MAX as u64 {
            return Err(StorageError::InvalidInput("byte limit"));
        }
        let path = self
            .root
            .join("staging")
            .join(format!("{}.part", uuid::Uuid::now_v7()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let result = (|| {
            let (hash, byte_size) = copy_hash(reader, &mut file, max_bytes, &mut cancelled)?;
            file.sync_all()?;
            sync_directory(&self.root.join("staging"))?;
            Ok(StagedFile {
                path: path.clone(),
                root: self.root.clone(),
                hash,
                byte_size,
            })
        })();
        drop(file);
        match result {
            Ok(staged) => Ok(staged),
            Err(error) => {
                if let Err(cleanup) = fs::remove_file(&path) {
                    return Err(StorageError::Cleanup {
                        source: Box::new(error),
                        cleanup,
                    });
                }
                sync_directory(&self.root.join("staging"))?;
                Err(error)
            }
        }
    }
}

pub(super) fn hash_file(
    path: &Path,
    max_bytes: u64,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(ContentHash, u64)> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StorageError::Integrity("original is not a regular file"));
    }
    let mut file = File::open(path)?;
    copy_hash(&mut file, &mut std::io::sink(), max_bytes, &mut cancelled)
}
fn copy_hash(
    reader: &mut impl Read,
    writer: &mut impl Write,
    max_bytes: u64,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(ContentHash, u64)> {
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancelled() {
            return Err(StorageError::Cancelled);
        }
        let remaining = max_bytes - size;
        let read_limit = remaining.saturating_add(1).min(buffer.len() as u64) as usize;
        let count = match reader.read(&mut buffer[..read_limit]) {
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            other => other?,
        };
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or(StorageError::InvalidInput("input byte overflow"))?;
        if size > max_bytes {
            return Err(StorageError::InvalidInput("input exceeds byte limit"));
        }
        writer.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
    }
    Ok((ContentHash::from_bytes(hash.finalize().into()), size))
}
