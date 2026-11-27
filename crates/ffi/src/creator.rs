use crate::{
    ArtifactLeaseHandle, BridgeError, CreatorAnimationTask, CreatorArtifactTask, CreatorVideoTask,
    DiscardInputTask, ErrorCode, LibraryHandle, Result,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, uniffi::Record)]
pub struct VideoInfo {
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub frame_rate: f64,
}
impl From<&memedock_core::creator::VideoInfo> for VideoInfo {
    fn from(v: &memedock_core::creator::VideoInfo) -> Self {
        Self {
            mime: v.mime.clone(),
            width: v.width,
            height: v.height,
            duration_ms: v.duration_ms,
            frame_rate: v.frame_rate,
        }
    }
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct VideoEditOptions {
    pub start_ms: u64,
    pub end_ms: u64,
    pub crop_x: f64,
    pub crop_y: f64,
    pub crop_width: f64,
    pub crop_height: f64,
    pub max_edge: u32,
    pub fps: u32,
    pub quality: u8,
}
impl TryFrom<VideoEditOptions> for memedock_core::creator::VideoEdit {
    type Error = BridgeError;
    fn try_from(v: VideoEditOptions) -> Result<Self> {
        Ok(Self::new(
            v.start_ms,
            v.end_ms,
            memedock_core::creator::VideoCrop::new(
                v.crop_x,
                v.crop_y,
                v.crop_width,
                v.crop_height,
            )?,
            v.max_edge,
            v.fps,
            v.quality,
        )?)
    }
}
#[derive(uniffi::Object)]
pub struct CreatorVideoHandle {
    inner: Mutex<Option<Arc<memedock_core::creator::CreatorVideo>>>,
    path: String,
    info: VideoInfo,
}
impl CreatorVideoHandle {
    pub(crate) fn new(video: Arc<memedock_core::creator::CreatorVideo>) -> Result<Arc<Self>> {
        let path = video
            .path()
            .to_str()
            .ok_or_else(|| BridgeError::new(ErrorCode::InvalidInput, "video path is not UTF-8"))?
            .to_owned();
        let info = video.info().into();
        Ok(Arc::new(Self {
            inner: Mutex::new(Some(video)),
            path,
            info,
        }))
    }
    fn get(&self) -> Result<Arc<memedock_core::creator::CreatorVideo>> {
        self.inner
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "video slot poisoned"))?
            .clone()
            .ok_or_else(|| BridgeError::new(ErrorCode::Conflict, "video already released"))
    }
    fn take(&self) -> Result<Arc<memedock_core::creator::CreatorVideo>> {
        self.inner
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "video slot poisoned"))?
            .take()
            .ok_or_else(|| BridgeError::new(ErrorCode::Conflict, "video already released"))
    }
}
#[uniffi::export]
impl CreatorVideoHandle {
    pub fn path(&self) -> String {
        self.path.clone()
    }
    pub fn info(&self) -> VideoInfo {
        self.info.clone()
    }
}
#[derive(uniffi::Record)]
pub struct CreatedAnimation {
    pub artifact: Arc<ArtifactLeaseHandle>,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub frame_rate: f64,
}
impl From<memedock_core::creator::CreatedAnimation> for CreatedAnimation {
    fn from(v: memedock_core::creator::CreatedAnimation) -> Self {
        Self {
            artifact: ArtifactLeaseHandle::new(v.artifact),
            width: v.width,
            height: v.height,
            duration_ms: v.duration_ms,
            frame_rate: v.frame_rate,
        }
    }
}
#[uniffi::export]
impl LibraryHandle {
    pub fn open_creator_video(&self, path: String) -> Result<Arc<CreatorVideoTask>> {
        if path.contains('\0') {
            return Err(BridgeError::new(
                ErrorCode::InvalidInput,
                "invalid video path",
            ));
        }
        Ok(CreatorVideoTask::new(
            self.inner.open_creator_video(path.into())?,
        ))
    }
    pub fn discard_creator_video(
        &self,
        video: Arc<CreatorVideoHandle>,
    ) -> Result<Arc<DiscardInputTask>> {
        Ok(DiscardInputTask::new(
            self.inner.discard_creator_video(video.take()?)?,
        ))
    }
    pub fn create_video_webp(
        &self,
        video: Arc<CreatorVideoHandle>,
        edit: VideoEditOptions,
    ) -> Result<Arc<CreatorAnimationTask>> {
        Ok(CreatorAnimationTask::new(
            self.inner
                .create_video_webp(video.get()?, edit.try_into()?)?,
        ))
    }
    pub fn creator_preview(
        &self,
        video: Arc<CreatorVideoHandle>,
        time_ms: u64,
    ) -> Result<Arc<CreatorArtifactTask>> {
        Ok(CreatorArtifactTask::new(
            self.inner.creator_preview(video.get()?, time_ms)?,
        ))
    }
    pub fn creation_gif(
        &self,
        source: Arc<ArtifactLeaseHandle>,
        max_bytes: Option<u64>,
    ) -> Result<Arc<CreatorArtifactTask>> {
        let limit = max_bytes
            .map(memedock_domain::export::GifSizeLimit::new)
            .transpose()?;
        Ok(CreatorArtifactTask::new(
            self.inner.creation_gif(source.inner.clone(), limit)?,
        ))
    }
}
