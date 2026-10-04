use super::FsBlobStore;
use crate::Result;
use memedock_domain::identity::ContentHash;
use std::{collections::HashSet, fs, path::PathBuf};

#[derive(Debug, Default)]
pub struct RecoveryScan {
    pub staging: Vec<PathBuf>,
    pub orphan_originals: Vec<ContentHash>,
    pub missing_originals: Vec<ContentHash>,
    pub unexpected_paths: Vec<PathBuf>,
}
impl FsBlobStore {
    /// Explicit maintenance scan, not a startup full-library rehash. Never
    /// follows symlinks, deletes originals, or assumes staging age implies idle.
    pub fn scan_recovery(&self, referenced: &HashSet<ContentHash>) -> Result<RecoveryScan> {
        let mut scan = RecoveryScan::default();
        let mut present = HashSet::new();
        for entry in fs::read_dir(self.root.join("staging"))? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let name = entry.file_name();
            let name = name.to_str();
            let controlled = name
                .and_then(|s| s.strip_suffix(".part"))
                .is_some_and(|s| s.parse::<memedock_domain::identity::OperationId>().is_ok());
            if ty.is_file() && controlled {
                scan.staging.push(entry.path());
            } else {
                scan.unexpected_paths.push(entry.path());
            }
        }
        for first in fs::read_dir(self.root.join("blobs"))? {
            let first = first?;
            if !first.file_type()?.is_dir() || !valid_shard(&first.file_name().to_string_lossy()) {
                scan.unexpected_paths.push(first.path());
                continue;
            }
            for second in fs::read_dir(first.path())? {
                let second = second?;
                if !second.file_type()?.is_dir()
                    || !valid_shard(&second.file_name().to_string_lossy())
                {
                    scan.unexpected_paths.push(second.path());
                    continue;
                }
                for entry in fs::read_dir(second.path())? {
                    let entry = entry?;
                    let hash = entry.file_name().to_string_lossy().parse::<ContentHash>();
                    if let Ok(hash) = hash
                        && entry.file_type()?.is_file()
                        && entry.path() == self.original_path(hash)
                    {
                        present.insert(hash);
                        if !referenced.contains(&hash) {
                            scan.orphan_originals.push(hash);
                        }
                    } else {
                        scan.unexpected_paths.push(entry.path());
                    }
                }
            }
        }
        scan.missing_originals = referenced.difference(&present).copied().collect();
        scan.staging.sort();
        scan.orphan_originals.sort();
        scan.missing_originals.sort();
        scan.unexpected_paths.sort();
        Ok(scan)
    }
}
fn valid_shard(text: &str) -> bool {
    text.len() == 2
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
