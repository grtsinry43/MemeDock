use super::{SourceCommit, TelegramFormat, TelegramPack};
use crate::{
    CoreError, ErrorCode, ImportInput, ImportOptions, ImportStatus, Library, Result,
    images::telegram,
    tasks::{Priority, Task, TaskController, scheduler::Lane},
};
use memedock_domain::{identity::StickerId, source::SourceItemId};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TelegramImportOutcome {
    Pending,
    Created,
    Reused,
    RestoreRequired,
    Failed(ErrorCode),
}
#[derive(Clone, Debug)]
pub struct TelegramImportItem {
    pub id: SourceItemId,
    pub sticker: Option<StickerId>,
    pub outcome: TelegramImportOutcome,
}
#[derive(Clone, Debug)]
pub struct TelegramImportReport {
    pub items: Vec<TelegramImportItem>,
    pub stopped: bool,
}
struct State {
    stop: AtomicBool,
    current: Mutex<Option<TaskController>>,
    report: watch::Sender<TelegramImportReport>,
}
#[must_use = "retain and await the import batch"]
pub struct TelegramImportTask {
    task: Task<TelegramImportReport>,
    state: Arc<State>,
}
#[derive(Clone)]
pub struct TelegramImportController {
    state: Arc<State>,
}
impl TelegramImportController {
    pub fn cancel(&self) -> Result<()> {
        let current = self
            .state
            .current
            .lock()
            .map_err(|_| CoreError::internal("telegram import control poisoned"))?;
        self.state.stop.store(true, Ordering::Release);
        if let Some(task) = current.as_ref() {
            task.cancel();
        }
        Ok(())
    }
    pub fn snapshot(&self) -> TelegramImportReport {
        self.state.report.borrow().clone()
    }
}
impl TelegramImportTask {
    pub fn controller(&self) -> TelegramImportController {
        TelegramImportController {
            state: self.state.clone(),
        }
    }
    pub fn snapshot(&self) -> TelegramImportReport {
        self.controller().snapshot()
    }
    pub fn cancel(&self) -> Result<()> {
        self.controller().cancel()
    }
    pub async fn wait(self) -> Result<TelegramImportReport> {
        match self.task.wait().await {
            Ok(report) => Ok(report),
            Err(error) if error.code() == ErrorCode::Cancelled => {
                let mut report = self.state.report.borrow().clone();
                report.stopped = true;
                Ok(report)
            }
            Err(error) => Err(error),
        }
    }
}
impl Library {
    pub fn import_telegram(
        &self,
        pack: Arc<TelegramPack>,
        ids: Vec<SourceItemId>,
    ) -> Result<TelegramImportTask> {
        pack.validate_library(self)?;
        if ids.is_empty() || ids.len() > 1000 {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "invalid telegram selection",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect();
        for id in &ids {
            pack.remote(id)?;
        }
        let (report, _) = watch::channel(TelegramImportReport {
            items: ids
                .iter()
                .map(|id| TelegramImportItem {
                    id: id.clone(),
                    sticker: None,
                    outcome: TelegramImportOutcome::Pending,
                })
                .collect(),
            stopped: false,
        });
        let state = Arc::new(State {
            stop: AtomicBool::new(false),
            current: Mutex::new(None),
            report,
        });
        let tracked = state.clone();
        let library = self.clone();
        let task = self.submit(
            Lane::Read,
            Priority::Interactive,
            move |_, control| async move {
                for (position, id) in ids.into_iter().enumerate() {
                    if tracked.stop.load(Ordering::Acquire) || control.is_cancelled() {
                        break;
                    }
                    let task = match library.import_telegram_item(pack.clone(), id) {
                        Ok(task) => task,
                        Err(error) => {
                            tracked.report.send_modify(|r| {
                                r.items[position].outcome =
                                    TelegramImportOutcome::Failed(error.code())
                            });
                            continue;
                        }
                    };
                    {
                        let mut current = tracked
                            .current
                            .lock()
                            .map_err(|_| CoreError::internal("telegram import control poisoned"))?;
                        *current = Some(task.controller());
                        if tracked.stop.load(Ordering::Acquire) || control.is_cancelled() {
                            task.cancel();
                        }
                    }
                    let mut waiting = Box::pin(task.wait());
                    let result = tokio::select! {
                        result = &mut waiting => result,
                        _ = control.cancelled() => {
                            TelegramImportController { state: tracked.clone() }.cancel()?;
                            waiting.await
                        }
                    };
                    tracked
                        .current
                        .lock()
                        .map_err(|_| CoreError::internal("telegram import control poisoned"))?
                        .take();
                    match result {
                        Ok(outcome) => tracked.report.send_modify(|r| {
                            r.items[position].sticker = Some(outcome.sticker.id());
                            r.items[position].outcome = match outcome.status {
                                ImportStatus::Created => TelegramImportOutcome::Created,
                                ImportStatus::Reused => TelegramImportOutcome::Reused,
                                ImportStatus::RestoreRequired => {
                                    TelegramImportOutcome::RestoreRequired
                                }
                            };
                        }),
                        Err(error) if error.code() == ErrorCode::Cancelled => {
                            tracked.stop.store(true, Ordering::Release);
                            break;
                        }
                        Err(error) => tracked.report.send_modify(|r| {
                            r.items[position].outcome = TelegramImportOutcome::Failed(error.code())
                        }),
                    }
                }
                tracked.report.send_modify(|r| {
                    r.stopped = tracked.stop.load(Ordering::Acquire) || control.is_cancelled()
                });
                Ok(tracked.report.borrow().clone())
            },
        )?;
        Ok(TelegramImportTask { task, state })
    }
    fn import_telegram_item(
        &self,
        pack: Arc<TelegramPack>,
        id: SourceItemId,
    ) -> Result<Task<crate::ImportOutcome>> {
        let library_id = self.identity().library_id;
        let library = self.clone();
        self.submit(
            Lane::Import,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                if let Some(item) = services.db.source_item(&id).await? {
                    let sticker = services.db.sticker(item.sticker).await?.ok_or_else(|| {
                        CoreError::new(ErrorCode::CorruptData, "source sticker missing")
                    })?;
                    if !sticker.lifecycle().is_active() {
                        return Ok(crate::ImportOutcome {
                            sticker,
                            status: ImportStatus::RestoreRequired,
                        });
                    }
                    let worker = services.clone();
                    let cancel = control.clone();
                    let asset = services
                        .db
                        .asset(item.sticker.content_hash())
                        .await?
                        .ok_or_else(|| {
                            CoreError::new(ErrorCode::CorruptData, "source asset missing")
                        })?;
                    let verified = tokio::task::spawn_blocking(move || {
                        worker
                            .blobs
                            .verify(asset.hash(), asset.byte_size().get() as u64, || {
                                cancel.is_cancelled()
                            })
                    })
                    .await?;
                    if verified.is_ok() {
                        let _write = services
                            .write_permit
                            .acquire()
                            .await
                            .map_err(|_| CoreError::internal("write service closed"))?;
                        control.check()?;
                        let mut tx = services.db.begin_write().await?;
                        let sticker = tx.sticker(item.sticker).await?.ok_or_else(|| {
                            CoreError::new(ErrorCode::CorruptData, "source sticker missing")
                        })?;
                        if !sticker.lifecycle().is_active() {
                            return Ok(crate::ImportOutcome {
                                sticker,
                                status: ImportStatus::RestoreRequired,
                            });
                        }
                        let mut local = tx
                            .local_asset(item.sticker.content_hash())
                            .await?
                            .unwrap_or_else(|| {
                                memedock_domain::local::LocalAsset::new(item.sticker.content_hash())
                            });
                        local.verified(crate::writes::now()?);
                        control.begin_commit()?;
                        tx.save_local_asset(&local).await?;
                        tx.commit().await?;
                        return Ok(crate::ImportOutcome {
                            sticker,
                            status: ImportStatus::Reused,
                        });
                    }
                }
                let remote = pack.remote(&id)?;
                let worker = services.clone();
                let pending =
                    tokio::task::spawn_blocking(move || worker.blobs.create_staging()).await??;
                control.stage("downloading_sticker");
                if let Err(error) = pack
                    .client
                    .download(
                        &remote.file_id,
                        pending.path(),
                        services.config.limits.max_file_bytes,
                        &control,
                    )
                    .await
                {
                    tokio::task::spawn_blocking(move || pending.discard()).await??;
                    return Err(error);
                }
                let format = remote.format();
                let pending = if format == TelegramFormat::Static {
                    pending
                } else {
                    let allowance = services.image_budget.acquire().await?;
                    let worker = services.clone();
                    let cancel = control.clone();
                    control.stage("converting_sticker");
                    tokio::task::spawn_blocking(move || -> Result<_> {
                        let _allowance = allowance;
                        let output = worker.blobs.create_staging()?;
                        let result = telegram::convert(
                            format,
                            pending.path(),
                            output.path(),
                            &cancel,
                            worker.config.limits.max_file_bytes,
                        );
                        pending.discard()?;
                        if let Err(error) = result {
                            output.discard()?;
                            return Err(error);
                        }
                        Ok(output)
                    })
                    .await??
                };
                let (pending, extension) = tokio::task::spawn_blocking(move || -> Result<_> {
                    let detected = image::ImageReader::open(pending.path())
                        .and_then(|reader| reader.with_guessed_format());
                    let extension = match detected {
                        Ok(reader) => match reader.format() {
                            Some(image::ImageFormat::Png) => Ok("png"),
                            Some(image::ImageFormat::WebP) => Ok("webp"),
                            _ => Err(CoreError::new(
                                ErrorCode::UnsupportedFormat,
                                "unsupported Telegram sticker format",
                            )),
                        },
                        Err(error) => Err(error.into()),
                    };
                    match extension {
                        Ok(extension) => Ok((pending, extension)),
                        Err(error) => {
                            pending.discard()?;
                            Err(error)
                        }
                    }
                })
                .await??;
                let name = format!("{}-{}.{extension}", pack.name.as_str(), id.as_str());
                let result = crate::use_cases::import::process(
                    services,
                    control,
                    ImportInput {
                        pending,
                        library_id,
                    },
                    ImportOptions {
                        original_name: name,
                        title: remote
                            .emoji
                            .clone()
                            .filter(|v| !v.is_empty())
                            .or_else(|| Some(pack.title.clone())),
                        collection: None,
                    },
                    Some(SourceCommit {
                        pack: pack.name.clone(),
                        title: pack.title.clone(),
                        item: id,
                    }),
                )
                .await?;
                if result.status != ImportStatus::RestoreRequired
                    && let Ok(thumbnail) =
                        library.request_thumbnail(result.sticker.id(), Priority::Background)
                {
                    tokio::spawn(async move {
                        let _result = thumbnail.wait().await;
                    });
                }
                Ok(result)
            },
        )
    }
}
