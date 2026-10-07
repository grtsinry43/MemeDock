use crate::{
    CoreError, ErrorCode, Library, Result,
    tasks::{Task, TaskController},
};
use memedock_domain::{
    identity::{CollectionId, StickerId, TagId},
    version::{Generation, Revision},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::watch;

pub const MAX_BATCH_ITEMS: usize = 1000;

#[derive(Clone, Copy, Debug)]
pub struct BatchTarget {
    pub id: StickerId,
    pub generation: Generation,
    pub deleted_revision: Option<Revision>,
}

#[derive(Clone, Debug)]
pub enum BatchAction {
    Assign(CollectionId, Generation),
    ClearCollection(Option<(CollectionId, Generation)>),
    AddTags(Vec<(TagId, Generation)>),
    RemoveTags(Vec<(TagId, Generation)>),
    Star(bool),
    Delete,
    Restore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchOutcome {
    Pending,
    Applied,
    Unchanged,
    Failed(ErrorCode),
}

#[derive(Clone, Debug)]
pub struct BatchItemResult {
    pub id: StickerId,
    pub outcome: BatchOutcome,
}

#[derive(Clone, Debug)]
pub struct BatchReport {
    pub items: Vec<BatchItemResult>,
    pub stopped: bool,
}

pub(crate) struct BatchState {
    pub(crate) stop: AtomicBool,
    pub(crate) current: Mutex<Option<TaskController>>,
    pub(crate) report: watch::Sender<BatchReport>,
}

#[derive(Clone)]
pub struct BatchController {
    pub(crate) state: Arc<BatchState>,
}
impl BatchController {
    pub fn cancel(&self) -> Result<()> {
        let current = self
            .state
            .current
            .lock()
            .map_err(|_| CoreError::internal("batch control poisoned"))?;
        self.state.stop.store(true, Ordering::Release);
        if let Some(task) = current.as_ref() {
            task.cancel();
        }
        Ok(())
    }
    pub fn snapshot(&self) -> BatchReport {
        self.state.report.borrow().clone()
    }
}

#[must_use = "retain the batch and await its result"]
pub struct BatchTask {
    pub(crate) task: Task<BatchReport>,
    pub(crate) controller: BatchController,
}
impl BatchTask {
    pub fn controller(&self) -> BatchController {
        self.controller.clone()
    }
    pub fn cancel(&self) -> Result<()> {
        self.controller.cancel()
    }
    pub fn snapshot(&self) -> BatchReport {
        self.controller.snapshot()
    }
    pub async fn wait(self) -> Result<BatchReport> {
        match self.task.wait().await {
            Ok(report) => Ok(report),
            Err(error) if error.code() == ErrorCode::Cancelled => {
                let mut report = self.controller.snapshot();
                report.stopped = true;
                Ok(report)
            }
            Err(error) => Err(error),
        }
    }
}

pub(crate) async fn wait_item<T>(state: &BatchState, task: Task<T>) -> Result<T> {
    {
        let mut current = state
            .current
            .lock()
            .map_err(|_| CoreError::internal("batch control poisoned"))?;
        *current = Some(task.controller());
        if state.stop.load(Ordering::Acquire) {
            task.cancel();
        }
    }
    let result = task.wait().await;
    state
        .current
        .lock()
        .map_err(|_| CoreError::internal("batch control poisoned"))?
        .take();
    result
}

impl Library {
    pub fn batch(&self, targets: Vec<BatchTarget>, action: BatchAction) -> Result<BatchTask> {
        crate::use_cases::batch_management::start(self, targets, action)
    }
}
