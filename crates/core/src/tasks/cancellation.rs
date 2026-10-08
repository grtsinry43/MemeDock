use super::{TaskId, TaskSnapshot, TaskStatus};
use crate::{CoreError, ErrorCode, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, watch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelResult {
    Requested,
    CommitInProgress,
    Finished,
    NotFound,
}

pub(crate) struct TaskControl {
    pub(crate) id: TaskId,
    pub(crate) progress: watch::Sender<TaskSnapshot>,
    cancelled: AtomicBool,
    wake: Arc<Notify>,
}
impl TaskControl {
    pub(crate) fn new(wake: Arc<Notify>) -> Arc<Self> {
        let id = TaskId::new();
        let (progress, _) = watch::channel(TaskSnapshot {
            id,
            status: TaskStatus::Queued,
            cancellation_requested: false,
            stage: "queued",
        });
        Arc::new(Self {
            id,
            progress,
            cancelled: AtomicBool::new(false),
            wake,
        })
    }
    pub(crate) fn cancel(&self) -> CancelResult {
        let mut result = CancelResult::Finished;
        self.progress.send_if_modified(|snapshot| {
            if snapshot.status.is_terminal() {
                return false;
            }
            if snapshot.status == TaskStatus::Committing {
                result = CancelResult::CommitInProgress;
                return false;
            }
            result = CancelResult::Requested;
            self.cancelled.store(true, Ordering::Release);
            if snapshot.cancellation_requested {
                return false;
            }
            snapshot.cancellation_requested = true;
            true
        });
        self.wake.notify_one();
        result
    }
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub(crate) fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(CoreError::new(ErrorCode::Cancelled, "task cancelled"))
        } else {
            Ok(())
        }
    }
    pub(crate) async fn cancelled(&self) {
        let mut progress = self.progress.subscribe();
        while !self.is_cancelled() {
            if progress.changed().await.is_err() {
                return;
            }
        }
    }
    pub(crate) fn running(&self) -> Result<()> {
        self.check()?;
        self.progress.send_if_modified(|s| {
            if s.cancellation_requested {
                return false;
            }
            s.status = TaskStatus::Running;
            s.stage = "running";
            true
        });
        self.check()
    }
    pub(crate) fn begin_commit(&self) -> Result<()> {
        let mut allowed = false;
        // Cancellation and the commit point are decided under the same short
        // watch lock. No lock is retained while SQL or foreign code runs.
        self.progress.send_if_modified(|s| {
            if s.cancellation_requested || s.status.is_terminal() {
                return false;
            }
            s.status = TaskStatus::Committing;
            s.stage = "committing";
            allowed = true;
            true
        });
        if allowed {
            Ok(())
        } else {
            Err(CoreError::new(
                ErrorCode::Cancelled,
                "task cancelled before commit",
            ))
        }
    }
    pub(crate) fn stage(&self, stage: &'static str) {
        self.progress.send_if_modified(|s| {
            if s.status.is_terminal() {
                false
            } else {
                s.stage = stage;
                true
            }
        });
    }
    pub(crate) fn finish<T>(&self, mut result: Result<T>) -> Result<T> {
        self.progress.send_if_modified(|s| {
            if result.is_ok() && s.cancellation_requested && s.status != TaskStatus::Committing {
                result = Err(CoreError::new(
                    ErrorCode::Cancelled,
                    "task cancelled before completion",
                ));
            }
            s.status = match &result {
                Ok(_) => TaskStatus::Succeeded,
                Err(e) if e.code() == ErrorCode::Cancelled => TaskStatus::Cancelled,
                Err(e) => TaskStatus::Failed(e.code()),
            };
            s.stage = "finished";
            true
        });
        result
    }
}
