use super::{directory, sync_directory};
use crate::{Result, StorageError};
use memedock_domain::identity::ContentHash;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct DerivedStore {
    root: PathBuf,
}
impl DerivedStore {
    /// No encoders are active during startup. Keep unknown files untouched.
    pub fn discard_abandoned_publications(&self) -> Result<()> {
        directory(&self.root)?;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry
                    .file_name()
                    .to_str()
                    .and_then(|name| name.strip_suffix(".part"))
                    .is_some_and(|name| {
                        name.parse::<memedock_domain::identity::OperationId>()
                            .is_ok()
                    })
            {
                fs::remove_file(entry.path())?;
            }
        }
        sync_directory(&self.root)
    }
    pub fn open(root: &Path) -> Result<Self> {
        directory(&root.join("thumbnails"))?;
        Ok(Self {
            root: root.join("thumbnails"),
        })
    }
    pub fn thumbnail_path(&self, hash: ContentHash) -> PathBuf {
        self.root.join(format!("thumb-v1-256-{hash}.png"))
    }
    pub fn ready(&self, hash: ContentHash) -> Result<bool> {
        let metadata = match fs::symlink_metadata(self.thumbnail_path(hash)) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::Integrity("unsafe thumbnail file"));
        }
        Ok(metadata.len() > 0)
    }
    /// The encoder writes directly to a file. No whole encoded-image buffer.
    pub fn publish(
        &self,
        hash: ContentHash,
        encode: impl FnOnce(&mut File) -> Result<()>,
    ) -> Result<PathBuf> {
        directory(&self.root)?;
        let temp = self.root.join(format!("{}.part", uuid::Uuid::now_v7()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            encode(&mut file)?;
            file.flush()?;
            file.sync_all()?;
            let output = self.thumbnail_path(hash);
            fs::rename(&temp, &output)?;
            sync_directory(&self.root)?;
            Ok(output)
        })();
        if result.is_err() {
            match fs::remove_file(&temp) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(cleanup) => {
                    return Err(StorageError::Cleanup {
                        source: Box::new(
                            result
                                .err()
                                .ok_or(StorageError::Integrity("missing publication error"))?,
                        ),
                        cleanup,
                    });
                }
            }
        }
        result
    }
}
