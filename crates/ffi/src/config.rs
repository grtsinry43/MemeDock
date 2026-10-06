use crate::{BridgeError, ErrorCode, Result};
use memedock_core::{LibraryConfig, ResourceLimits};

#[derive(Clone, Debug, uniffi::Record)]
pub struct LibraryConfiguration {
    pub data_dir: String,
    pub cache_dir: String,
    pub export_dir: String,
    pub limits: ResourceConfiguration,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct ResourceConfiguration {
    pub task_capacity: u32,
    pub async_jobs: u32,
    pub file_jobs: u32,
    pub runtime_threads: u32,
    pub blocking_threads: u32,
    pub event_capacity: u32,
    pub max_file_bytes: u64,
    pub max_frame_pixels: u64,
    pub large_decode_jobs: u32,
    pub thumbnail_jobs: u32,
    pub max_decode_bytes: u64,
    pub image_budget_bytes: u64,
    pub export_budget_bytes: u64,
    pub max_archive_bytes: u64,
    pub max_archive_metadata_bytes: u64,
    pub max_archive_entries: u32,
}
fn size(value: u32) -> Result<usize> {
    usize::try_from(value)
        .map_err(|_| BridgeError::new(ErrorCode::InvalidInput, "resource limit overflow"))
}
impl TryFrom<LibraryConfiguration> for LibraryConfig {
    type Error = BridgeError;
    fn try_from(value: LibraryConfiguration) -> Result<Self> {
        if [&value.data_dir, &value.cache_dir, &value.export_dir]
            .iter()
            .any(|p| p.contains('\0'))
        {
            return Err(BridgeError::new(
                ErrorCode::InvalidInput,
                "path contains a null byte",
            ));
        }
        let l = value.limits;
        Ok(Self {
            data_dir: value.data_dir.into(),
            cache_dir: value.cache_dir.into(),
            export_dir: value.export_dir.into(),
            limits: ResourceLimits {
                task_capacity: size(l.task_capacity)?,
                async_jobs: size(l.async_jobs)?,
                file_jobs: size(l.file_jobs)?,
                runtime_threads: size(l.runtime_threads)?,
                blocking_threads: size(l.blocking_threads)?,
                event_capacity: size(l.event_capacity)?,
                max_file_bytes: l.max_file_bytes,
                max_frame_pixels: l.max_frame_pixels,
                large_decode_jobs: size(l.large_decode_jobs)?,
                thumbnail_jobs: size(l.thumbnail_jobs)?,
                max_decode_bytes: l.max_decode_bytes,
                image_budget_bytes: l.image_budget_bytes,
                export_budget_bytes: l.export_budget_bytes,
                max_archive_bytes: l.max_archive_bytes,
                max_archive_metadata_bytes: l.max_archive_metadata_bytes,
                max_archive_entries: size(l.max_archive_entries)?,
            },
        })
    }
}
#[uniffi::export]
pub fn default_resource_configuration() -> Result<ResourceConfiguration> {
    let l = ResourceLimits::default();
    let small = |v| {
        u32::try_from(v)
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "default resource limit overflow"))
    };
    Ok(ResourceConfiguration {
        task_capacity: small(l.task_capacity)?,
        async_jobs: small(l.async_jobs)?,
        file_jobs: small(l.file_jobs)?,
        runtime_threads: small(l.runtime_threads)?,
        blocking_threads: small(l.blocking_threads)?,
        event_capacity: small(l.event_capacity)?,
        max_file_bytes: l.max_file_bytes,
        max_frame_pixels: l.max_frame_pixels,
        large_decode_jobs: small(l.large_decode_jobs)?,
        thumbnail_jobs: small(l.thumbnail_jobs)?,
        max_decode_bytes: l.max_decode_bytes,
        image_budget_bytes: l.image_budget_bytes,
        export_budget_bytes: l.export_budget_bytes,
        max_archive_bytes: l.max_archive_bytes,
        max_archive_metadata_bytes: l.max_archive_metadata_bytes,
        max_archive_entries: small(l.max_archive_entries)?,
    })
}
