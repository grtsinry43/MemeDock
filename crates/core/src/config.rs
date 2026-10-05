use crate::{CoreError, ErrorCode, Result};
use std::{fs, path::PathBuf};

#[derive(Clone, Debug)]
pub struct ResourceLimits {
    pub task_capacity: usize,
    pub async_jobs: usize,
    pub file_jobs: usize,
    pub runtime_threads: usize,
    pub blocking_threads: usize,
    pub event_capacity: usize,
    pub max_file_bytes: u64,
    /// Reserved for the image use cases; original hash verification does not
    /// decode images and therefore does not enforce pixel/decode limits.
    pub max_frame_pixels: u64,
    pub large_decode_jobs: usize,
    pub thumbnail_jobs: usize,
    pub max_decode_bytes: u64,
    pub image_budget_bytes: u64,
    pub export_budget_bytes: u64,
}
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            task_capacity: 32,
            async_jobs: 4,
            file_jobs: 2,
            runtime_threads: 2,
            blocking_threads: 4,
            event_capacity: 64,
            max_file_bytes: 32 * 1024 * 1024,
            max_frame_pixels: 16_000_000,
            large_decode_jobs: 1,
            thumbnail_jobs: 2,
            max_decode_bytes: 128 * 1024 * 1024,
            image_budget_bytes: 192 * 1024 * 1024,
            export_budget_bytes: 512 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Debug)]
pub struct LibraryConfig {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub export_dir: PathBuf,
    pub limits: ResourceLimits,
}
impl LibraryConfig {
    pub fn new(data_dir: PathBuf, cache_dir: PathBuf, export_dir: PathBuf) -> Self {
        Self {
            data_dir,
            cache_dir,
            export_dir,
            limits: ResourceLimits::default(),
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        let l = &self.limits;
        for (value, maximum) in [
            (l.task_capacity, 4096),
            (l.async_jobs, 32),
            (l.file_jobs, 8),
            (l.runtime_threads, 8),
            (l.blocking_threads, 32),
            (l.event_capacity, 4096),
            (l.large_decode_jobs, 8),
            (l.thumbnail_jobs, 8),
        ] {
            if value == 0 || value > maximum {
                return Err(CoreError::new(
                    ErrorCode::InvalidInput,
                    "resource limit is outside the supported range",
                ));
            }
        }
        if l.file_jobs > l.blocking_threads
            || l.max_file_bytes == 0
            || l.export_budget_bytes == 0
            || l.max_file_bytes > i64::MAX as u64
            || l.max_frame_pixels == 0
            || l.max_decode_bytes == 0
            || l.max_decode_bytes > l.image_budget_bytes
            || l.max_decode_bytes.div_ceil(1024) > l.image_budget_bytes / 1024
            || l.image_budget_bytes > u32::MAX as u64 * 1024
        {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "inconsistent resource limits",
            ));
        }
        if [&self.data_dir, &self.cache_dir, &self.export_dir]
            .iter()
            .any(|p| !p.is_absolute())
        {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "library directories must be absolute",
            ));
        }
        Ok(())
    }
    /// Blocking initialization on the runtime owner thread, never a UI thread.
    pub(crate) fn resolve(mut self) -> Result<Self> {
        self.validate()?;
        for path in [
            &mut self.data_dir,
            &mut self.cache_dir,
            &mut self.export_dir,
        ] {
            match fs::symlink_metadata(&*path) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(CoreError::new(
                        ErrorCode::InvalidInput,
                        "library directory symlink",
                    ));
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            fs::create_dir_all(&*path)?;
            *path = fs::canonicalize(&*path)?;
        }
        let paths = [&self.data_dir, &self.cache_dir, &self.export_dir];
        for i in 0..3 {
            for j in i + 1..3 {
                if paths[i].starts_with(paths[j]) || paths[j].starts_with(paths[i]) {
                    return Err(CoreError::new(
                        ErrorCode::InvalidInput,
                        "data, cache and share directories must not overlap",
                    ));
                }
            }
        }
        Ok(self)
    }
}
