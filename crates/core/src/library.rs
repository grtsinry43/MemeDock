use crate::{
    config::LibraryConfig,
    error::{CoreError, ErrorCode, Result},
    events::{EventHub, Subscription},
    runtime::Services,
    tasks::scheduler::{Job, Lane, TypedAction},
    tasks::{CancelResult, Priority, Task, TaskControl, TaskId},
};
use memedock_storage::db::LibraryIdentity;
use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::{Notify, Semaphore, mpsc, oneshot, watch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryState {
    Open,
    Closing,
    Closed,
}
pub(crate) struct Gate {
    pub(crate) phase: LibraryState,
    pub(crate) active: HashMap<TaskId, Arc<TaskControl>>,
}
pub(crate) struct Shared {
    pub(crate) gate: Mutex<Gate>,
    pub(crate) wake: Arc<Notify>,
    pub(crate) events: Arc<EventHub>,
    pub(crate) closed: watch::Receiver<Option<Result<()>>>,
    sender: mpsc::Sender<Job>,
    admission: Arc<Semaphore>,
}
impl Shared {
    pub(crate) fn new(
        capacity: usize,
        event_capacity: usize,
        closed: watch::Receiver<Option<Result<()>>>,
    ) -> (Arc<Self>, mpsc::Receiver<Job>) {
        let (sender, receiver) = mpsc::channel(capacity);
        (
            Arc::new(Self {
                gate: Mutex::new(Gate {
                    phase: LibraryState::Open,
                    active: HashMap::new(),
                }),
                wake: Arc::new(Notify::new()),
                events: EventHub::new(event_capacity),
                closed,
                sender,
                admission: Arc::new(Semaphore::new(capacity)),
            }),
            receiver,
        )
    }
    pub(crate) fn gate(&self) -> Result<MutexGuard<'_, Gate>> {
        self.gate
            .lock()
            .map_err(|_| CoreError::internal("library state poisoned"))
    }
    pub(crate) fn request_close(&self) -> Result<()> {
        let mut gate = self.gate()?;
        if gate.phase == LibraryState::Open {
            gate.phase = LibraryState::Closing;
            for control in gate.active.values() {
                control.cancel();
            }
        }
        drop(gate);
        self.wake.notify_one();
        Ok(())
    }
    pub(crate) fn remove_task(&self, id: TaskId) -> Result<()> {
        self.gate()?.active.remove(&id);
        Ok(())
    }
}
struct Client {
    shared: Arc<Shared>,
    identity: LibraryIdentity,
    data_dir: PathBuf,
}
impl Drop for Client {
    fn drop(&mut self) {
        // Last library handle initiates cooperative shutdown without blocking its
        // caller. Explicit close is required to receive shutdown errors.
        if self.shared.request_close().is_err() {
            self.shared.wake.notify_one();
        }
    }
}
#[derive(Clone)]
pub struct Library {
    client: Arc<Client>,
}
impl Library {
    pub async fn open(config: LibraryConfig) -> Result<Self> {
        crate::runtime::open(config).await
    }
    pub(crate) fn from_shared(
        shared: Arc<Shared>,
        identity: LibraryIdentity,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            client: Arc::new(Client {
                shared,
                identity,
                data_dir,
            }),
        }
    }
    pub fn identity(&self) -> LibraryIdentity {
        self.client.identity
    }
    pub fn data_dir(&self) -> &std::path::Path {
        &self.client.data_dir
    }
    pub fn state(&self) -> Result<LibraryState> {
        Ok(self.client.shared.gate()?.phase)
    }
    pub fn active_tasks(&self) -> Result<usize> {
        Ok(self.client.shared.gate()?.active.len())
    }
    pub fn subscribe(&self) -> Result<Subscription> {
        let gate = self.client.shared.gate()?;
        if gate.phase != LibraryState::Open {
            return Err(CoreError::new(ErrorCode::Closed, "library is closing"));
        }
        self.client.shared.events.subscribe()
    }
    pub fn cancel_task(&self, id: TaskId) -> Result<CancelResult> {
        Ok(self
            .client
            .shared
            .gate()?
            .active
            .get(&id)
            .map_or(CancelResult::NotFound, |c| c.cancel()))
    }
    pub async fn close(&self) -> Result<()> {
        self.client.shared.request_close()?;
        let mut receiver = self.client.shared.closed.clone();
        loop {
            let result = receiver.borrow_and_update().clone();
            if let Some(result) = result {
                return result;
            }
            receiver.changed().await.map_err(|e| {
                CoreError::caused(ErrorCode::Internal, "runtime owner disconnected", e)
            })?;
        }
    }
    pub(crate) fn submit<T, F, Fut>(
        &self,
        lane: Lane,
        priority: Priority,
        work: F,
    ) -> Result<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce(Arc<Services>, Arc<TaskControl>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let shared = &self.client.shared;
        let mut gate = shared.gate()?;
        if gate.phase != LibraryState::Open {
            return Err(CoreError::new(ErrorCode::Closed, "library is closing"));
        }
        let permit = shared
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| CoreError::new(ErrorCode::Busy, "task capacity reached"))?;
        let control = TaskControl::new(shared.wake.clone());
        let (sender, receiver) = oneshot::channel();
        let job = Job {
            control: control.clone(),
            priority,
            lane,
            action: Box::new(TypedAction {
                work,
                result: sender,
            }),
            permit,
        };
        shared.sender.try_send(job).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => {
                CoreError::new(ErrorCode::Busy, "task queue full")
            }
            mpsc::error::TrySendError::Closed(_) => {
                CoreError::new(ErrorCode::Closed, "task scheduler stopped")
            }
        })?;
        gate.active.insert(control.id, control.clone());
        Ok(Task {
            control,
            result: Some(receiver),
        })
    }
}
