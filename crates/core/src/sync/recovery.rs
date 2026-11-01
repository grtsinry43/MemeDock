use super::{budget, diagnostics, transfer};
use crate::{CoreError, ErrorCode, Result, runtime::Services};
use std::sync::Arc;

/// Startup is exclusive. Validate content before exposing an upgraded library;
/// each result commits independently so an interrupted scan is safe to repeat.
pub(crate) async fn recover(services: &Arc<Services>) -> Result<()> {
    if services.db.sync_binding(budget(services)).await?.is_none() {
        return Ok(());
    }
    let mut after = None;
    loop {
        let hashes = services.db.asset_hashes_after(after, 128).await?;
        if hashes.is_empty() {
            break;
        }
        after = hashes.last().copied();
        for (asset, local) in services.db.resource_rows(&hashes).await? {
            if services
                .db
                .sticker(asset.sticker_id())
                .await?
                .is_some_and(|s| s.lifecycle().is_purged())
            {
                continue;
            }
            let store = services.blobs.clone();
            let hash = asset.hash();
            let size = asset.byte_size().get() as u64;
            let verified =
                tokio::task::spawn_blocking(move || store.verify(hash, size, || false)).await?;
            let mut local = local.unwrap_or_else(|| memedock_domain::local::LocalAsset::new(hash));
            let mut failure = None;
            match verified {
                Ok(()) => local.verified(crate::writes::now()?),
                Err(error) => {
                    let error = CoreError::from(error);
                    if local.blob_status() == memedock_domain::local::BlobStatus::Ready
                        || error.code() != ErrorCode::NotFound
                    {
                        failure = Some(error);
                    }
                    local.mark_missing();
                }
            }
            let mut tx = services.db.begin_write().await?;
            tx.save_local_asset(&local).await?;
            if let Some(error) = failure {
                let mut binding = services
                    .db
                    .sync_binding(budget(services))
                    .await?
                    .ok_or_else(|| CoreError::internal("sync binding missing during recovery"))?;
                diagnostics::failed(
                    &mut binding,
                    &error,
                    "recover original",
                    Some(hash.to_string()),
                    true,
                )?;
                tx.commit_sync(&binding, None, budget(services)).await?;
            } else {
                tx.commit().await?;
            }
        }
    }
    transfer::refresh_locked(services).await
}
