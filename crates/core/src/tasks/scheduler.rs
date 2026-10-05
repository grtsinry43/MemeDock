use super::{Priority, TaskControl, TaskId};
use crate::{
    CoreError, ErrorCode, Result,
    config::ResourceLimits,
    library::{LibraryState, Shared},
    runtime::Services,
};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::Arc,
};
use tokio::{
    sync::{OwnedSemaphorePermit, mpsc, oneshot},
    task::{Id, JoinSet},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lane {
    Read,
    Blocking,
    Write,
    Import,
    Thumbnail,
}
impl Lane {
    fn index(self) -> usize {
        match self {
            Self::Read => 0,
            Self::Blocking => 1,
            Self::Write => 2,
            Self::Import => 3,
            Self::Thumbnail => 4,
        }
    }
}
type WorkFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub(crate) trait Action: Send {
    fn run(self: Box<Self>, services: Arc<Services>, control: Arc<TaskControl>) -> WorkFuture;
    fn reject(self: Box<Self>, error: CoreError);
}
pub(crate) struct TypedAction<T, F> {
    pub(crate) work: F,
    pub(crate) result: oneshot::Sender<Result<T>>,
}
impl<T, F, Fut> Action for TypedAction<T, F>
where
    T: Send + 'static,
    F: FnOnce(Arc<Services>, Arc<TaskControl>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    fn run(self: Box<Self>, services: Arc<Services>, control: Arc<TaskControl>) -> WorkFuture {
        Box::pin(async move {
            let result = match control.running() {
                Err(e) => Err(e),
                Ok(()) => (self.work)(services, control.clone()).await,
            };
            let result = control.finish(result);
            // A dropped result receiver means its owner cancelled/abandoned the
            // task. Work outcome remains observable in TaskSnapshot.
            let _ = self.result.send(result);
        })
    }
    fn reject(self: Box<Self>, error: CoreError) {
        let _ = self.result.send(Err(error));
    }
}
pub(crate) struct Job {
    pub(crate) control: Arc<TaskControl>,
    pub(crate) priority: Priority,
    pub(crate) lane: Lane,
    pub(crate) action: Box<dyn Action>,
    pub(crate) permit: OwnedSemaphorePermit,
}
impl Job {
    fn reject(self, error: CoreError) {
        let _ = self.control.finish::<()>(Err(error.clone()));
        self.action.reject(error);
    }
}
pub(crate) async fn run(
    shared: Arc<Shared>,
    services: Arc<Services>,
    mut receiver: mpsc::Receiver<Job>,
    limits: &ResourceLimits,
) -> Result<()> {
    let mut pending: [VecDeque<Job>; 3] = std::array::from_fn(|_| VecDeque::new());
    let max = [
        limits.async_jobs,
        limits.file_jobs,
        1,
        limits.large_decode_jobs,
        limits.thumbnail_jobs,
    ];
    let mut active = [0_usize; 5];
    let mut jobs = JoinSet::new();
    let mut running: HashMap<Id, (TaskId, Lane, Arc<TaskControl>)> = HashMap::new();
    loop {
        for _ in 0..limits.task_capacity {
            match receiver.try_recv() {
                Ok(job) => pending[job.priority as usize].push_back(job),
                Err(_) => break,
            }
        }
        let closing = shared.gate()?.phase != LibraryState::Open;
        if closing {
            receiver.close();
        }
        for queue in &mut pending {
            let mut retained = VecDeque::new();
            while let Some(job) = queue.pop_front() {
                if closing || job.control.is_cancelled() {
                    shared.remove_task(job.control.id)?;
                    job.reject(CoreError::new(
                        ErrorCode::Cancelled,
                        "task cancelled before execution",
                    ));
                } else {
                    retained.push_back(job);
                }
            }
            *queue = retained;
        }
        if !closing {
            for queue in &mut pending {
                loop {
                    let Some(index) = queue
                        .iter()
                        .position(|job| active[job.lane.index()] < max[job.lane.index()])
                    else {
                        break;
                    };
                    let job = queue
                        .remove(index)
                        .ok_or_else(|| CoreError::internal("scheduler queue changed"))?;
                    let id = job.control.id;
                    let lane = job.lane;
                    let control = job.control.clone();
                    active[lane.index()] += 1;
                    let services = services.clone();
                    let abort = jobs.spawn(async move {
                        job.action.run(services, job.control).await;
                        drop(job.permit);
                        (id, lane)
                    });
                    running.insert(abort.id(), (id, lane, control));
                }
            }
        }
        if closing && jobs.is_empty() && receiver.is_empty() {
            break;
        }
        tokio::select! {
            result=jobs.join_next_with_id(),if !jobs.is_empty()=> {
                match result {
                    Some(Ok((tokio_id,(id,lane))))=> {running.remove(&tokio_id);active[lane.index()]-=1;shared.remove_task(id)?;}
                    Some(Err(error))=> {
                        if let Some((id,lane,control))=running.remove(&error.id()) {
                            let _=control.finish::<()>(Err(error.into()));active[lane.index()]-=1;shared.remove_task(id)?;
                        } else {return Err(CoreError::internal("untracked failed task"));}
                    }
                    None=>{},
                }
            }
            job=receiver.recv(),if !closing=> {
                match job {Some(job)=>pending[job.priority as usize].push_back(job),None=>{shared.request_close()?;}}
            }
            _=shared.wake.notified()=>{},
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Library, LibraryConfig,
        tasks::{CancelResult, TaskStatus},
    };
    use std::{error::Error, sync::Mutex, time::Duration};
    type TestResult = std::result::Result<(), Box<dyn Error>>;

    async fn library(
        capacity: usize,
    ) -> std::result::Result<(tempfile::TempDir, Library), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let mut config = LibraryConfig::new(
            directory.path().join("data"),
            directory.path().join("cache"),
            directory.path().join("share"),
        );
        config.limits.task_capacity = capacity;
        config.limits.async_jobs = 1;
        config.limits.file_jobs = 1;
        Ok((directory, Library::open(config).await?))
    }

    #[tokio::test]
    async fn admission_is_bounded_and_pending_jobs_follow_priority_then_fifo() -> TestResult {
        let (_directory, library) = library(5).await?;
        let (started, started_rx) = oneshot::channel();
        let (release, release_rx) = oneshot::channel();
        let first = library.submit(Lane::Read, Priority::Background, move |_, _| async move {
            let _ = started.send(());
            release_rx
                .await
                .map_err(|_| CoreError::internal("test release dropped"))?;
            Ok(())
        })?;
        tokio::time::timeout(Duration::from_secs(5), started_rx).await??;
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = Vec::new();
        for (number, priority) in [
            (0, Priority::Background),
            (1, Priority::Visible),
            (2, Priority::Interactive),
            (3, Priority::Interactive),
        ] {
            let order = order.clone();
            tasks.push(
                library.submit(Lane::Read, priority, move |_, _| async move {
                    order
                        .lock()
                        .map_err(|_| CoreError::internal("test order poisoned"))?
                        .push(number);
                    Ok(())
                })?,
            );
        }
        assert_eq!(
            library
                .space_statistics()
                .err()
                .ok_or("expected capacity rejection")?
                .code(),
            ErrorCode::Busy
        );
        release.send(()).map_err(|_| "release failed")?;
        tokio::time::timeout(Duration::from_secs(5), first.wait()).await??;
        for task in tasks {
            tokio::time::timeout(Duration::from_secs(5), task.wait()).await??;
        }
        assert_eq!(*order.lock().map_err(|_| "order poisoned")?, [2, 3, 1, 0]);
        library.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn queued_cancellation_never_executes_and_wait_drop_requests_cancellation() -> TestResult
    {
        let (_directory, library) = library(4).await?;
        let (started, started_rx) = oneshot::channel();
        let (release, release_rx) = oneshot::channel();
        let first = library.submit(Lane::Read, Priority::Interactive, move |_, _| async move {
            let _ = started.send(());
            release_rx
                .await
                .map_err(|_| CoreError::internal("test release dropped"))?;
            Ok(())
        })?;
        tokio::time::timeout(Duration::from_secs(5), started_rx).await??;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = calls.clone();
        let queued = library.submit(Lane::Read, Priority::Background, move |_, _| async move {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })?;
        assert_eq!(queued.cancel(), CancelResult::Requested);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), queued.wait())
                .await?
                .err()
                .ok_or("expected cancellation")?
                .code(),
            ErrorCode::Cancelled
        );
        let counter = calls.clone();
        let abandoned =
            library.submit(Lane::Read, Priority::Background, move |_, _| async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })?;
        let mut progress = abandoned.progress();
        let mut waiting = Box::pin(abandoned.wait());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), waiting.as_mut())
                .await
                .is_err()
        );
        drop(waiting);
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(snapshot) = progress.next().await {
                if snapshot.status.is_terminal() {
                    assert_eq!(snapshot.status, TaskStatus::Cancelled);
                    break;
                }
            }
        })
        .await?;
        release.send(()).map_err(|_| "release failed")?;
        first.wait().await?;
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        library.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn accepted_cancel_cannot_finish_as_success_and_panics_release_the_lane() -> TestResult {
        let (_directory, library) = library(4).await?;
        let (started, started_rx) = oneshot::channel();
        let (release, release_rx) = oneshot::channel();
        let task = library.submit(Lane::Read, Priority::Interactive, move |_, _| async move {
            let _ = started.send(());
            release_rx
                .await
                .map_err(|_| CoreError::internal("test release dropped"))?;
            Ok(42)
        })?;
        tokio::time::timeout(Duration::from_secs(5), started_rx).await??;
        assert_eq!(task.cancel(), CancelResult::Requested);
        release.send(()).map_err(|_| "release failed")?;
        assert_eq!(
            task.wait().await.err().ok_or("cancel result")?.code(),
            ErrorCode::Cancelled
        );
        let panic =
            library.submit::<(), _, _>(Lane::Read, Priority::Interactive, |_, _| async {
                panic!("intentional worker panic")
            })?;
        let mut progress = panic.progress();
        assert_eq!(
            panic.wait().await.err().ok_or("panic result")?.code(),
            ErrorCode::Internal
        );
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(snapshot) = progress.next().await {
                if snapshot.status.is_terminal() {
                    assert_eq!(snapshot.status, TaskStatus::Failed(ErrorCode::Internal));
                    break;
                }
            }
        })
        .await?;
        tokio::time::timeout(Duration::from_secs(5), library.space_statistics()?.wait()).await??;
        library.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn close_waits_for_started_blocking_work_and_preserves_committing_results() -> TestResult
    {
        for committing in [false, true] {
            let (_directory, library) = library(4).await?;
            let (started, started_rx) = oneshot::channel();
            let (release, release_rx) = std::sync::mpsc::channel();
            let task = library.submit(
                Lane::Blocking,
                Priority::Interactive,
                move |_, control| async move {
                    if committing {
                        control.begin_commit()?;
                    }
                    tokio::task::spawn_blocking(move || -> Result<u32> {
                        let _ = started.send(());
                        release_rx
                            .recv()
                            .map_err(|_| CoreError::internal("test release dropped"))?;
                        Ok(42)
                    })
                    .await?
                },
            )?;
            tokio::time::timeout(Duration::from_secs(5), started_rx).await??;
            assert_eq!(
                task.cancel(),
                if committing {
                    CancelResult::CommitInProgress
                } else {
                    CancelResult::Requested
                }
            );
            let mut closing = Box::pin(library.close());
            assert!(
                tokio::time::timeout(Duration::from_millis(10), closing.as_mut())
                    .await
                    .is_err()
            );
            assert_eq!(library.state()?, LibraryState::Closing);
            assert_eq!(
                library
                    .space_statistics()
                    .err()
                    .ok_or("closing request")?
                    .code(),
                ErrorCode::Closed
            );
            release.send(())?;
            let result = tokio::time::timeout(Duration::from_secs(5), task.wait()).await?;
            if committing {
                assert_eq!(result?, 42);
            } else {
                assert_eq!(
                    result.err().ok_or("cancel result")?.code(),
                    ErrorCode::Cancelled
                );
            }
            tokio::time::timeout(Duration::from_secs(5), closing).await??;
        }
        Ok(())
    }
}
