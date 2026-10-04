use crate::{CoreError, ErrorCode, Result};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

fn registry() -> &'static Mutex<HashSet<PathBuf>> {
    static PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    PATHS.get_or_init(|| Mutex::new(HashSet::new()))
}
pub(crate) struct Reservation(Vec<PathBuf>);
impl Reservation {
    pub(crate) fn acquire(config: &crate::LibraryConfig) -> Result<Self> {
        let requested = vec![
            config.data_dir.clone(),
            config.cache_dir.clone(),
            config.export_dir.clone(),
        ];
        let mut paths = registry()
            .lock()
            .map_err(|_| CoreError::internal("library registry poisoned"))?;
        if requested.iter().any(|path| {
            paths
                .iter()
                .any(|existing| existing.starts_with(path) || path.starts_with(existing))
        }) {
            return Err(CoreError::new(
                ErrorCode::AlreadyOpen,
                "library directory is already owned",
            ));
        }
        paths.extend(requested.iter().cloned());
        Ok(Self(requested))
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        // Only set insert/remove runs under this lock; no user code or IO. If an
        // unrelated panic poisons it, removing our reservation is still safe.
        let mut paths = registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for path in &self.0 {
            paths.remove(path);
        }
    }
}
