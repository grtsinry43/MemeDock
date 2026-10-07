use super::sticker_management::TagUpdate;
use crate::{
    CoreError, ErrorCode, Library, Result,
    batch::{
        BatchAction, BatchController, BatchItemResult, BatchOutcome, BatchReport, BatchState,
        BatchTarget, BatchTask, MAX_BATCH_ITEMS, wait_item,
    },
    tasks::{Priority, scheduler::Lane},
};
use memedock_domain::change::{FieldPatch, StickerPatch};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::watch;

pub(crate) fn start(
    library: &Library,
    targets: Vec<BatchTarget>,
    action: BatchAction,
) -> Result<BatchTask> {
    if targets.is_empty() || targets.len() > MAX_BATCH_ITEMS {
        return Err(CoreError::new(
            ErrorCode::ResourceLimit,
            "invalid batch size",
        ));
    }
    if matches!(&action, BatchAction::AddTags(v) | BatchAction::RemoveTags(v) if v.is_empty() || v.len() > MAX_BATCH_ITEMS)
    {
        return Err(CoreError::new(
            ErrorCode::InvalidInput,
            "invalid tag selection",
        ));
    }
    let mut seen = BTreeSet::new();
    let targets: Vec<_> = targets.into_iter().filter(|v| seen.insert(v.id)).collect();
    let initial = BatchReport {
        items: targets
            .iter()
            .map(|v| BatchItemResult {
                id: v.id,
                outcome: BatchOutcome::Pending,
            })
            .collect(),
        stopped: false,
    };
    let (report, _) = watch::channel(initial);
    let state = Arc::new(BatchState {
        stop: AtomicBool::new(false),
        current: Mutex::new(None),
        report,
    });
    let tracked = Arc::clone(&state);
    let owner = library.clone();
    // Orchestration never holds the write lane while awaiting a child write task.
    let task = library.submit(
        Lane::Read,
        Priority::Interactive,
        move |_, control| async move {
            for (position, target) in targets.into_iter().enumerate() {
                if tracked.stop.load(Ordering::Acquire) || control.is_cancelled() {
                    break;
                }
                let result = apply(&owner, &tracked, target, &action).await;
                let outcome = match result {
                    Ok(true) => BatchOutcome::Applied,
                    Ok(false) => BatchOutcome::Unchanged,
                    Err(error) if error.code() == ErrorCode::Cancelled => {
                        tracked.stop.store(true, Ordering::Release);
                        break;
                    }
                    Err(error) => BatchOutcome::Failed(error.code()),
                };
                tracked
                    .report
                    .send_modify(|r| r.items[position].outcome = outcome);
            }
            tracked.report.send_modify(|r| {
                r.stopped = tracked.stop.load(Ordering::Acquire) || control.is_cancelled()
            });
            Ok(tracked.report.borrow().clone())
        },
    )?;
    Ok(BatchTask {
        task,
        controller: BatchController { state },
    })
}

async fn apply(
    library: &Library,
    state: &BatchState,
    target: BatchTarget,
    action: &BatchAction,
) -> Result<bool> {
    match action {
        BatchAction::Assign(id, generation) => {
            wait_item(
                state,
                library.edit_organization(
                    target.id,
                    target.generation,
                    Some(Some((*id, *generation))),
                    None,
                )?,
            )
            .await
        }
        BatchAction::ClearCollection(expected) => {
            wait_item(
                state,
                library.clear_sticker_collection(target.id, target.generation, *expected)?,
            )
            .await
        }
        BatchAction::AddTags(tags) => {
            wait_item(
                state,
                library.edit_organization(
                    target.id,
                    target.generation,
                    None,
                    Some(TagUpdate::Add(tags.clone())),
                )?,
            )
            .await
        }
        BatchAction::RemoveTags(tags) => {
            wait_item(
                state,
                library.edit_organization(
                    target.id,
                    target.generation,
                    None,
                    Some(TagUpdate::Remove(tags.clone())),
                )?,
            )
            .await
        }
        BatchAction::Star(value) => wait_item(
            state,
            library.patch_sticker(
                target.id,
                target.generation,
                StickerPatch::new(
                    FieldPatch::Missing,
                    FieldPatch::Missing,
                    FieldPatch::Set(*value),
                )?,
            )?,
        )
        .await
        .map(|_| true),
        BatchAction::Delete => {
            wait_item(state, library.delete_sticker(target.id, target.generation)?)
                .await
                .map(|_| true)
        }
        BatchAction::Restore => wait_item(
            state,
            library.restore_sticker(
                target.id,
                target.generation,
                target.deleted_revision.ok_or_else(|| {
                    CoreError::new(ErrorCode::InvalidInput, "missing deleted revision")
                })?,
            )?,
        )
        .await
        .map(|_| true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memedock_domain::{
        identity::{ContentHash, StickerId},
        version::Generation,
    };
    use std::time::Duration;

    #[tokio::test]
    async fn cancellation_preserves_pending_items_before_a_queued_write_starts()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let library = Library::open(crate::LibraryConfig::new(
            directory.path().join("data"),
            directory.path().join("cache"),
            directory.path().join("share"),
        ))
        .await?;
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = tokio::sync::oneshot::channel();
        let blocker =
            library.submit(Lane::Write, Priority::Interactive, move |_, _| async move {
                let _ = started.send(());
                wait.await
                    .map_err(|_| CoreError::internal("test release dropped"))?;
                Ok(())
            })?;
        tokio::time::timeout(Duration::from_secs(5), ready).await??;
        let targets = (1..=3)
            .map(|byte| BatchTarget {
                id: StickerId::new(ContentHash::from_bytes([byte; 32])),
                generation: Generation::INITIAL,
                deleted_revision: None,
            })
            .collect();
        let batch = library.batch(targets, BatchAction::Delete)?;
        batch.cancel()?;
        release.send(()).map_err(|_| "release dropped")?;
        blocker.wait().await?;
        let report = tokio::time::timeout(Duration::from_secs(5), batch.wait()).await??;
        assert!(report.stopped);
        assert_eq!(report.items.len(), 3);
        assert!(
            report
                .items
                .iter()
                .all(|item| item.outcome == BatchOutcome::Pending)
        );
        library.close().await?;
        Ok(())
    }
}
