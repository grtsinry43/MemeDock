use super::{budget, catalog, client::Client, diagnostics, project, worker};
use crate::{CoreError, ErrorCode, Result, runtime::Services, tasks::TaskControl};
use memedock_domain::{archive::ArchiveData, local::BlobStatus, version::LocalOrder};
use memedock_storage::client_sync::transfers::Transfer;
use std::{collections::HashSet, sync::Arc};

/// Caller owns the write permit. Rebuild against the latest canonical state
/// and local edits rather than publishing the metadata captured at download start.
pub(super) async fn refresh_locked(services: &Services) -> Result<()> {
    let mut binding = worker::required(services).await?;
    let live = services
        .db
        .archive_data_bounded(services.config.limits.max_archive_metadata_bytes)
        .await?;
    let mut view = binding.deferred.clone();
    catalog::merge(&mut view, &live)?;
    catalog::merge(&mut view, &binding.canonical)?;
    for job in services
        .db
        .sync_jobs(budget(services))
        .await?
        .iter()
        .filter(|j| !j.done)
    {
        if let Err(error) = project::overlay(
            &mut view,
            job.envelope.operation().kind(),
            job.envelope.client_time_ms(),
        ) {
            diagnostics::failed(
                &mut binding,
                &error,
                "local overlay during recovery",
                None,
                true,
            )?;
        }
    }
    let mut after = if binding.enqueued > 0 {
        Some(LocalOrder::new(binding.enqueued)?)
    } else {
        None
    };
    loop {
        let changes = services.db.changes_after(after, 128).await?;
        if changes.is_empty() {
            break;
        }
        for change in changes {
            after = Some(change.local_order());
            if change.contributes_overlay()
                && let Err(error) =
                    project::overlay(&mut view, change.operation().kind(), change.created_at())
            {
                diagnostics::failed(
                    &mut binding,
                    &error,
                    "concurrent local overlay during recovery",
                    None,
                    true,
                )?;
            }
        }
    }
    catalog::clean(&mut view);
    let tx = services.db.begin_write().await?;
    tx.commit_sync(&binding, Some(&view), budget(services))
        .await?;
    worker::notify(services, &view)?;
    super::status(services).await?;
    Ok(())
}

pub(super) async fn run(
    services: &Arc<Services>,
    client: &Client,
    control: &Arc<TaskControl>,
) -> Result<()> {
    let binding = worker::required(services).await?;
    let mut desired = ArchiveData::default();
    catalog::merge(&mut desired, &binding.deferred)?;
    catalog::merge(&mut desired, &binding.canonical)?;
    let mut transfers = services.db.sync_transfers(budget(services)).await?;
    let mut wanted = HashSet::new();
    {
        let _write = services
            .write_permit
            .acquire()
            .await
            .map_err(|_| CoreError::internal("write service closed"))?;
        let mut tx = services.db.begin_write().await?;
        for asset in &desired.assets {
            if !desired
                .stickers
                .iter()
                .any(|s| s.id() == asset.sticker_id() && !s.lifecycle().is_purged())
            {
                continue;
            }
            if services
                .db
                .sticker(asset.sticker_id())
                .await?
                .is_some_and(|s| s.lifecycle().is_purged())
            {
                continue;
            }
            if services
                .db
                .resource_rows(&[asset.hash()])
                .await?
                .first()
                .is_some_and(|(_, local)| {
                    local
                        .as_ref()
                        .is_some_and(|l| l.blob_status() == BlobStatus::Ready)
                })
            {
                continue;
            }
            wanted.insert(asset.hash());
            if !transfers.iter().any(|t| t.asset.hash() == asset.hash()) {
                let transfer = Transfer::new(asset.clone());
                tx.save_sync_transfer(&transfer).await?;
                transfers.push(transfer);
            }
        }
        for transfer in &transfers {
            if !wanted.contains(&transfer.asset.hash()) {
                tx.delete_sync_transfer(transfer.asset.hash()).await?;
            }
        }
        tx.commit().await?;
    }
    transfers.sort_by_key(|t| t.next_retry_ms);
    let now = crate::writes::now()?.get();
    for mut transfer in transfers
        .into_iter()
        .filter(|t| wanted.contains(&t.asset.hash()) && !t.blocked && t.next_retry_ms <= now)
        .take(8)
    {
        control.check()?;
        transfer.attempts = transfer.attempts.saturating_add(1);
        {
            let _write = services
                .write_permit
                .acquire()
                .await
                .map_err(|_| CoreError::internal("write service closed"))?;
            let mut tx = services.db.begin_write().await?;
            // A crash consumes a bounded backoff too; it cannot create a hot loop.
            transfer.next_retry_ms = now.saturating_add(diagnostics::delay(transfer.attempts));
            tx.save_sync_transfer(&transfer).await?;
            tx.commit().await?;
        }
        // Reuse a fully published file after a crash between file publication
        // and its database commit. Never trust existence without verification.
        let blobs = services.blobs.clone();
        let asset = transfer.asset.clone();
        let check = control.clone();
        let reused = tokio::task::spawn_blocking(move || {
            blobs.verify(asset.hash(), asset.byte_size().get() as u64, || {
                check.is_cancelled()
            })
        })
        .await?;
        let result = match reused {
            Ok(()) => Ok(()),
            Err(memedock_storage::StorageError::Cancelled) => {
                return Err(CoreError::new(ErrorCode::Cancelled, "sync cancelled"));
            }
            Err(_) => worker::download_one(services, client, control, &transfer.asset).await,
        };
        let _write = services
            .write_permit
            .acquire()
            .await
            .map_err(|_| CoreError::internal("write service closed"))?;
        match result {
            Ok(()) => {
                if services
                    .db
                    .sticker(transfer.asset.sticker_id())
                    .await?
                    .is_some_and(|s| s.lifecycle().is_purged())
                {
                    let mut tx = services.db.begin_write().await?;
                    tx.delete_sync_transfer(transfer.asset.hash()).await?;
                    tx.commit().await?;
                    continue;
                }
                let mut tx = services.db.begin_write().await?;
                let mut local = tx
                    .local_asset(transfer.asset.hash())
                    .await?
                    .unwrap_or_else(|| {
                        memedock_domain::local::LocalAsset::new(transfer.asset.hash())
                    });
                local.verified(crate::writes::now()?);
                tx.save_local_asset(&local).await?;
                tx.delete_sync_transfer(transfer.asset.hash()).await?;
                tx.commit().await?;
                refresh_locked(services).await?;
            }
            Err(error) => {
                if error.code() == ErrorCode::EntityDeleted {
                    let mut tx = services.db.begin_write().await?;
                    tx.delete_sync_transfer(transfer.asset.hash()).await?;
                    tx.commit().await?;
                    continue;
                }
                if matches!(
                    error.code(),
                    ErrorCode::Cancelled
                        | ErrorCode::Unauthorized
                        | ErrorCode::UnsupportedSchema
                        | ErrorCode::StorageFull
                        | ErrorCode::PermissionDenied
                        | ErrorCode::Io
                        | ErrorCode::Database
                        | ErrorCode::Network
                        | ErrorCode::Timeout
                        | ErrorCode::RateLimited
                ) {
                    let detail = diagnostics::diagnostic(
                        &error,
                        "download",
                        Some(transfer.asset.hash().to_string()),
                        transfer.attempts,
                    )?;
                    transfer.error = Some(detail.clone());
                    let mut tx = services.db.begin_write().await?;
                    tx.save_sync_transfer(&transfer).await?;
                    tx.commit().await?;
                    return Err(error.with_sync_diagnostic(detail));
                }
                transfer.blocked = transfer.attempts >= 5
                    || matches!(
                        error.code(),
                        ErrorCode::InvalidInput | ErrorCode::ResourceLimit
                    );
                let mut detail = diagnostics::diagnostic(
                    &error,
                    "download",
                    Some(transfer.asset.hash().to_string()),
                    transfer.attempts,
                )?;
                detail.next_retry_ms = if transfer.blocked {
                    None
                } else {
                    Some(transfer.next_retry_ms)
                };
                transfer.error = Some(detail.clone());
                let mut binding = worker::required(services).await?;
                binding.last_error = Some(error.code().as_str().into());
                memedock_storage::client_sync::diagnostics::record(&mut binding, detail);
                let mut tx = services.db.begin_write().await?;
                tx.save_sync_transfer(&transfer).await?;
                tx.commit_sync(&binding, None, budget(services)).await?;
                super::status(services).await?;
            }
        }
    }
    Ok(())
}
