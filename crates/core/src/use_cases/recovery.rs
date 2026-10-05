use crate::{Result, runtime::Services};
use std::sync::Arc;

/// Runs before the library is exposed, while the directory reservation is held.
/// There are no live staging owners in this session yet. Unknown files remain.
pub(crate) async fn recover(services: &Arc<Services>) -> Result<()> {
    let blobs = services.blobs.clone();
    let derived = services.derived.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        blobs.discard_abandoned_staging()?;
        derived.discard_abandoned_publications()?;
        Ok(())
    })
    .await??;
    let mut cursor = None;
    loop {
        let locals = services.db.interrupted_thumbnails(cursor).await?;
        if locals.is_empty() {
            break;
        }
        cursor = locals.last().map(|local| local.hash());
        for mut local in locals {
            local.evict_thumbnail();
            let mut tx = services.db.begin_write().await?;
            tx.save_local_asset(&local).await?;
            tx.commit().await?;
        }
    }
    Ok(())
}
