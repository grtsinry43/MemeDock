mod cancellation;
pub(crate) mod scheduler;
mod state;
use crate::{CoreError, Result};
pub use cancellation::CancelResult;
pub(crate) use cancellation::TaskControl;
pub use state::{Priority, TaskId, TaskSnapshot, TaskStatus};
use std::sync::Arc;
use tokio::sync::{oneshot, watch};

#[must_use = "retain the task and await its result, or cancel it explicitly"]
pub struct Task<T> {
    pub(crate) control: Arc<TaskControl>,
    pub(crate) result: Option<oneshot::Receiver<Result<T>>>,
}
impl<T> Task<T> {
    pub fn id(&self) -> TaskId {
        self.control.id
    }
    pub fn snapshot(&self) -> TaskSnapshot {
        self.control.progress.borrow().clone()
    }
    pub fn cancel(&self) -> CancelResult {
        self.control.cancel()
    }
    pub fn progress(&self) -> TaskProgress {
        TaskProgress {
            receiver: self.control.progress.subscribe(),
            finished: false,
        }
    }
    pub async fn wait(mut self) -> Result<T> {
        let receiver = self
            .result
            .take()
            .ok_or_else(|| CoreError::internal("task result already consumed"))?;
        receiver.await.map_err(|e| {
            CoreError::caused(crate::ErrorCode::Internal, "task worker disconnected", e)
        })?
    }
}
impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        self.control.cancel();
    }
}
pub struct TaskProgress {
    receiver: watch::Receiver<TaskSnapshot>,
    finished: bool,
}
impl TaskProgress {
    pub fn snapshot(&self) -> TaskSnapshot {
        self.receiver.borrow().clone()
    }
    pub async fn next(&mut self) -> Option<TaskSnapshot> {
        if self.finished {
            return None;
        }
        if self.receiver.borrow().status.is_terminal() {
            self.finished = true;
            return Some(self.receiver.borrow().clone());
        }
        if self.receiver.changed().await.is_err() {
            self.finished = true;
            return None;
        }
        let value = self.receiver.borrow_and_update().clone();
        self.finished = value.status.is_terminal();
        Some(value)
    }
}
