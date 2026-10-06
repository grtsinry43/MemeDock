use crate::{
    ArtifactLeaseHandle, ClipboardProtectionTask, ClipboardUpdateTask, ExportTask, LibraryHandle,
    Result,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum ExportPreset {
    Original,
    CompatiblePng,
    WhiteBackground,
    SmallJpeg,
}
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum AnimationPolicy {
    Preserve,
    FirstFrame,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct ExportOptions {
    pub preset: ExportPreset,
    pub max_edge: Option<u32>,
    pub background_rgba: Option<u32>,
    pub strip_metadata: bool,
    pub animation: AnimationPolicy,
}
impl TryFrom<ExportOptions> for memedock_domain::export::ExportOptions {
    type Error = crate::BridgeError;
    fn try_from(v: ExportOptions) -> Result<Self> {
        use memedock_domain::export as d;
        Ok(Self::new(
            match v.preset {
                ExportPreset::Original => d::ExportPreset::Original,
                ExportPreset::CompatiblePng => d::ExportPreset::CompatiblePng,
                ExportPreset::WhiteBackground => d::ExportPreset::WhiteBackground,
                ExportPreset::SmallJpeg => d::ExportPreset::SmallJpeg,
            },
            v.max_edge,
            v.background_rgba,
            v.strip_metadata,
            match v.animation {
                AnimationPolicy::Preserve => d::AnimationPolicy::Preserve,
                AnimationPolicy::FirstFrame => d::AnimationPolicy::FirstFrame,
            },
        )?)
    }
}
#[uniffi::export]
impl LibraryHandle {
    pub fn export(&self, id: String, options: ExportOptions) -> Result<Arc<ExportTask>> {
        Ok(ExportTask::new(
            self.inner.export(id.parse()?, options.try_into()?)?,
        ))
    }
    pub fn protect_clipboard(
        &self,
        lease: Arc<ArtifactLeaseHandle>,
    ) -> Result<Arc<ClipboardProtectionTask>> {
        Ok(ClipboardProtectionTask::new(
            self.inner.protect_clipboard(lease.inner.clone())?,
        ))
    }
    pub fn reconcile_clipboard(
        &self,
        observed: Option<String>,
    ) -> Result<Arc<ClipboardUpdateTask>> {
        Ok(ClipboardUpdateTask::new(self.inner.reconcile_clipboard(
            observed.map(|v| v.parse()).transpose()?,
        )?))
    }
    pub fn abort_clipboard(&self, reference: String) -> Result<Arc<ClipboardUpdateTask>> {
        Ok(ClipboardUpdateTask::new(
            self.inner.abort_clipboard(reference.parse()?)?,
        ))
    }
}
