//! Recovery of an interrupted current replacement, never an old-layout upgrade.
use crate::{Result, StorageError};
use memedock_domain::identity::OperationId;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Switch {
    id: OperationId,
}

pub struct LibraryLayout {
    root: PathBuf,
}
impl LibraryLayout {
    pub fn open(root: &Path) -> Result<Self> {
        super::directory(root)?;
        super::directory(&root.join("restores"))?;
        super::directory(&root.join("checkpoints"))?;
        let layout = Self {
            root: root.to_owned(),
        };
        let unfinished = root.join("replacement.pending");
        if unfinished.try_exists()? {
            let metadata = fs::symlink_metadata(&unfinished)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(StorageError::Integrity(
                    "invalid pending replacement marker",
                ));
            }
            fs::remove_file(&unfinished)?;
            super::sync_directory(root)?;
        }
        let marker = root.join("replacement.json");
        if marker.try_exists()? {
            let metadata = fs::symlink_metadata(&marker)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 1024 {
                return Err(StorageError::Integrity("invalid replacement marker"));
            }
            let state: Switch =
                serde_json::from_reader(File::open(&marker)?).map_err(|source| {
                    StorageError::InvalidData {
                        table: "replacement",
                        source,
                    }
                })?;
            let candidate = layout.candidate(state.id)?;
            let checkpoint = layout.checkpoint(state.id);
            let current = root.join("library.sqlite");
            if !current.try_exists()? {
                if !checkpoint.try_exists()? {
                    return Err(StorageError::Integrity(
                        "replacement has no current library or checkpoint",
                    ));
                }
                fs::rename(checkpoint, &current)?;
                super::sync_directory(&root.join("checkpoints"))?;
                super::sync_directory(root)?;
            } else if candidate.try_exists()? && layout.checkpoint(state.id).try_exists()? {
                return Err(StorageError::Integrity("ambiguous replacement state"));
            }
            fs::remove_file(marker)?;
            super::sync_directory(root)?;
        }
        Ok(layout)
    }
    pub fn candidate(&self, id: OperationId) -> Result<PathBuf> {
        let directory = self.root.join("restores").join(id.to_string());
        super::directory(&directory)?;
        Ok(directory.join("library.sqlite"))
    }
    pub fn checkpoint(&self, id: OperationId) -> PathBuf {
        self.root.join("checkpoints").join(format!("{id}.sqlite"))
    }
    /// Called once at session startup, after interrupted switch recovery.
    pub fn discard_abandoned_candidates(&self) -> Result<()> {
        let directory = self.root.join("restores");
        for entry in fs::read_dir(&directory)? {
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
        super::sync_directory(&directory)?;
        Ok(())
    }
    /// Both pools must be closed. Originals are immutable and shared under root;
    /// retaining the complete old DB retains its original reference set.
    pub fn switch(&self, id: OperationId) -> Result<PathBuf> {
        let current = self.root.join("library.sqlite");
        let candidate = self.candidate(id)?;
        let checkpoint = self.checkpoint(id);
        if checkpoint.try_exists()? {
            return Err(StorageError::Conflict("checkpoint exists"));
        }
        if self.root.join("library.sqlite-wal").try_exists()?
            || self.root.join("library.sqlite-shm").try_exists()?
        {
            return Err(StorageError::Conflict("library database is still open"));
        }
        File::open(&candidate)?.sync_all()?;
        let marker = self.root.join("replacement.json");
        if marker.try_exists()? {
            return Err(StorageError::Conflict("replacement in progress"));
        }
        let pending = self.root.join("replacement.pending");
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        file.write_all(&serde_json::to_vec(&Switch { id }).map_err(|source| {
            StorageError::InvalidData {
                table: "replacement",
                source,
            }
        })?)?;
        file.sync_all()?;
        fs::rename(pending, &marker)?;
        super::sync_directory(&self.root)?;
        fs::rename(&current, &checkpoint)?;
        super::sync_directory(&self.root.join("checkpoints"))?;
        super::sync_directory(&self.root)?;
        fs::rename(&candidate, &current)?;
        super::sync_directory(
            candidate
                .parent()
                .ok_or(StorageError::Integrity("candidate parent"))?,
        )?;
        super::sync_directory(&self.root)?;
        fs::remove_file(marker)?;
        super::sync_directory(&self.root)?;
        fs::remove_dir(
            candidate
                .parent()
                .ok_or(StorageError::Integrity("candidate parent"))?,
        )?;
        super::sync_directory(&self.root.join("restores"))?;
        Ok(checkpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn interrupted_current_switch_reopens_old_or_new_complete_database()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        for phase in 0..3 {
            let root = tempfile::tempdir()?;
            let layout = LibraryLayout::open(root.path())?;
            let old = crate::LibraryDatabase::open(root.path().join("library.sqlite")).await?;
            let old_identity = old.identity();
            old.close().await?;
            let id = OperationId::new();
            let candidate_path = layout.candidate(id)?;
            let new = crate::LibraryDatabase::open(&candidate_path).await?;
            let new_identity = new.identity();
            new.close().await?;
            fs::write(
                root.path().join("replacement.json"),
                serde_json::to_vec(&Switch { id })?,
            )?;
            if phase >= 1 {
                fs::rename(root.path().join("library.sqlite"), layout.checkpoint(id))?;
            }
            if phase >= 2 {
                fs::rename(candidate_path, root.path().join("library.sqlite"))?;
            }
            let layout = LibraryLayout::open(root.path())?;
            layout.discard_abandoned_candidates()?;
            let opened = crate::LibraryDatabase::open(root.path().join("library.sqlite")).await?;
            assert_eq!(
                opened.identity(),
                if phase == 2 {
                    new_identity
                } else {
                    old_identity
                }
            );
            opened.close().await?;
            assert!(!root.path().join("replacement.json").exists());
        }
        Ok(())
    }
}
