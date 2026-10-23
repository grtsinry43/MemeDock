mod groups;
#[cfg(test)]
pub(crate) mod tests;

use crate::i18n::{self, Key};
use gtk4::{glib, prelude::*};
use libadwaita::{self as adw, prelude::*};
use memedock_core::{
    Library, TrashTarget,
    tasks::{Task, TaskController},
};
use memedock_domain::{
    lifecycle::EntityId,
    version::{Generation, Revision},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub type Notify = Rc<dyn Fn(&str)>;
pub type Finished = Rc<dyn Fn(Report)>;
#[derive(Clone)]
pub struct Report {
    pub targets: Vec<TrashTarget>,
    pub succeeded: Vec<EntityId>,
    pub failure: Option<memedock_core::ErrorCode>,
    pub stopped: bool,
}

pub struct Coordinator {
    library: Library,
    suspended: Cell<bool>,
    closed: Cell<bool>,
    busy: Cell<bool>,
    epoch: Cell<u64>,
    current: RefCell<Option<TaskController>>,
    operation: RefCell<Option<glib::JoinHandle<()>>>,
    confirmation: RefCell<glib::WeakRef<adw::AlertDialog>>,
    groups: RefCell<glib::WeakRef<adw::Dialog>>,
}
impl Coordinator {
    pub fn new(library: Library) -> Rc<Self> {
        Rc::new(Self {
            library,
            suspended: Cell::new(false),
            closed: Cell::new(false),
            busy: Cell::new(false),
            epoch: Cell::new(0),
            current: RefCell::new(None),
            operation: RefCell::new(None),
            confirmation: RefCell::new(glib::WeakRef::new()),
            groups: RefCell::new(glib::WeakRef::new()),
        })
    }
    pub fn busy(&self) -> bool {
        self.busy.get() || self.confirmation.borrow().upgrade().is_some()
    }
    fn available(&self) -> bool {
        !self.suspended.get() && !self.closed.get() && !self.busy()
    }
    async fn task<T>(&self, task: memedock_core::Result<Task<T>>) -> memedock_core::Result<T> {
        let task = task?;
        self.current.replace(Some(task.controller()));
        if self.suspended.get() || self.closed.get() {
            task.cancel();
        }
        let result = task.wait().await;
        self.current.borrow_mut().take();
        result
    }
    pub fn request(
        self: &Rc<Self>,
        parent: &impl IsA<gtk4::Widget>,
        targets: Vec<TrashTarget>,
        notify: Notify,
        finished: Finished,
    ) {
        if !self.available() {
            finished(Report {
                targets,
                succeeded: Vec::new(),
                failure: Some(memedock_core::ErrorCode::Busy),
                stopped: true,
            });
            return;
        }
        if targets.is_empty() {
            notify(i18n::text(Key::TrashEmpty));
            return;
        }
        let body = format!(
            "{}\n\n{}",
            i18n::selection_count(targets.len()),
            i18n::text(Key::PurgeHint)
        );
        let dialog = adw::AlertDialog::new(Some(i18n::text(Key::Purge)), Some(&body));
        dialog.add_response("cancel", i18n::text(Key::Cancel));
        dialog.add_response("purge", i18n::text(Key::Purge));
        dialog.set_response_appearance("purge", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        self.confirmation.borrow_mut().set(Some(&dialog));
        let this = Rc::clone(self);
        let epoch = self.epoch.get();
        dialog.connect_response(None, move |_, response| {
            this.confirmation.borrow_mut().set(None);
            if response != "purge" || epoch != this.epoch.get() || !this.available() {
                finished(Report {
                    targets: targets.clone(),
                    succeeded: Vec::new(),
                    failure: None,
                    stopped: true,
                });
                return;
            }
            this.execute(targets.clone(), Rc::clone(&notify), Rc::clone(&finished));
        });
        dialog.present(Some(parent));
    }
    fn execute(self: &Rc<Self>, targets: Vec<TrashTarget>, notify: Notify, finished: Finished) {
        self.busy.set(true);
        let this = Rc::clone(self);
        let epoch = self.epoch.get();
        self.operation
            .replace(Some(glib::spawn_future_local(async move {
                let mut report = Report {
                    targets,
                    succeeded: Vec::new(),
                    failure: None,
                    stopped: false,
                };
                for target in &report.targets {
                    if this.suspended.get() || this.closed.get() || this.epoch.get() != epoch {
                        report.stopped = true;
                        break;
                    }
                    match this
                        .task(this.library.purge_entity(
                            target.entity,
                            target.generation,
                            target.deleted_revision,
                        ))
                        .await
                    {
                        Ok(()) => report.succeeded.push(target.entity),
                        Err(error) => report.failure = Some(error.code()),
                    }
                }
                this.busy.set(false);
                if !this.closed.get() && this.epoch.get() == epoch {
                    let mut message = i18n::purge_result(
                        report.succeeded.len(),
                        report.targets.len() - report.succeeded.len(),
                    );
                    if let Some(code) = report.failure {
                        message.push('\n');
                        message.push_str(i18n::core(code));
                    }
                    notify(&message);
                }
                finished(report);
            })));
    }
    pub fn empty(self: &Rc<Self>, parent: &impl IsA<gtk4::Widget>, notify: Notify) {
        if !self.available() {
            return;
        }
        self.busy.set(true);
        let this = Rc::clone(self);
        let parent = parent.upcast_ref::<gtk4::Widget>().downgrade();
        let epoch = self.epoch.get();
        self.operation
            .replace(Some(glib::spawn_future_local(async move {
                let result = this.task(this.library.trash_snapshot()).await;
                this.busy.set(false);
                if this.epoch.get() != epoch || this.closed.get() {
                    return;
                }
                match result {
                    Ok(targets) => {
                        if let Some(parent) = parent.upgrade() {
                            this.request(&parent, targets, notify, Rc::new(|_| {}));
                        }
                    }
                    Err(error) => notify(i18n::core(error.code())),
                }
            })));
    }
    pub fn cleanup(self: &Rc<Self>, notify: Notify) {
        if !self.available() {
            return;
        }
        self.busy.set(true);
        let this = Rc::clone(self);
        let epoch = self.epoch.get();
        self.operation
            .replace(Some(glib::spawn_future_local(async move {
                let result = this.task(this.library.retry_purge_cleanup()).await;
                this.busy.set(false);
                if this.epoch.get() == epoch && !this.closed.get() {
                    notify(match result {
                        Ok(()) => i18n::text(Key::PurgeCleanupDone),
                        Err(error) => i18n::core(error.code()),
                    });
                }
            })));
    }
    pub fn groups(self: &Rc<Self>, parent: &impl IsA<gtk4::Widget>, notify: Notify) {
        if self.suspended.get() || self.closed.get() {
            return;
        }
        if let Some(dialog) = self.groups.borrow().upgrade() {
            dialog.present(Some(parent));
            return;
        }
        let dialog = groups::present(parent, Rc::clone(self), notify);
        self.groups.borrow_mut().set(Some(&dialog));
    }
    fn restore(self: &Rc<Self>, target: TrashTarget, notify: Notify) {
        if !self.available() {
            return;
        }
        self.busy.set(true);
        let this = Rc::clone(self);
        let epoch = self.epoch.get();
        self.operation
            .replace(Some(glib::spawn_future_local(async move {
                let result = match target.entity {
                    EntityId::Collection(id) => this
                        .task(this.library.restore_collection(
                            id,
                            target.generation,
                            target.deleted_revision,
                        ))
                        .await
                        .map(|_| ()),
                    EntityId::Tag(id) => this
                        .task(this.library.restore_tag(
                            id,
                            target.generation,
                            target.deleted_revision,
                        ))
                        .await
                        .map(|_| ()),
                    EntityId::Sticker(id) => this
                        .task(this.library.restore_sticker(
                            id,
                            target.generation,
                            target.deleted_revision,
                        ))
                        .await
                        .map(|_| ()),
                };
                this.busy.set(false);
                if this.epoch.get() == epoch && !this.closed.get() {
                    notify(match result {
                        Ok(()) => i18n::text(Key::Restored),
                        Err(error) => i18n::core(error.code()),
                    });
                }
            })));
    }
    pub async fn suspend(&self) {
        self.suspended.set(true);
        self.epoch.set(self.epoch.get().wrapping_add(1));
        let dialog = self.confirmation.borrow().upgrade();
        if let Some(dialog) = dialog {
            dialog.force_close();
        }
        let dialog = self.groups.borrow().upgrade();
        if let Some(dialog) = dialog {
            dialog.force_close();
        }
        if let Some(task) = self.current.borrow().as_ref() {
            task.cancel();
        }
        let operation = self.operation.borrow_mut().take();
        if let Some(operation) = operation {
            let _ = operation.await;
        }
        self.busy.set(false);
    }
    pub fn resume(&self) {
        if !self.closed.get() {
            self.suspended.set(false);
        }
    }
    pub async fn shutdown(&self) {
        self.closed.set(true);
        self.suspend().await;
    }
}
pub fn sticker(
    id: memedock_domain::identity::StickerId,
    generation: Generation,
    revision: Revision,
) -> TrashTarget {
    TrashTarget {
        entity: EntityId::Sticker(id),
        generation,
        deleted_revision: revision,
    }
}
