use super::{SyncStatus, bootstrap, budget, catalog, client::Client, project, status};
use crate::{
    CoreError, ErrorCode, Result, events::ChangeKind, runtime::Services, tasks::TaskControl,
};
use memedock_domain::{
    archive::ArchiveData,
    change::{Operation, OperationKind},
    identity::OperationId,
    local::ChangeStatus,
    sync::{OperationEnvelope, ReceiptOutcome},
    version::{LocalOrder, Revision},
};
use memedock_storage::client_sync::{Binding, SyncJob};
use std::sync::Arc;

pub(super) async fn run(
    services: &Arc<Services>,
    token: String,
    control: &Arc<TaskControl>,
) -> Result<SyncStatus> {
    let Some(binding) = services.db.sync_binding(budget(services)).await? else {
        return status(services).await;
    };
    if binding.paused {
        return status(services).await;
    }
    let client = Client::new(&binding.origin, token)?;
    if binding.initialized {
        enqueue(services, false).await?;
    }
    let mut complete = false;
    for _ in 0..16 {
        control.check()?;
        if !pull(services, &client, control).await? {
            complete = true;
            break;
        }
    }
    if !binding.initialized && !complete {
        return status(services).await;
    }
    if !binding.initialized {
        initialize(services).await?;
    }
    enqueue(services, false).await?;
    for mut job in services
        .db
        .sync_jobs(budget(services))
        .await?
        .into_iter()
        .filter(|j| !j.done && j.accepted.is_none())
        .take(16)
    {
        control.check()?;
        if !job.attempted
            && let OperationKind::PurgeEntity {
                entity, generation, ..
            } = job.envelope.operation().kind()
        {
            let binding = required(services).await?;
            let life = match entity {
                memedock_domain::lifecycle::EntityId::Sticker(id) => binding
                    .canonical
                    .stickers
                    .iter()
                    .find(|v| v.id() == *id)
                    .map(|v| v.lifecycle()),
                memedock_domain::lifecycle::EntityId::Collection(id) => binding
                    .canonical
                    .collections
                    .iter()
                    .find(|v| v.id() == *id)
                    .map(|v| v.lifecycle()),
                memedock_domain::lifecycle::EntityId::Tag(id) => binding
                    .canonical
                    .tags
                    .iter()
                    .find(|v| v.id() == *id)
                    .map(|v| v.lifecycle()),
            }
            .cloned()
            .or_else(|| {
                binding
                    .canonical
                    .tombstones
                    .iter()
                    .find(|t| t.entity() == *entity)
                    .map(|t| {
                        memedock_domain::lifecycle::Lifecycle::from_state(
                            t.generation(),
                            t.revision(),
                            t.deleted_at(),
                            t.deleted_at(),
                            Some(t.deleted_at()),
                        )
                        .with_purged_at(t.purged_at())
                    })
            });
            if let Some(life) = life.filter(|v| v.generation() == *generation) {
                // A local soft delete may still be awaiting its canonical event.
                // Never freeze a revision before that preceding operation is pulled.
                if life.is_active() {
                    continue;
                }
                job.envelope = OperationEnvelope::new(
                    binding.library_id,
                    binding.epoch,
                    binding.device_id,
                    job.envelope.op_id(),
                    job.envelope.client_time_ms(),
                    Operation::new(OperationKind::PurgeEntity {
                        entity: *entity,
                        generation: *generation,
                        expected_deleted_revision: life.revision(),
                    })?,
                );
            }
        }
        if let OperationKind::CreateSticker { asset, .. } = job.envelope.operation().kind()
            && services
                .db
                .sticker(asset.sticker_id())
                .await?
                .is_some_and(|v| v.lifecycle().is_purged())
        {
            block(services, &mut job, "superseded_by_permanent_delete").await?;
            continue;
        }
        let upload = match job.envelope.operation().kind() {
            OperationKind::CreateSticker { asset, .. } => {
                client.upload(asset, &services.blobs, control).await
            }
            OperationKind::RestoreSticker {
                rebuild: Some(rebuild),
                ..
            }
            | OperationKind::ReimportSticker { rebuild, .. } => {
                client
                    .upload(&rebuild.asset, &services.blobs, control)
                    .await
            }
            _ => Ok(()),
        };
        if let Err(error) = upload {
            if matches!(
                error.code(),
                ErrorCode::InvalidInput | ErrorCode::ResourceLimit
            ) {
                block(services, &mut job, error.code().as_str()).await?;
                continue;
            }
            return Err(error);
        }
        {
            let _write = services
                .write_permit
                .acquire()
                .await
                .map_err(|_| CoreError::internal("write service closed"))?;
            let mut tx = services.db.begin_write().await?;
            if let Some(order) = job.source_order
                && let Some(mut source) = tx.sync_source(order).await?
            {
                if source.status() == ChangeStatus::Sending {
                    source.retry(crate::writes::now()?, "retry".into())?;
                }
                if source.status() == ChangeStatus::Pending {
                    source.begin_send()?;
                    tx.save_change(&source).await?;
                }
            }
            job.attempted = true;
            tx.save_sync_job(&job).await?;
            tx.commit().await?;
        }
        let response = match client.push(job.envelope.clone(), control).await {
            Ok(response) => response,
            Err(error)
                if matches!(
                    error.code(),
                    ErrorCode::InvalidInput | ErrorCode::ResourceLimit
                ) =>
            {
                block(services, &mut job, error.code().as_str()).await?;
                continue;
            }
            Err(error) => return Err(error),
        };
        if response.receipt.op_id != job.envelope.op_id()
            || response.receipt.device_id != job.envelope.device_id()
        {
            return Err(CoreError::new(
                ErrorCode::CorruptData,
                "sync receipt identity mismatch",
            ));
        }
        let _write = services
            .write_permit
            .acquire()
            .await
            .map_err(|_| CoreError::internal("write service closed"))?;
        let mut tx = services.db.begin_write().await?;
        match response.receipt.outcome {
            ReceiptOutcome::Accepted { event_seq, .. } => job.accepted = Some(event_seq.into()),
            ReceiptOutcome::NoOp { canonical_revision } => job.accepted = Some(canonical_revision),
            ReceiptOutcome::Rejected { code } => {
                job.done = true;
                job.error = Some(format!("{code:?}"));
            }
        }
        if let Some(order) = job.source_order
            && let Some(mut source) = tx.sync_source(order).await?
        {
            if let Some(revision) = job.accepted {
                source.accepted(revision)?;
            } else {
                source.block(job.error.clone().unwrap_or_else(|| "rejected".into()))?;
            }
            tx.save_change(&source).await?;
        }
        tx.save_sync_job(&job).await?;
        tx.commit().await?;
        if matches!(
            job.envelope.operation().kind(),
            OperationKind::DeleteSticker { .. }
                | OperationKind::DeleteCollection { .. }
                | OperationKind::DeleteTag { .. }
        ) {
            // Pull the assigned deletion revision before sending a dependent purge.
            break;
        }
    }
    for _ in 0..16 {
        control.check()?;
        if !pull(services, &client, control).await? {
            break;
        }
    }
    download(services, &client, control).await?;
    crate::use_cases::purge::cleanup(services).await?;
    let _write = services
        .write_permit
        .acquire()
        .await
        .map_err(|_| CoreError::internal("write service closed"))?;
    let mut binding = required(services).await?;
    binding.last_success_ms = Some(crate::writes::now()?.get());
    binding.last_error = None;
    let mut tx = services.db.begin_write().await?;
    tx.delete_finished_sync_jobs().await?;
    tx.commit_sync(&binding, None, budget(services)).await?;
    status(services).await
}
async fn required(services: &Services) -> Result<Binding> {
    services
        .db
        .sync_binding(budget(services))
        .await?
        .ok_or_else(|| CoreError::new(ErrorCode::Conflict, "sync binding missing"))
}
async fn block(services: &Services, job: &mut SyncJob, code: &str) -> Result<()> {
    let _write = services
        .write_permit
        .acquire()
        .await
        .map_err(|_| CoreError::internal("write service closed"))?;
    let mut tx = services.db.begin_write().await?;
    job.done = true;
    job.error = Some(code.into());
    if let Some(order) = job.source_order
        && let Some(mut source) = tx.sync_source(order).await?
    {
        source.block(code.into())?;
        tx.save_change(&source).await?;
    }
    tx.save_sync_job(job).await?;
    tx.commit().await?;
    Ok(())
}
fn job(
    binding: &Binding,
    kind: OperationKind,
    id: OperationId,
    source_order: Option<i64>,
    at: memedock_domain::version::TimestampMs,
) -> Result<SyncJob> {
    Ok(SyncJob {
        ordinal: 0,
        source_order,
        envelope: OperationEnvelope::new(
            binding.library_id,
            binding.epoch,
            binding.device_id,
            id,
            at,
            Operation::new(kind)?,
        ),
        attempted: false,
        accepted: None,
        done: false,
        projected: false,
        error: None,
    })
}
async fn initialize(services: &Services) -> Result<()> {
    let _write = services
        .write_permit
        .acquire()
        .await
        .map_err(|_| CoreError::internal("write service closed"))?;
    let mut binding = required(services).await?;
    let mut tx = services.db.begin_write().await?;
    for kind in bootstrap::operations(&binding.baseline, &binding.canonical) {
        tx.enqueue_sync(
            &job(
                &binding,
                kind,
                OperationId::new(),
                None,
                crate::writes::now()?,
            )?,
            budget(services),
        )
        .await?;
    }
    enqueue_in(services, &mut tx, &mut binding, true).await?;
    binding.initialized = true;
    let mut view = services
        .db
        .archive_data_bounded(services.config.limits.max_archive_metadata_bytes)
        .await?;
    adopt_generations(&mut view, &binding)?;
    catalog::merge(&mut view, &binding.canonical)?;
    for job in tx
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
            eprintln!("initial sync overlay deferred: {}", error.code().as_str());
        }
    }
    catalog::clean(&mut view);
    tx.commit_sync(&binding, Some(&view), budget(services))
        .await?;
    notify(services, &view)?;
    Ok(())
}
async fn enqueue(services: &Services, rebase: bool) -> Result<()> {
    let _write = services
        .write_permit
        .acquire()
        .await
        .map_err(|_| CoreError::internal("write service closed"))?;
    let mut binding = required(services).await?;
    let mut tx = services.db.begin_write().await?;
    enqueue_in(services, &mut tx, &mut binding, rebase).await?;
    tx.commit_sync(&binding, None, budget(services)).await?;
    Ok(())
}
async fn enqueue_in(
    services: &Services,
    tx: &mut memedock_storage::transaction::WriteTransaction,
    binding: &mut Binding,
    rebase: bool,
) -> Result<()> {
    loop {
        let after = if binding.enqueued == 0 {
            None
        } else {
            Some(LocalOrder::new(binding.enqueued)?)
        };
        let changes = services.db.changes_after(after, 128).await?;
        if changes.is_empty() {
            break;
        }
        for mut source in changes {
            binding.enqueued = source.local_order().get();
            if !source.contributes_overlay() {
                continue;
            }
            if let OperationKind::ImportArchive { state, .. } = source.operation().kind() {
                // Expand a local restore audit into transportable, idempotent commands.
                let mut expected = binding.canonical.clone();
                for queued in tx
                    .sync_jobs(budget(services))
                    .await?
                    .iter()
                    .filter(|j| !j.done)
                {
                    project::overlay(
                        &mut expected,
                        queued.envelope.operation().kind(),
                        queued.envelope.client_time_ms(),
                    )?;
                }
                for kind in super::reconcile::operations(state, &expected, source.created_at())? {
                    tx.enqueue_sync(
                        &job(binding, kind, OperationId::new(), None, source.created_at())?,
                        budget(services),
                    )
                    .await?;
                }
                source.seal_local()?;
                tx.save_change(&source).await?;
                continue;
            }
            let mut kind = source.operation().kind().clone();
            if rebase {
                kind = bootstrap::rebase(kind, &binding.baseline, &binding.canonical);
            }
            tx.enqueue_sync(
                &job(
                    binding,
                    kind,
                    source.op_id(),
                    Some(source.local_order().get()),
                    source.created_at(),
                )?,
                budget(services),
            )
            .await?;
        }
    }
    Ok(())
}
async fn pull(services: &Services, client: &Client, control: &TaskControl) -> Result<bool> {
    let binding = required(services).await?;
    let page = client.pull(&binding, control).await?;
    let more = page.has_more();
    let _write = services
        .write_permit
        .acquire()
        .await
        .map_err(|_| CoreError::internal("write service closed"))?;
    let mut binding = required(services).await?;
    let mut jobs = services.db.sync_jobs(budget(services)).await?;
    if page.events().is_empty()
        && !jobs.iter().any(|j| j.done && !j.projected)
        && !jobs
            .iter()
            .any(|j| !j.done && j.accepted.is_some_and(|r| r <= page.next_cursor()))
    {
        return Ok(more);
    }
    let mut tx = services.db.begin_write().await?;
    for event in page.events() {
        catalog::event(&mut binding.canonical, event)?;
        for job in jobs.iter_mut().filter(|j| {
            !j.done
                && j.envelope.device_id() == event.device_id()
                && j.envelope.op_id() == event.op_id()
        }) {
            if !job.attempted {
                return Err(CoreError::new(
                    ErrorCode::CorruptData,
                    "unattempted operation appeared remotely",
                ));
            }
            job.accepted = Some(event.seq().into());
            job.done = true;
            if let Some(order) = job.source_order
                && let Some(mut source) = tx.sync_source(order).await?
                && source.contributes_overlay()
            {
                source.confirm_event(event.op_id(), event.seq())?;
                tx.save_change(&source).await?;
            }
            tx.save_sync_job(job).await?;
        }
    }
    binding.cursor = page.next_cursor();
    for job in jobs
        .iter_mut()
        .filter(|j| !j.done && j.accepted.is_some_and(|r| r <= binding.cursor))
    {
        job.done = true;
        if let Some(order) = job.source_order
            && let Some(mut source) = tx.sync_source(order).await?
            && source.status() == ChangeStatus::AcceptedWaitingPull
        {
            source.confirm_canonical(binding.cursor)?;
            tx.save_change(&source).await?;
        }
        tx.save_sync_job(job).await?;
    }
    let view = if binding.initialized {
        let mut view = services
            .db
            .archive_data_bounded(services.config.limits.max_archive_metadata_bytes)
            .await?;
        catalog::merge(&mut view, &binding.canonical)?;
        for job in jobs.iter().filter(|j| !j.done) {
            if let Err(error) = project::overlay(
                &mut view,
                job.envelope.operation().kind(),
                job.envelope.client_time_ms(),
            ) {
                eprintln!("sync overlay deferred: {}", error.code().as_str());
            }
        }
        // Include edits committed while the network request was in flight.
        let mut after = if binding.enqueued == 0 {
            None
        } else {
            Some(LocalOrder::new(binding.enqueued)?)
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
                    eprintln!("sync overlay deferred: {}", error.code().as_str());
                }
            }
        }
        catalog::clean(&mut view);
        Some(view)
    } else {
        None
    };
    control.check()?;
    for job in jobs.iter_mut().filter(|j| j.done && !j.projected) {
        job.projected = true;
        tx.save_sync_job(job).await?;
    }
    tx.commit_sync(&binding, view.as_ref(), budget(services))
        .await?;
    if let Some(view) = view {
        notify(services, &view)?;
    }
    Ok(more)
}
fn notify(services: &Services, view: &ArchiveData) -> Result<()> {
    services.events.publish(ChangeKind::CollectionsChanged)?;
    services.events.publish(ChangeKind::TagsChanged)?;
    for sticker in &view.stickers {
        services
            .events
            .publish(ChangeKind::StickerChanged(sticker.id()))?;
    }
    Ok(())
}
fn adopt_generations(view: &mut ArchiveData, binding: &Binding) -> Result<()> {
    use memedock_domain::{
        collection::Collection,
        lifecycle::Lifecycle,
        relation::{CollectionItem, StickerTag},
        sticker::Sticker,
        tag::Tag,
        version::Generation,
    };
    let life = |v: &Lifecycle| {
        Lifecycle::from_state(
            Generation::INITIAL,
            Revision::LOCAL,
            v.created_at(),
            v.updated_at(),
            v.deleted_at(),
        )
        .with_purged_at(v.purged_at())
    };
    for v in &mut view.stickers {
        if binding
            .baseline
            .stickers
            .iter()
            .any(|b| b.id() == v.id() && b.lifecycle().is_active())
            && !binding.canonical.stickers.iter().any(|r| r.id() == v.id())
        {
            *v = Sticker::from_state(
                v.id(),
                v.title().into(),
                v.original_name().into(),
                v.note().into(),
                v.starred(),
                life(v.lifecycle()),
            );
        }
    }
    for v in &mut view.collections {
        if binding
            .baseline
            .collections
            .iter()
            .any(|b| b.id() == v.id() && b.lifecycle().is_active())
            && !binding
                .canonical
                .collections
                .iter()
                .any(|r| r.id() == v.id())
        {
            *v = Collection::from_state(
                v.id(),
                v.name().clone(),
                v.sort_key().clone(),
                life(v.lifecycle()),
            );
        }
    }
    for v in &mut view.tags {
        if binding
            .baseline
            .tags
            .iter()
            .any(|b| b.id() == v.id() && b.lifecycle().is_active())
            && !binding.canonical.tags.iter().any(|r| r.id() == v.id())
        {
            *v = Tag::from_state(
                v.id(),
                v.name().clone(),
                v.normalized_name(),
                life(v.lifecycle()),
            )?;
        }
    }
    for v in &mut view.collection_items {
        if let (Some(c), Some(s)) = (
            view.collections
                .iter()
                .find(|c| c.id() == v.collection_id()),
            view.stickers.iter().find(|s| s.id() == v.sticker_id()),
        ) {
            *v = CollectionItem::from_state(
                (v.collection_id(), c.lifecycle().generation()),
                (v.sticker_id(), s.lifecycle().generation()),
                v.present(),
                v.sort_key().clone(),
                Revision::LOCAL,
                v.updated_at(),
            );
        }
    }
    for v in &mut view.sticker_tags {
        if let (Some(s), Some(t)) = (
            view.stickers.iter().find(|s| s.id() == v.sticker_id()),
            view.tags.iter().find(|t| t.id() == v.tag_id()),
        ) {
            *v = StickerTag::from_state(
                (v.sticker_id(), s.lifecycle().generation()),
                (v.tag_id(), t.lifecycle().generation()),
                v.present(),
                Revision::LOCAL,
                v.updated_at(),
            );
        }
    }
    Ok(())
}
async fn download(
    services: &Arc<Services>,
    client: &Client,
    control: &Arc<TaskControl>,
) -> Result<()> {
    let binding = required(services).await?;
    let mut missing = Vec::new();
    for assets in binding.canonical.assets.chunks(200) {
        let hashes: Vec<_> = assets.iter().map(|a| a.hash()).collect();
        for (asset, local) in services.db.resource_rows(&hashes).await? {
            if binding
                .canonical
                .stickers
                .iter()
                .any(|v| v.id() == asset.sticker_id() && v.lifecycle().is_purged())
            {
                continue;
            }
            if !local.is_some_and(|v| v.blob_status() == memedock_domain::local::BlobStatus::Ready)
            {
                missing.push(asset);
                if missing.len() == 8 {
                    break;
                }
            }
        }
        if missing.len() == 8 {
            break;
        }
    }
    for asset in &missing {
        control.check()?;
        let blobs = services.blobs.clone();
        let pending = tokio::task::spawn_blocking(move || blobs.create_staging()).await??;
        if let Err(error) = client.download(asset, &pending, control).await {
            tokio::task::spawn_blocking(move || pending.discard()).await??;
            return Err(error);
        }
        let blobs = services.blobs.clone();
        let expected = asset.clone();
        let check = control.clone();
        let _write = services
            .write_permit
            .acquire()
            .await
            .map_err(|_| CoreError::internal("write service closed"))?;
        if services
            .db
            .sticker(asset.sticker_id())
            .await?
            .is_some_and(|v| v.lifecycle().is_purged())
        {
            tokio::task::spawn_blocking(move || pending.discard()).await??;
            continue;
        }
        tokio::task::spawn_blocking(move || -> Result<()> {
            let staged =
                pending.finish(expected.byte_size().get() as u64, || check.is_cancelled())?;
            if staged.hash() != expected.hash() {
                staged.discard()?;
                return Err(CoreError::new(
                    ErrorCode::CorruptData,
                    "downloaded original hash mismatch",
                ));
            }
            blobs.publish(staged, || check.is_cancelled())?;
            Ok(())
        })
        .await??;
        let mut tx = services.db.begin_write().await?;
        let mut local = tx
            .local_asset(asset.hash())
            .await?
            .unwrap_or_else(|| memedock_domain::local::LocalAsset::new(asset.hash()));
        local.verified(crate::writes::now()?);
        tx.save_local_asset(&local).await?;
        tx.commit().await?;
        services
            .events
            .publish(ChangeKind::StickerChanged(asset.sticker_id()))?;
    }
    Ok(())
}
