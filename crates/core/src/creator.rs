//! Owned source snapshots and generated animation metadata.
use crate::ArtifactLease;
use memedock_domain::identity::LibraryId;
use memedock_storage::files::StagedFile;
use std::{path::Path, sync::Arc};

pub use memedock_domain::creator::{VideoCrop, VideoEdit};
pub const MAX_VIDEO_BYTES: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct VideoInfo {
    pub mime: String,
    /// Square-pixel, oriented display dimensions.
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub frame_rate: f64,
}
/// Read-only private snapshot. Release after players/tasks finish; abandoned
/// snapshots are removed by staging recovery on the next library open.
pub struct CreatorVideo {
    pub(crate) staged: StagedFile,
    pub(crate) library_id: LibraryId,
    pub(crate) info: VideoInfo,
}
impl CreatorVideo {
    pub fn info(&self) -> &VideoInfo {
        &self.info
    }
    pub fn path(&self) -> &Path {
        self.staged.path()
    }
}
pub struct CreatedAnimation {
    pub artifact: Arc<ArtifactLease>,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub frame_rate: f64,
}
