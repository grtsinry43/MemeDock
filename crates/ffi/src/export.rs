use crate::{
    ArtifactLeaseHandle, ClipboardProtectionTask, ClipboardUpdateTask, CollectionExportTask,
    ExportTask, LibraryHandle, Result,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum ExportPreset {
    Original,
    CompatiblePng,
    WhiteBackground,
    SmallJpeg,
    CompatibleGif,
    BestCompatibility,
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
                ExportPreset::CompatibleGif => d::ExportPreset::CompatibleGif,
                ExportPreset::BestCompatibility => d::ExportPreset::BestCompatibility,
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
    /// Writes one collection, or every collection, into one archive.
    ///
    /// Stickers whose original is missing or damaged are reported in the
    /// result instead of failing the export. `utc_offset_minutes` is the
    /// caller's offset from UTC, which is what the archive's entry times are
    /// written in because zip carries no zone.
    pub fn export_collection(
        &self,
        collection_id: Option<String>,
        destination: String,
        utc_offset_minutes: i32,
    ) -> Result<Arc<CollectionExportTask>> {
        let scope = match collection_id {
            Some(id) => memedock_core::ExportScope::Collection(id.parse()?),
            None => memedock_core::ExportScope::All,
        };
        Ok(CollectionExportTask::new(self.inner.export_collections(
            scope,
            std::path::PathBuf::from(destination),
            utc_offset_minutes,
        )?))
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
