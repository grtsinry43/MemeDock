use super::{StagedFile, directory, staging::hash_file, sync_directory};
use crate::{Result, StorageError};
use memedock_domain::identity::ContentHash;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct FsBlobStore {
    pub(super) root: PathBuf,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishDisposition {
    Created,
    Reused,
}
#[derive(Debug)]
pub struct PublishedBlob {
    pub hash: ContentHash,
    pub byte_size: u64,
    pub path: PathBuf,
    pub disposition: PublishDisposition,
}

impl FsBlobStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if root
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(StorageError::InvalidInput("library root symlink"));
        }
        fs::create_dir_all(root)?;
        let root = fs::canonicalize(root)?;
        directory(&root.join("staging"))?;
        directory(&root.join("blobs"))?;
        sync_directory(&root)?;
        if let Some(parent) = root.parent() {
            sync_directory(parent)?;
        }
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn original_path(&self, hash: ContentHash) -> PathBuf {
        let hash = hash.to_string();
        self.root
            .join("blobs")
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(hash)
    }
    pub fn verify(
        &self,
        hash: ContentHash,
        expected_bytes: u64,
        cancelled: impl FnMut() -> bool,
    ) -> Result<()> {
        let path = self.original_path(hash);
        self.check_shards(hash)?;
        let (actual, bytes) = hash_file(&path, expected_bytes, cancelled)?;
        if actual != hash || bytes != expected_bytes {
            return Err(StorageError::Integrity("original hash or length mismatch"));
        }
        Ok(())
    }
    pub fn open_original(&self, hash: ContentHash) -> Result<File> {
        self.check_shards(hash)?;
        let path = self.original_path(hash);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::Integrity("original is not a regular file"));
        }
        Ok(File::open(path)?)
    }
    /// Core must validate actual image format/dimensions before calling this.
    /// NOREPLACE rename atomically publishes without overwrite on Android/Linux,
    /// require staging and originals to be on the same filesystem.
    pub fn publish(
        &self,
        staged: StagedFile,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<PublishedBlob> {
        if staged.root != self.root
            || staged.path.parent() != Some(self.root.join("staging").as_path())
        {
            return Err(StorageError::InvalidInput(
                "staging belongs to another library",
            ));
        }
        let (actual, bytes) = hash_file(&staged.path, staged.byte_size, &mut cancelled)?;
        if actual != staged.hash || bytes != staged.byte_size {
            return Err(StorageError::Integrity("staging changed after validation"));
        }
        let path = self.original_path(staged.hash);
        let shard = self.root.join("blobs").join(&staged.hash.to_string()[..2]);
        directory(&shard)?;
        let parent = path
            .parent()
            .ok_or(StorageError::Integrity("blob parent"))?;
        directory(parent)?;
        if cancelled() {
            return Err(StorageError::Cancelled);
        }
        let disposition = match rustix::fs::renameat_with(
            rustix::fs::CWD,
            &staged.path,
            rustix::fs::CWD,
            &path,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)
        {
            Ok(()) => PublishDisposition::Created,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                self.verify(staged.hash, staged.byte_size, &mut cancelled)?;
                PublishDisposition::Reused
            }
            Err(e) => return Err(e.into()),
        };
        // Publication is the file commit point: finish durability and cleanup
        // even if cancellation arrives now. An orphan is safer than a dangling DB ref.
        File::open(&path)?.sync_all()?;
        sync_directory(parent)?;
        if disposition == PublishDisposition::Reused {
            fs::remove_file(&staged.path)?;
        }
        sync_directory(&self.root.join("staging"))?;
        Ok(PublishedBlob {
            hash: staged.hash,
            byte_size: staged.byte_size,
            path,
            disposition,
        })
    }
    fn check_shards(&self, hash: ContentHash) -> Result<()> {
        let text = hash.to_string();
        for path in [
            self.root.join("blobs"),
            self.root.join("blobs").join(&text[..2]),
            self.root.join("blobs").join(&text[..2]).join(&text[2..4]),
        ] {
            let m = fs::symlink_metadata(path)?;
            if !m.is_dir() || m.file_type().is_symlink() {
                return Err(StorageError::Integrity("unsafe original directory"));
            }
        }
        Ok(())
    }
}
