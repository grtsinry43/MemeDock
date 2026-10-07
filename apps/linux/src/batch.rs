use crate::{
    detail,
    i18n::{self, Key},
};
use gtk4::{glib, prelude::*};
use libadwaita::{self as adw, prelude::*};
use memedock_core::{
    Library,
    batch::{BatchAction, BatchController, BatchOutcome, BatchReport, BatchTarget},
};
use memedock_domain::{
    identity::{CollectionId, StickerId},
    version::Generation,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

#[derive(Default)]
pub struct Selection {
    pub selecting: bool,
    pub syncing: bool,
    pub busy: bool,
    pub items: BTreeMap<StickerId, BatchTarget>,
}

#[derive(Clone)]
struct Host {
    library: Library,
    targets: Vec<BatchTarget>,
    busy: Rc<Cell<bool>>,
    current: Rc<RefCell<Option<BatchController>>>,
    dialog: glib::WeakRef<adw::Dialog>,
    column: glib::WeakRef<gtk4::Box>,
    notify: Rc<dyn Fn(&str)>,
    finished: Rc<dyn Fn(BatchReport)>,
    completed: Rc<Cell<bool>>,
}

pub fn show(
    parent: &impl IsA<gtk4::Widget>,
    library: Library,
    targets: Vec<BatchTarget>,
    trash: bool,
    expected: Option<(CollectionId, Generation)>,
    notify: Rc<dyn Fn(&str)>,
    finished: Rc<dyn Fn(BatchReport)>,
) {
    if targets.is_empty() {
        return;
    }
    let dialog = adw::Dialog::new();
    dialog.set_title(i18n::text(Key::BatchOrganize));
    dialog.set_content_width(420);
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    outer.append(&adw::HeaderBar::new());
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_min_content_height(240);
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    for side in [0, 1, 2, 3] {
        match side {
            0 => column.set_margin_start(18),
            1 => column.set_margin_end(18),
            2 => column.set_margin_top(12),
            _ => column.set_margin_bottom(18),
        }
    }
    scroll.set_child(Some(&column));
    outer.append(&scroll);
    dialog.set_child(Some(&outer));
    let host = Host {
        library,
        targets,
        busy: Rc::new(Cell::new(false)),
        current: Rc::new(RefCell::new(None)),
        dialog: dialog.downgrade(),
        column: column.downgrade(),
        notify,
        finished,
        completed: Rc::new(Cell::new(false)),
    };
    let current = Rc::clone(&host.current);
    let completed = Rc::clone(&host.completed);
    let finished = Rc::clone(&host.finished);
    let targets = host.targets.clone();
    dialog.connect_closed(move |_| {
        if let Some(control) = current.borrow().as_ref() {
            let _ = control.cancel();
        } else if !completed.replace(true) {
            finished(BatchReport {
                items: targets
                    .iter()
                    .map(|v| memedock_core::batch::BatchItemResult {
                        id: v.id,
                        outcome: BatchOutcome::Pending,
                    })
                    .collect(),
                stopped: true,
            });
        }
    });
    column.append(&gtk4::Label::new(Some(&i18n::selection_count(
        host.targets.len(),
    ))));
    if trash {
        action(&column, &host, Key::Restore, BatchAction::Restore);
    } else {
        let picker = button(Key::AssignCollection);
        let tracked = host.clone();
        picker.connect_clicked(move |_| choose(tracked.clone(), 0));
        column.append(&picker);
        for (key, mode) in [(Key::AddTags, 1), (Key::RemoveTags, 2)] {
            let picker = button(key);
            let tracked = host.clone();
            picker.connect_clicked(move |_| choose(tracked.clone(), mode));
            column.append(&picker);
        }
        action(&column, &host, Key::Favorite, BatchAction::Star(true));
        action(&column, &host, Key::Unfavorite, BatchAction::Star(false));
        action(
            &column,
            &host,
            Key::ClearCollection,
            BatchAction::ClearCollection(expected),
        );
        let delete = button(Key::Delete);
        delete.add_css_class("destructive-action");
        let tracked = host.clone();
        delete.connect_clicked(move |_| {
            let Some(parent) = tracked.column.upgrade() else {
                return;
            };
            let tracked = tracked.clone();
            detail::confirm(
                &parent,
                i18n::text(Key::Delete),
                i18n::text(Key::DeleteHint),
                i18n::text(Key::Delete),
                true,
                move || run(tracked.clone(), BatchAction::Delete),
            );
        });
        column.append(&delete);
    }
    dialog.present(Some(parent));
}

fn button(key: Key) -> gtk4::Button {
    gtk4::Button::with_label(i18n::text(key))
}
fn clear(column: &gtk4::Box) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
}
fn action(column: &gtk4::Box, host: &Host, key: Key, action: BatchAction) {
    let button = button(key);
    let host = host.clone();
    button.connect_clicked(move |_| run(host.clone(), action.clone()));
    column.append(&button);
}
fn choose(host: Host, mode: u8) {
    if host.busy.get() {
        return;
    }
    let Some(column) = host.column.upgrade() else {
        return;
    };
    clear(&column);
    column.append(&gtk4::Label::new(Some(i18n::text(Key::SharePreparing))));
    glib::spawn_future_local(async move {
        if mode == 0 {
            let result = match host.library.collections(false) {
                Ok(t) => t.wait().await,
                Err(e) => Err(e),
            };
            let Some(column) = host.column.upgrade() else {
                return;
            };
            clear(&column);
            match result {
                Ok(values) => {
                    if values.is_empty() {
                        column.append(&gtk4::Label::new(Some(i18n::text(Key::CollectionEmpty))));
                    }
                    for value in values {
                        let button = gtk4::Button::with_label(value.name().as_str());
                        let tracked = host.clone();
                        button.connect_clicked(move |_| {
                            run(
                                tracked.clone(),
                                BatchAction::Assign(value.id(), value.lifecycle().generation()),
                            )
                        });
                        column.append(&button);
                    }
                }
                Err(error) => {
                    column.append(&gtk4::Label::new(Some(i18n::core(error.code()))));
                    let retry = button(Key::Retry);
                    let h = host.clone();
                    retry.connect_clicked(move |_| choose(h.clone(), mode));
                    column.append(&retry);
                }
            }
        } else {
            let result = match host.library.tags(false) {
                Ok(t) => t.wait().await,
                Err(e) => Err(e),
            };
            let Some(column) = host.column.upgrade() else {
                return;
            };
            clear(&column);
            match result {
                Ok(values) => {
                    let choices: Vec<_> = values
                        .into_iter()
                        .map(|value| {
                            let check = gtk4::CheckButton::with_label(value.name().as_str());
                            column.append(&check);
                            (value, check)
                        })
                        .collect();
                    let accept = button(Key::Done);
                    let tracked = host.clone();
                    accept.connect_clicked(move |_| {
                        let tags: Vec<_> = choices
                            .iter()
                            .filter(|(_, check)| check.is_active())
                            .map(|(v, _)| (v.id(), v.lifecycle().generation()))
                            .collect();
                        if !tags.is_empty() {
                            run(
                                tracked.clone(),
                                if mode == 1 {
                                    BatchAction::AddTags(tags)
                                } else {
                                    BatchAction::RemoveTags(tags)
                                },
                            );
                        }
                    });
                    column.append(&accept);
                }
                Err(error) => {
                    column.append(&gtk4::Label::new(Some(i18n::core(error.code()))));
                    let retry = button(Key::Retry);
                    let h = host.clone();
                    retry.connect_clicked(move |_| choose(h.clone(), mode));
                    column.append(&retry);
                }
            }
        }
    });
}

fn run(host: Host, action: BatchAction) {
    if host.busy.replace(true) {
        return;
    }
    let batch = match host.library.batch(host.targets.clone(), action) {
        Ok(value) => value,
        Err(error) => {
            host.busy.set(false);
            (host.notify)(i18n::core(error.code()));
            return;
        }
    };
    *host.current.borrow_mut() = Some(batch.controller());
    if let Some(column) = host.column.upgrade() {
        clear(&column);
        column.append(&gtk4::Label::new(Some(i18n::text(Key::SharePreparing))));
        let spinner = adw::Spinner::new();
        column.append(&spinner);
        let cancel = button(Key::Cancel);
        let current = Rc::clone(&host.current);
        cancel.connect_clicked(move |_| {
            if let Some(control) = current.borrow().as_ref() {
                let _ = control.cancel();
            }
        });
        column.append(&cancel);
    }
    let progress = gtk4::Label::new(None);
    if let Some(column) = host.column.upgrade() {
        column.prepend(&progress);
    }
    let weak_progress = progress.downgrade();
    let controller = batch.controller();
    let polling = glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
        let Some(label) = weak_progress.upgrade() else {
            return glib::ControlFlow::Continue;
        };
        let report = controller.snapshot();
        let done = report
            .items
            .iter()
            .filter(|i| matches!(i.outcome, BatchOutcome::Applied | BatchOutcome::Unchanged))
            .count();
        let failed = report
            .items
            .iter()
            .filter(|i| matches!(i.outcome, BatchOutcome::Failed(_)))
            .count();
        label.set_text(&i18n::batch_result(
            done,
            failed,
            report.items.len() - done - failed,
        ));
        glib::ControlFlow::Continue
    });
    glib::spawn_future_local(async move {
        let result = batch.wait().await;
        polling.remove();
        host.current.borrow_mut().take();
        host.busy.set(false);
        match result {
            Ok(report) => {
                let done = report
                    .items
                    .iter()
                    .filter(|v| {
                        matches!(v.outcome, BatchOutcome::Applied | BatchOutcome::Unchanged)
                    })
                    .count();
                let failed = report
                    .items
                    .iter()
                    .filter(|v| matches!(v.outcome, BatchOutcome::Failed(_)))
                    .count();
                (host.notify)(&i18n::batch_result(
                    done,
                    failed,
                    report.items.len() - done - failed,
                ));
                host.completed.set(true);
                (host.finished)(report);
                if let Some(dialog) = host.dialog.upgrade() {
                    dialog.force_close();
                }
            }
            Err(error) => {
                (host.notify)(i18n::core(error.code()));
                if let Some(dialog) = host.dialog.upgrade() {
                    dialog.force_close();
                }
            }
        }
    });
}
