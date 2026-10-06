pub(crate) mod format;
pub(crate) mod merge;
pub(crate) mod validation;

use crate::{CoreError, ErrorCode};
use memedock_domain::{
    archive::{ArchiveData, RestoreSummary},
    identity::ContentHash,
};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct BackupFile {
    pub path: PathBuf,
    pub byte_size: u64,
    pub file_name: String,
}

/// Prepared input is owned by its library; Android never supplies a raw path.
pub struct ArchiveInput {
    pub(crate) path: PathBuf,
    pub(crate) root: PathBuf,
}
impl ArchiveInput {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

pub struct PreparedArchive {
    pub(crate) path: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) hash: ContentHash,
    pub(crate) data: ArchiveData,
    pub(crate) target_revision: i64,
    pub(crate) summary: RestoreSummary,
}
impl PreparedArchive {
    pub fn summary(&self) -> &RestoreSummary {
        &self.summary
    }
}

pub(crate) fn corrupt(message: &'static str) -> CoreError {
    CoreError::new(ErrorCode::CorruptData, message)
}
pub(crate) fn json(error: serde_json::Error) -> CoreError {
    CoreError::caused(ErrorCode::CorruptData, "invalid archive metadata", error)
}
pub(crate) fn zip(error: zip::result::ZipError) -> CoreError {
    match error {
        zip::result::ZipError::Io(error) => error.into(),
        other => CoreError::caused(ErrorCode::CorruptData, "invalid archive container", other),
    }
}
