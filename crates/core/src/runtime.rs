use crate::{
    CoreError, ErrorCode, Library, LibraryConfig, Result,
    events::EventHub,
    library::{LibraryState, Shared},
    registry::Reservation,
    tasks::scheduler,
};
use memedock_storage::{
    LibraryDatabase,
    files::{DerivedStore, FsBlobStore},
};
use std::sync::Arc;
use tokio::sync::{oneshot, watch};

pub(crate) struct Services {
    pub(crate) db: LibraryDatabase,
    pub(crate) blobs: FsBlobStore,
    pub(crate) config: LibraryConfig,
    pub(crate) events: Arc<EventHub>,
    pub(crate) derived: DerivedStore,
    pub(crate) image_budget: crate::images::budget::ImageBudget,
    pub(crate) write_permit: tokio::sync::Semaphore,
    pub(crate) thumbnail_locks: std::sync::Mutex<
        std::collections::HashMap<
            memedock_domain::identity::StickerId,
            std::sync::Weak<tokio::sync::Semaphore>,
        >,
    >,
}
pub(crate) async fn open(config: LibraryConfig) -> Result<Library> {
    config.validate()?;
    let (ready_tx, ready_rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("memedock-owner".into())
        .spawn(move || owner(config, ready_tx))?;
    ready_rx.await.map_err(|e| {
        CoreError::caused(
            ErrorCode::Internal,
            "library initialization worker disconnected",
            e,
        )
    })?
}
fn owner(config: LibraryConfig, ready: oneshot::Sender<Result<Library>>) {
    let resolved = config
        .resolve()
        .and_then(|config| Reservation::acquire(&config).map(|reservation| (config, reservation)));
    let (config, reservation) = match resolved {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.limits.runtime_threads)
        .max_blocking_threads(config.limits.blocking_threads)
        .thread_name("memedock-worker")
        .enable_all()
        .build();
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            drop(reservation);
            let _ = ready.send(Err(error.into()));
            return;
        }
    };
    let (closed_tx, closed_rx) = watch::channel(None);
    let (shared, receiver) = Shared::new(
        config.limits.task_capacity,
        config.limits.event_capacity,
        closed_rx,
    );
    let mut ready = Some(ready);
    let result = runtime.block_on(async {
        let blobs = match FsBlobStore::open(&config.data_dir) {
            Ok(blobs) => blobs,
            Err(error) => {
                let e = CoreError::from(error);
                return Err(e);
            }
        };
        let db = match LibraryDatabase::open(config.data_dir.join("library.sqlite")).await {
            Ok(db) => db,
            Err(error) => {
                let e = CoreError::from(error);
                return Err(e);
            }
        };
        let library = Library::from_shared(shared.clone(), db.identity(), config.data_dir.clone());
        let services = Arc::new(Services {
            derived: DerivedStore::open(&config.cache_dir)?,
            image_budget: crate::images::budget::ImageBudget::new(&config.limits)?,
            write_permit: tokio::sync::Semaphore::new(1),
            thumbnail_locks: std::sync::Mutex::new(Default::default()),
            db,
            blobs,
            config,
            events: shared.events.clone(),
        });
        crate::use_cases::recovery::recover(&services).await?;
        // If open's future was abandoned, dropping the unsent handle requests
        // shutdown. Initialization still releases the reservation and DB safely.
        if let Some(ready) = ready.take() {
            let _ = ready.send(Ok(library));
        }
        let scheduled = scheduler::run(
            shared.clone(),
            services.clone(),
            receiver,
            &services.config.limits,
        )
        .await;
        let notifications = shared.events.close();
        let services = Arc::try_unwrap(services)
            .map_err(|_| CoreError::internal("tasks retained library services during shutdown"))?;
        let database = services.db.close().await.map_err(CoreError::from);
        scheduled.and(notifications).and(database)
    });
    // Tokio's blocking shutdown runs here, on its dedicated owner thread. Never
    // use shutdown_timeout to claim that blocking work has actually stopped.
    drop(runtime);
    drop(reservation);
    let result = match shared.gate() {
        Ok(mut gate) => {
            gate.phase = LibraryState::Closed;
            result
        }
        Err(e) => Err(e),
    };
    if let Some(ready) = ready {
        let _ = ready
            .send(Err(result.clone().err().unwrap_or_else(|| {
                CoreError::internal("library never opened")
            })));
    }
    closed_tx.send_replace(Some(result));
}
