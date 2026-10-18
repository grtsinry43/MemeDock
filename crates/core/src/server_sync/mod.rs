//! Server-authoritative use cases. No HTTP or platform credential dependencies.
mod apply;
mod assets;
pub use assets::{prepare_thumbnail, validate_original};
/// Caller serializes this with asset publication and sync writes.
pub async fn cleanup_purged(
    db: &ServerDatabase,
    root: &std::path::Path,
    cache: &std::path::Path,
) -> Result<()> {
    loop {
        let jobs = db.server_purge_jobs().await?;
        if jobs.is_empty() {
            return Ok(());
        }
        for (hash, generation) in jobs {
            if db.server_sticker(hash).await?.is_none_or(|v| {
                v.lifecycle().is_purged() && v.lifecycle().generation() == generation
            }) {
                let root = root.to_owned();
                let cache = cache.to_owned();
                tokio::task::spawn_blocking(move || -> Result<()> {
                    memedock_storage::files::FsBlobStore::open(root)?.remove_original(hash)?;
                    std::fs::create_dir_all(&cache)?;
                    memedock_storage::files::DerivedStore::open(&cache)?.remove_for(hash)?;
                    Ok(())
                })
                .await
                .map_err(|_| StorageError::Integrity("purge worker failed"))??;
                db.forget_server_asset(hash).await?;
            }
            db.finish_server_purge(hash, generation).await?;
        }
    }
}
use memedock_domain::{
    identity::DeviceId,
    sync::{OperationEnvelope, OperationReceipt, ReceiptOutcome, RejectionCode, SyncEvent},
    version::{EventSeq, TimestampMs},
};
use memedock_storage::{Result, StorageError, server::ServerDatabase};

pub async fn push(
    db: &ServerDatabase,
    device: DeviceId,
    op: OperationEnvelope,
    now: TimestampMs,
) -> Result<OperationReceipt> {
    let tx = db.begin_sync(device).await?;
    let code = if op.device_id() != device {
        Some(RejectionCode::InvalidOperation)
    } else if op.library_id() != tx.identity.library_id {
        Some(RejectionCode::LibraryMismatch)
    } else if op.sync_epoch() != tx.identity.epoch {
        Some(RejectionCode::EpochMismatch)
    } else {
        None
    };
    // Identity mismatches must never poison another device's operation namespace.
    if let Some(code) = code {
        return Ok(OperationReceipt {
            device_id: device,
            op_id: op.op_id(),
            outcome: ReceiptOutcome::Rejected { code },
        });
    }
    match tx.prior_receipt(&op).await {
        Ok(Some(receipt)) => return Ok(receipt),
        Err(StorageError::Conflict(_)) => {
            return Ok(OperationReceipt {
                device_id: device,
                op_id: op.op_id(),
                outcome: ReceiptOutcome::Rejected {
                    code: RejectionCode::OpIdReused,
                },
            });
        }
        Err(error) => return Err(error),
        Ok(None) => {}
    }
    let seq = EventSeq::new(tx.identity.head.checked_next()?.get())?;
    let applied = apply::apply(&tx, op.operation().kind(), seq.into(), now).await;
    let (outcome, event) = match applied {
        Ok((Some(kind), note)) => (
            ReceiptOutcome::Accepted {
                event_seq: seq,
                note,
            },
            Some(SyncEvent::new(seq, device, op.op_id(), now, kind)?),
        ),
        Ok((None, _)) => (
            ReceiptOutcome::NoOp {
                canonical_revision: tx.identity.head,
            },
            None,
        ),
        Err(apply::ApplyError::Rejected(code)) => (ReceiptOutcome::Rejected { code }, None),
        Err(apply::ApplyError::Storage(error)) => return Err(error),
    };
    let receipt = OperationReceipt {
        device_id: device,
        op_id: op.op_id(),
        outcome,
    };
    tx.finish(&op, &receipt, event.as_ref(), now).await?;
    Ok(receipt)
}
