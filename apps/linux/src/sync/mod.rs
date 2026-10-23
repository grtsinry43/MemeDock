#[cfg(test)]
pub(crate) mod cross_platform_tests;
mod settings;
mod state;
#[cfg(test)]
pub(crate) mod tests;

pub use settings::present;
pub use state::State;

use crate::{
    credentials::Store,
    i18n::{self, Key},
};
use gtk4::{gio, glib, prelude::*};
use memedock_core::{
    Library,
    tasks::{Task, TaskController},
};
use state::Command;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

type Listener = (glib::WeakRef<glib::Object>, Rc<dyn Fn(&State)>);

/// One GLib-local serial runner per library; only Core performs network/database work.
pub struct Coordinator {
    library: Library,
    directory: Option<PathBuf>,
    state: RefCell<State>,
    commands: RefCell<VecDeque<Command>>,
    runner: RefCell<Option<glib::JoinHandle<()>>>,
    current: RefCell<Option<TaskController>>,
    wake: RefCell<gio::Cancellable>,
    pending: Cell<bool>,
    lifecycle: RefCell<gio::Cancellable>,
    closed: Cell<bool>,
    listeners: RefCell<Vec<Listener>>,
    subscriptions: RefCell<Vec<glib::JoinHandle<()>>>,
    network: RefCell<Option<(gio::NetworkMonitor, glib::SignalHandlerId)>>,
    store: RefCell<Option<(String, Rc<Store>)>>,
}

impl Coordinator {
    pub fn new(library: Library, directory: Option<PathBuf>) -> Rc<Self> {
        let this = Rc::new(Self {
            library,
            directory,
            state: RefCell::new(State {
                busy: true,
                ..State::default()
            }),
            commands: RefCell::new(VecDeque::new()),
            runner: RefCell::new(None),
            current: RefCell::new(None),
            wake: RefCell::new(gio::Cancellable::new()),
            pending: Cell::new(false),
            lifecycle: RefCell::new(gio::Cancellable::new()),
            closed: Cell::new(false),
            listeners: RefCell::new(Vec::new()),
            subscriptions: RefCell::new(Vec::new()),
            network: RefCell::new(None),
            store: RefCell::new(None),
        });
        this.subscribe();
        this.start();
        this
    }
    pub fn snapshot(&self) -> State {
        self.state.borrow().clone()
    }
    pub fn observe(&self, owner: &impl IsA<glib::Object>, update: impl Fn(&State) + 'static) {
        update(&self.snapshot());
        self.listeners
            .borrow_mut()
            .push((owner.upcast_ref().downgrade(), Rc::new(update)));
    }
    fn update(&self, change: impl FnOnce(&mut State)) {
        change(&mut self.state.borrow_mut());
        let state = self.snapshot();
        let callbacks = {
            let mut listeners = self.listeners.borrow_mut();
            listeners.retain(|(owner, _)| owner.upgrade().is_some());
            listeners
                .iter()
                .map(|(_, callback)| Rc::clone(callback))
                .collect::<Vec<_>>()
        };
        for callback in callbacks {
            callback(&state);
        }
    }
    fn wake(&self) {
        self.pending.set(true);
        self.wake.borrow().cancel();
    }
    fn request(&self, command: Command) {
        if self.closed.get() || self.snapshot().suspended || self.snapshot().busy {
            return;
        }
        self.commands.borrow_mut().push_back(command);
        self.update(|s| s.busy = true);
        self.wake();
    }
    pub fn connect(&self, origin: String, token: String) {
        self.request(Command::Connect { origin, token });
    }
    pub fn synchronize(&self) {
        self.request(Command::Synchronize);
    }
    pub fn pause(&self, paused: bool) {
        self.request(Command::Pause(paused));
    }
    pub fn disconnect(&self) {
        self.request(Command::Disconnect);
    }
    fn subscribe(self: &Rc<Self>) {
        let monitor = gio::NetworkMonitor::default();
        let weak = Rc::downgrade(self);
        let handler = monitor.connect_network_changed(move |_, available| {
            if available && let Some(this) = weak.upgrade() {
                this.wake();
            }
        });
        self.network.replace(Some((monitor, handler)));
        if let Ok(mut events) = self.library.subscribe() {
            let weak = Rc::downgrade(self);
            self.subscriptions
                .borrow_mut()
                .push(glib::spawn_future_local(async move {
                    loop {
                        use memedock_core::events::{ChangeKind, Notification};
                        match events.next().await {
                            Ok(Notification::Changed(change))
                                if !matches!(change.kind, ChangeKind::ThumbnailChanged(_)) =>
                            {
                                if let Some(this) = weak.upgrade() {
                                    this.wake();
                                } else {
                                    break;
                                }
                            }
                            Ok(Notification::ReloadRequired { .. }) => {
                                if let Some(this) = weak.upgrade() {
                                    this.wake();
                                } else {
                                    break;
                                }
                            }
                            Ok(Notification::Closed) | Err(_) => break,
                            _ => (),
                        }
                    }
                }));
        }
        if let Ok(mut events) = self.library.subscribe_sync() {
            let weak = Rc::downgrade(self);
            self.subscriptions
                .borrow_mut()
                .push(glib::spawn_future_local(async move {
                    while let Ok(Some(status)) = events.next().await {
                        let Some(this) = weak.upgrade() else {
                            break;
                        };
                        this.update(|s| s.status = status);
                    }
                }));
        }
    }
    fn start(self: &Rc<Self>) {
        if self.closed.get() || self.runner.borrow().is_some() {
            return;
        }
        let this = Rc::clone(self);
        self.runner
            .replace(Some(glib::spawn_future_local(async move {
                this.run().await;
            })));
    }
    async fn task<T>(&self, task: memedock_core::Result<Task<T>>) -> memedock_core::Result<T> {
        let task = task?;
        self.current.replace(Some(task.controller()));
        if self.closed.get() || self.snapshot().suspended {
            task.cancel();
        }
        let result = task.wait().await;
        self.current.borrow_mut().take();
        result
    }
    async fn credential<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, ()>>,
    ) -> Result<T, Key> {
        let cancel = self.lifecycle.borrow().clone();
        gio::CancellableFuture::new(future, cancel)
            .await
            .map_err(|_| Key::SyncCredential)?
            .map_err(|_| Key::SyncCredential)
    }
    async fn store(&self, origin: &str) -> Result<Rc<Store>, Key> {
        if let Some((saved, store)) = self.store.borrow().as_ref()
            && saved == origin
        {
            return Ok(Rc::clone(store));
        }
        let library = self.library.identity().library_id.to_string();
        let store = self
            .credential(Store::sync(&library, origin, self.directory.as_deref()))
            .await?;
        let store = Rc::new(store);
        self.store.replace(Some((origin.into(), Rc::clone(&store))));
        Ok(store)
    }
    async fn restore(&self) -> Result<(), Key> {
        let status = self
            .task(self.library.sync_status())
            .await
            .map_err(|e| error(e.code()))?;
        self.update(|s| {
            s.status = status.clone();
            s.credential = false;
            s.terminal = status.error.as_deref() == Some("unauthorized");
        });
        if let Some(origin) = &status.origin {
            let store = self.store(origin).await?;
            let credential = self.credential(store.load()).await?.is_some();
            self.update(|s| {
                s.credential = credential;
                s.file_credential = store.is_file();
            });
        }
        Ok(())
    }
    async fn run(&self) {
        let restored = self.restore().await;
        self.update(|s| {
            s.loaded = true;
            s.busy = false;
            s.error = restored.err();
            s.terminal |= s.error.is_some();
        });
        while !self.closed.get() && !self.snapshot().suspended {
            let command = self.commands.borrow_mut().pop_front();
            let command =
                command.or_else(|| self.snapshot().automatic().then_some(Command::Synchronize));
            if let Some(command) = command {
                self.update(|s| {
                    s.busy = true;
                    s.error = None;
                });
                let result = self.execute(command).await;
                if let Err(key) = result {
                    self.update(|s| {
                        s.error = Some(key);
                        s.terminal |= key == Key::SyncCredential;
                    });
                }
                self.update(|s| s.busy = false);
            }
            if self.closed.get() || self.snapshot().suspended {
                break;
            }
            if !self.pending.replace(false) {
                let delay =
                    Duration::from_millis(self.snapshot().status.retry_after_ms.clamp(500, 60_000));
                let cancel = self.wake.borrow().clone();
                let _ = gio::CancellableFuture::new(glib::timeout_future(delay), cancel).await;
            }
            self.wake.replace(gio::Cancellable::new());
        }
    }
    async fn execute(&self, command: Command) -> Result<(), Key> {
        match command {
            Command::Connect { origin, token } => {
                let status = self
                    .task(
                        self.library
                            .connect_sync(origin.trim().into(), token.clone()),
                    )
                    .await
                    .map_err(|e| error(e.code()))?;
                self.update(|s| {
                    s.status = status.clone();
                    s.credential = false;
                });
                let origin = status.origin.as_deref().ok_or(Key::FailureLibrary)?;
                let saved = async {
                    let store = self.store(origin).await?;
                    self.credential(store.save(token)).await?;
                    self.update(|s| {
                        s.credential = true;
                        s.file_credential = store.is_file();
                        s.terminal = false;
                    });
                    Ok::<_, Key>(())
                }
                .await;
                if saved.is_err() {
                    self.update(|s| s.terminal = true);
                    let status = self
                        .task(self.library.set_sync_paused(true))
                        .await
                        .map_err(|e| error(e.code()))?;
                    self.update(|s| s.status = status);
                }
                saved?;
                self.wake();
            }
            Command::Synchronize => {
                if !self.snapshot().automatic() {
                    return Ok(());
                }
                let origin = self.snapshot().status.origin.ok_or(Key::FailureInput)?;
                let store = self.store(&origin).await?;
                let token = match self.credential(store.load()).await {
                    Ok(Some(token)) => token,
                    Ok(None) => {
                        self.update(|s| s.credential = false);
                        return Ok(());
                    }
                    Err(key) => {
                        self.update(|s| s.terminal = true);
                        return Err(key);
                    }
                };
                self.update(|s| s.file_credential = store.is_file());
                let result = self.task(self.library.synchronize(token)).await;
                match result {
                    Ok(status) => self.update(|s| s.status = status),
                    Err(failure) => {
                        use memedock_core::ErrorCode;
                        let terminal = matches!(
                            failure.code(),
                            ErrorCode::Unauthorized
                                | ErrorCode::InvalidInput
                                | ErrorCode::UnsupportedSchema
                                | ErrorCode::CorruptData
                        );
                        self.update(|s| s.terminal = terminal);
                        if let Ok(status) = self.task(self.library.sync_status()).await {
                            self.update(|s| s.status = status);
                        }
                        return Err(error(failure.code()));
                    }
                }
            }
            Command::Pause(paused) => {
                let status = self
                    .task(self.library.set_sync_paused(paused))
                    .await
                    .map_err(|e| error(e.code()))?;
                self.update(|s| s.status = status);
            }
            Command::Disconnect => {
                let status = self
                    .task(self.library.set_sync_paused(true))
                    .await
                    .map_err(|e| error(e.code()))?;
                self.update(|s| s.status = status.clone());
                if let Some(origin) = &status.origin {
                    let store = self.store(origin).await?;
                    self.credential(store.clear()).await?;
                }
                self.update(|s| s.credential = false);
            }
        }
        Ok(())
    }
    pub async fn suspend(&self) {
        self.update(|s| s.suspended = true);
        self.commands.borrow_mut().clear();
        self.lifecycle.borrow().cancel();
        if let Some(task) = self.current.borrow().as_ref() {
            task.cancel();
        }
        self.wake();
        let runner = self.runner.borrow_mut().take();
        if let Some(runner) = runner {
            let _ = runner.await;
        }
        self.update(|s| s.busy = false);
    }
    pub fn resume(self: &Rc<Self>) {
        if self.closed.get() || !self.snapshot().suspended {
            return;
        }
        self.lifecycle.replace(gio::Cancellable::new());
        self.update(|s| {
            s.suspended = false;
            s.busy = true;
        });
        self.start();
    }
    pub async fn shutdown(&self) {
        self.closed.set(true);
        self.suspend().await;
        for task in self.subscriptions.borrow_mut().drain(..) {
            task.abort();
        }
        if let Some((monitor, handler)) = self.network.borrow_mut().take() {
            monitor.disconnect(handler);
        }
        self.store.borrow_mut().take();
    }
}
fn error(code: memedock_core::ErrorCode) -> Key {
    if code == memedock_core::ErrorCode::Unauthorized {
        Key::SyncUnauthorized
    } else {
        i18n::key_for_core(code)
    }
}
