use super::{directory, staging::hash_file, sync_directory};
use crate::{Result, StorageError, artifacts::ArtifactRecord};
use memedock_domain::identity::OperationId;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct ExportStore {
    root: PathBuf,
}
pub struct OrphanCandidate {
    pub id: OperationId,
    name: String,
}
impl OrphanCandidate {
    pub fn cursor(&self) -> &str {
        &self.name
    }
}
impl ExportStore {
    /// Bounded scan of recognizable, old outputs. Database ownership is checked
    /// by core before removal; unknown files and symlinks are never candidates.
    pub fn orphan_page(
        &self,
        after: Option<&str>,
        older_than: i64,
    ) -> Result<Vec<OrphanCandidate>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if after.is_some_and(|after| name.as_str() <= after) {
                continue;
            }
            let Some((id, extension)) = name.rsplit_once('.') else {
                continue;
            };
            if !matches!(extension, "png" | "jpg" | "gif" | "webp") {
                continue;
            }
            let Ok(id) = id.parse::<OperationId>() else {
                continue;
            };
            let modified = entry
                .metadata()?
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .and_then(|v| i64::try_from(v.as_millis()).ok());
            if modified.is_none_or(|v| v > older_than) {
                continue;
            }
            files.push(OrphanCandidate { id, name });
            files.sort_by(|a, b| a.name.cmp(&b.name));
            if files.len() > 100 {
                files.pop();
            }
        }
        Ok(files)
    }
    pub fn remove_orphan(&self, candidate: &OrphanCandidate) -> Result<()> {
        let path = self.root.join(&candidate.name);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::Integrity("unsafe orphan output"));
        }
        fs::remove_file(path)?;
        sync_directory(&self.root)
    }
    pub fn open(root: &Path) -> Result<Self> {
        directory(root)?;
        Ok(Self {
            root: fs::canonicalize(root)?,
        })
    }
    pub fn path(&self, record: &ArtifactRecord) -> PathBuf {
        self.root
            .join(format!("{}.{}", record.id, record.format.extension()))
    }
    pub fn bytes_used(&self) -> Result<u64> {
        directory(&self.root)?;
        let mut size = 0u64;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                size = size
                    .checked_add(entry.metadata()?.len())
                    .ok_or(StorageError::Integrity("export usage overflow"))?;
            }
        }
        Ok(size)
    }
    pub fn verify(
        &self,
        record: &ArtifactRecord,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<()> {
        let (hash, size) = hash_file(&self.path(record), record.byte_size, &mut cancelled)?;
        if hash != record.output_hash || size != record.byte_size {
            return Err(StorageError::Integrity("artifact bytes changed"));
        }
        Ok(())
    }
    /// Bounded streaming copy. Never overwrite an already issued URI's file.
    pub fn publish(
        &self,
        record: &ArtifactRecord,
        mut source: File,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<PathBuf> {
        directory(&self.root)?;
        let temporary = self.root.join(format!("{}.part", OperationId::new()));
        let result = (|| {
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            let mut digest = Sha256::new();
            let mut size = 0u64;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                if cancelled() {
                    return Err(StorageError::Cancelled);
                }
                let read = source.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                size = size
                    .checked_add(read as u64)
                    .ok_or(StorageError::Integrity("artifact size overflow"))?;
                if size > record.byte_size {
                    return Err(StorageError::Integrity("original size changed"));
                }
                digest.update(&buffer[..read]);
                output.write_all(&buffer[..read])?;
            }
            let actual: [u8; 32] = digest.finalize().into();
            if memedock_domain::identity::ContentHash::from_bytes(actual) != record.output_hash
                || size != record.byte_size
            {
                return Err(StorageError::Integrity("original bytes changed"));
            }
            output.flush()?;
            output.sync_all()?;
            if cancelled() {
                return Err(StorageError::Cancelled);
            }
            let path = self.path(record);
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                &temporary,
                rustix::fs::CWD,
                &path,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(std::io::Error::from)?;
            sync_directory(&self.root)?;
            Ok(path)
        })();
        if result.is_err() {
            match fs::remove_file(&temporary) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(cleanup) => {
                    return Err(StorageError::Cleanup {
                        source: Box::new(
                            result
                                .err()
                                .ok_or(StorageError::Integrity("missing export error"))?,
                        ),
                        cleanup,
                    });
                }
            }
        }
        result
    }
    pub fn remove(&self, record: &ArtifactRecord) -> Result<()> {
        directory(&self.root)?;
        match fs::symlink_metadata(self.path(record)) {
            Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
                return Err(StorageError::Integrity("unsafe artifact file"));
            }
            Ok(_) => fs::remove_file(self.path(record))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        sync_directory(&self.root)
    }
    /// Startup has no live encoders. Unknown paths and published files survive.
    pub fn discard_abandoned_publications(&self) -> Result<()> {
        directory(&self.root)?;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry
                    .file_name()
                    .to_str()
                    .and_then(|n| n.strip_suffix(".part"))
                    .is_some_and(|n| n.parse::<OperationId>().is_ok())
            {
                fs::remove_file(entry.path())?;
            }
        }
        sync_directory(&self.root)
    }
}
