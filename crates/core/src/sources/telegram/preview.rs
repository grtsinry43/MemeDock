use super::TelegramPack;
use crate::{
    CoreError, ErrorCode, Library, Preview, Result,
    images::inspect,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::{identity::ContentHash, source::SourceItemId};
use sha2::{Digest, Sha256};
use std::sync::Arc;

impl Library {
    pub fn telegram_preview(
        &self,
        pack: Arc<TelegramPack>,
        id: SourceItemId,
    ) -> Result<Task<Preview>> {
        pack.validate_library(self)?;
        pack.remote(&id)?;
        self.submit(
            Lane::Thumbnail,
            Priority::Visible,
            move |services, control| async move {
                let thumbnail = pack.remote(&id)?.thumbnail.as_ref().ok_or_else(|| {
                    CoreError::new(ErrorCode::NotFound, "telegram thumbnail unavailable")
                })?;
                let thumb_id = SourceItemId::new(thumbnail.file_unique_id.clone())?;
                let key = ContentHash::from_bytes(
                    Sha256::digest(format!("telegram-preview:{}", thumb_id.as_str()).as_bytes())
                        .into(),
                );
                let cache = services.derived.clone();
                let check = cache.clone();
                if tokio::task::spawn_blocking(move || check.source_preview_ready(key)).await?? {
                    return Ok(Preview {
                        path: cache.source_preview_path(key),
                    });
                }
                let worker = services.clone();
                let input =
                    tokio::task::spawn_blocking(move || worker.blobs.create_staging()).await??;
                if let Err(error) = pack
                    .client
                    .download(&thumbnail.file_id, input.path(), 1024 * 1024, &control)
                    .await
                {
                    tokio::task::spawn_blocking(move || input.discard()).await??;
                    return Err(error);
                }
                let allowance = services.image_budget.acquire().await?;
                let mut limits = services.config.limits.clone();
                limits.max_frame_pixels = 512 * 512;
                limits.max_decode_bytes = limits.max_decode_bytes.min(16 * 1024 * 1024);
                tokio::task::spawn_blocking(move || {
                    let _allowance = allowance;
                    let result = (|| -> Result<Preview> {
                        let image = inspect::decode(
                            std::fs::File::open(input.path())?,
                            &limits,
                            &control,
                            false,
                        )?
                        .image;
                        control.check()?;
                        cache.trim_source_previews(127 * 1024 * 1024)?;
                        let path = cache.publish(&cache.source_preview_path(key), |file| {
                            image
                                .thumbnail(256, 256)
                                .write_to(file, image::ImageFormat::Png)
                                .map_err(|_| {
                                    memedock_storage::StorageError::Integrity(
                                        "preview encoding failed",
                                    )
                                })?;
                            Ok(())
                        })?;
                        Ok(Preview { path })
                    })();
                    input.discard()?;
                    result
                })
                .await?
            },
        )
    }
}
