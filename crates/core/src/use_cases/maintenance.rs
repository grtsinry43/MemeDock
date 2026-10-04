use crate::{
    CoreError, ErrorCode, Library, Result,
    tasks::scheduler::Lane,
    tasks::{Priority, Task},
};
use memedock_domain::identity::ContentHash;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedOriginal {
    pub hash: ContentHash,
    pub byte_size: u64,
}
impl Library {
    pub fn verify_original(
        &self,
        hash: ContentHash,
        priority: Priority,
    ) -> Result<Task<VerifiedOriginal>> {
        self.submit(
            Lane::Blocking,
            priority,
            move |services, control| async move {
                let asset = services
                    .db
                    .asset(hash)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "asset not found"))?;
                let byte_size = u64::try_from(asset.byte_size().get()).map_err(|_| {
                    CoreError::new(ErrorCode::CorruptData, "negative original size")
                })?;
                if byte_size > services.config.limits.max_file_bytes {
                    return Err(CoreError::new(
                        ErrorCode::InvalidInput,
                        "original exceeds configured file limit",
                    ));
                }
                control.check()?;
                control.stage("verifying_original");
                // The scheduler reserves a blocking lane slot before spawning this.
                tokio::task::spawn_blocking(move || -> Result<VerifiedOriginal> {
                    services
                        .blobs
                        .verify(hash, byte_size, || control.is_cancelled())?;
                    Ok(VerifiedOriginal { hash, byte_size })
                })
                .await?
            },
        )
    }
    /// Explicit maintenance output under data_dir/checkpoints; not a portable
    /// archive or a FileProvider sharing artifact.
    pub fn create_checkpoint(&self) -> Result<Task<PathBuf>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            crate::writes::checkpoint,
        )
    }
}
