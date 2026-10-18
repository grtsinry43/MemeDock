mod bootstrap;
mod catalog;
mod client;
mod project;
mod reconcile;
mod worker;

use crate::{
    CoreError, ErrorCode, Library, Result,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_storage::client_sync::Binding;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncStatus {
    pub origin: Option<String>,
    pub device_id: Option<String>,
    pub connected: bool,
    pub paused: bool,
    pub pending: u64,
    pub blocked: u64,
    pub cursor: i64,
    pub last_success_ms: Option<i64>,
    pub error: Option<String>,
    pub retry_after_ms: u64,
}
pub struct SyncSubscription {
    receiver: tokio::sync::watch::Receiver<SyncStatus>,
    closed: tokio::sync::watch::Receiver<Option<Result<()>>>,
    first: bool,
}
impl SyncSubscription {
    pub(crate) fn new(
        receiver: tokio::sync::watch::Receiver<SyncStatus>,
        closed: tokio::sync::watch::Receiver<Option<Result<()>>>,
    ) -> Self {
        Self {
            receiver,
            closed,
            first: true,
        }
    }
    pub async fn next(&mut self) -> Result<Option<SyncStatus>> {
        if self.closed.borrow().is_some() {
            return Ok(None);
        }
        if self.first {
            self.first = false;
            return Ok(Some(self.receiver.borrow_and_update().clone()));
        }
        tokio::select! {changed=self.receiver.changed()=>{if changed.is_err(){return Ok(None);}Ok(Some(self.receiver.borrow_and_update().clone()))},_=self.closed.changed()=>Ok(None)}
    }
}
pub(super) fn budget(services: &crate::runtime::Services) -> u64 {
    services
        .config
        .limits
        .max_archive_metadata_bytes
        .saturating_mul(4)
}
pub(super) async fn status(services: &crate::runtime::Services) -> Result<SyncStatus> {
    let Some(binding) = services.db.sync_binding(budget(services)).await? else {
        services.sync_status.send_if_modified(|old| {
            if *old == SyncStatus::default() {
                false
            } else {
                *old = SyncStatus::default();
                true
            }
        });
        return Ok(SyncStatus::default());
    };
    let jobs = services.db.sync_jobs(budget(services)).await?;
    let mut result = SyncStatus {
        origin: Some(binding.origin),
        device_id: Some(binding.device_id.to_string()),
        connected: true,
        paused: binding.paused,
        cursor: binding.cursor.get(),
        last_success_ms: binding.last_success_ms,
        error: binding.last_error.clone(),
        retry_after_ms: 15_000,
        ..Default::default()
    };
    result.pending = jobs.iter().filter(|j| !j.done).count() as u64;
    result.blocked = jobs.iter().filter(|j| j.error.is_some()).count() as u64;
    for assets in binding.canonical.assets.chunks(200) {
        let hashes: Vec<_> = assets.iter().map(|a| a.hash()).collect();
        result.pending += services
            .db
            .resource_rows(&hashes)
            .await?
            .iter()
            .filter(|(asset, local)| {
                !binding
                    .canonical
                    .stickers
                    .iter()
                    .any(|v| v.id() == asset.sticker_id() && v.lifecycle().is_purged())
                    && !local.as_ref().is_some_and(|v| {
                        v.blob_status() == memedock_domain::local::BlobStatus::Ready
                    })
            })
            .count() as u64;
    }
    let mut after = Some(memedock_domain::version::LocalOrder::new(
        binding.enqueued.max(1),
    )?);
    if binding.enqueued == 0 {
        after = None;
    }
    loop {
        let changes = services.db.changes_after(after, 128).await?;
        if changes.is_empty() {
            break;
        }
        for change in changes {
            after = Some(change.local_order());
            if change.contributes_overlay() {
                result.pending += 1;
            }
        }
    }
    if result.pending > 0 {
        result.retry_after_ms = 500;
    }
    if result.error.is_some() {
        result.retry_after_ms = 30_000;
    }
    if !binding.initialized {
        result.pending += 1;
        result.retry_after_ms = 500;
    }
    services.sync_status.send_if_modified(|old| {
        if old == &result {
            false
        } else {
            *old = result.clone();
            true
        }
    });
    Ok(result)
}
impl Library {
    pub fn sync_status(&self) -> Result<Task<SyncStatus>> {
        self.submit(
            Lane::Read,
            Priority::Interactive,
            |services, _| async move { status(&services).await },
        )
    }
    pub fn connect_sync(&self, origin: String, token: String) -> Result<Task<SyncStatus>> {
        self.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, control| async move {
                let _sync = services
                    .sync_permit
                    .try_acquire()
                    .map_err(|_| CoreError::new(ErrorCode::Busy, "sync is already running"))?;
                let client = client::Client::new(&origin, token)?;
                let info = client.info(&control).await?;
                if info.protocol_version != 1 {
                    return Err(CoreError::new(
                        ErrorCode::UnsupportedSchema,
                        "unsupported sync protocol",
                    ));
                }
                let _write = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                let old = services.db.sync_binding(budget(&services)).await?;
                if let Some(mut old) = old.filter(|old| {
                    old.origin == client.origin()
                        && old.library_id.to_string() == info.library_id
                        && old.epoch.to_string() == info.sync_epoch
                        && old.device_id.to_string() == info.device_id
                }) {
                    old.paused = false;
                    old.last_error = None;
                    let tx = services.db.begin_write().await?;
                    tx.commit_sync(&old, None, budget(&services)).await?;
                    return status(&services).await;
                }
                let baseline = services
                    .db
                    .archive_data_bounded(services.config.limits.max_archive_metadata_bytes)
                    .await?;
                let mut tx = services.db.begin_write().await?;
                let cutoff = tx.sync_cutoff().await?;
                tx.settle_local_history(cutoff).await?;
                tx.reset_sync_jobs().await?;
                let binding = Binding {
                    origin: client.origin().into(),
                    library_id: info.library_id.parse()?,
                    epoch: info.sync_epoch.parse()?,
                    device_id: info.device_id.parse()?,
                    cursor: memedock_domain::version::Revision::LOCAL,
                    cutoff,
                    enqueued: cutoff,
                    initialized: false,
                    paused: false,
                    baseline,
                    canonical: Default::default(),
                    last_success_ms: None,
                    last_error: None,
                };
                control.check()?;
                tx.commit_sync(&binding, None, budget(&services)).await?;
                status(&services).await
            },
        )
    }
    pub fn set_sync_paused(&self, paused: bool) -> Result<Task<SyncStatus>> {
        self.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, _| async move {
                let _sync = services
                    .sync_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("sync service closed"))?;
                let _write = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                if let Some(mut binding) = services.db.sync_binding(budget(&services)).await? {
                    binding.paused = paused;
                    let tx = services.db.begin_write().await?;
                    tx.commit_sync(&binding, None, budget(&services)).await?;
                }
                status(&services).await
            },
        )
    }
    pub fn synchronize(&self, token: String) -> Result<Task<SyncStatus>> {
        self.submit(
            Lane::Read,
            Priority::Background,
            move |services, control| async move {
                let _sync = services
                    .sync_permit
                    .try_acquire()
                    .map_err(|_| CoreError::new(ErrorCode::Busy, "sync is already running"))?;
                let result = worker::run(&services, token, &control).await;
                if let Err(error) = &result
                    && error.code() != ErrorCode::Cancelled
                {
                    let _write = services
                        .write_permit
                        .acquire()
                        .await
                        .map_err(|_| CoreError::internal("write service closed"))?;
                    if let Some(mut binding) = services.db.sync_binding(budget(&services)).await? {
                        binding.last_error = Some(error.code().as_str().into());
                        let tx = services.db.begin_write().await?;
                        tx.commit_sync(&binding, None, budget(&services)).await?;
                    }
                }
                if result.is_err() {
                    let _ = status(&services).await;
                }
                result
            },
        )
    }
}
