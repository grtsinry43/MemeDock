mod files;
mod state;
use crate::{
    i18n::{self, Key},
    settings::Host,
};
use gtk4::{gio, glib, prelude::*};
use libadwaita::{self as adw, prelude::*};
use memedock_core::{Library, PreparedArchive, RestoreMode, tasks::TaskController};
use state::Phase;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Model {
    file_cancel: gio::Cancellable,
    phase: Phase,
    preview: Option<Arc<PreparedArchive>>,
    controller: Option<TaskController>,
    stop: Arc<AtomicBool>,
}
struct View {
    dialog: adw::Dialog,
    label: gtk4::Label,
    create: gtk4::Button,
    open: gtk4::Button,
    merge: gtk4::Button,
    replace: gtk4::Button,
    cancel: gtk4::Button,
}
impl View {
    fn phase(&self, phase: Phase) {
        self.dialog.set_can_close(!phase.busy());
        self.create.set_sensitive(phase == Phase::Idle);
        self.open.set_sensitive(phase == Phase::Idle);
        self.merge.set_visible(phase == Phase::Preview);
        self.replace.set_visible(phase == Phase::Preview);
        self.cancel.set_visible(phase != Phase::Idle);
        if phase.busy() {
            self.label.set_text(i18n::text(Key::Working));
        }
    }
}
pub fn present(host: Host) {
    if (host.busy)() || host.maintenance.get() {
        return;
    }
    let dialog = adw::Dialog::new();
    i18n::bind(&dialog, "title", Key::Backup);
    dialog.set_content_width(520);
    dialog.set_content_height(420);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    for side in ["margin-top", "margin-bottom", "margin-start", "margin-end"] {
        content.set_property(side, 20i32);
    }
    let label = i18n::label(Key::BackupHint);
    label.set_wrap(true);
    label.set_vexpand(true);
    content.append(&label);
    let create = i18n::button(Key::CreateBackup);
    let open = i18n::button(Key::RestoreBackup);
    let merge = i18n::button(Key::Merge);
    let replace = i18n::button(Key::Replace);
    let cancel = i18n::button(Key::Cancel);
    merge.add_css_class("suggested-action");
    replace.add_css_class("destructive-action");
    for b in [&create, &open, &merge, &replace, &cancel] {
        content.append(b);
    }
    toolbar.set_content(Some(&content));
    dialog.set_child(Some(&toolbar));
    let view = Rc::new(View {
        dialog: dialog.clone(),
        label,
        create,
        open,
        merge,
        replace,
        cancel,
    });
    let model = Rc::new(RefCell::new(Model {
        file_cancel: gio::Cancellable::new(),
        phase: Phase::Idle,
        preview: None,
        controller: None,
        stop: Arc::new(AtomicBool::new(false)),
    }));
    view.phase(Phase::Idle);
    let state = Rc::clone(&model);
    dialog.connect_close_attempt(move |_| cancel_work(&state));
    let state = Rc::clone(&model);
    let library = host.library.clone();
    dialog.connect_closed(move |_| {
        cancel_work(&state);
        let preview = state.borrow_mut().preview.take();
        if let Some(preview) = preview {
            let library = library.clone();
            glib::spawn_future_local(async move {
                if let Ok(task) = library.discard_prepared_archive(preview) {
                    let _ = task.wait().await;
                }
            });
        }
    });
    let weak_view = Rc::downgrade(&view);
    let state = Rc::clone(&model);
    let source = host.clone();
    view.create.connect_clicked(move |_| {
        let Some(view) = weak_view.upgrade() else {
            return;
        };
        if !begin(&source, &state, &view, Phase::Creating) {
            return;
        }
        let state = Rc::clone(&state);
        let source = source.clone();
        glib::spawn_future_local(async move {
            let result = create_backup(&source, &state, &view).await;
            finish(&source, &state, &view, result, Key::BackupSaved);
        });
    });
    let weak_view = Rc::downgrade(&view);
    let state = Rc::clone(&model);
    let source = host.clone();
    view.open.connect_clicked(move |_| {
        let Some(view) = weak_view.upgrade() else {
            return;
        };
        if !begin(&source, &state, &view, Phase::Reading) {
            return;
        }
        let state = Rc::clone(&state);
        let source = source.clone();
        glib::spawn_future_local(async move {
            match inspect(&source, &state, &view).await {
                Ok(Some(preview)) => {
                    let summary = preview.summary();
                    view.label.set_text(&summary_text(summary));
                    let mut state = state.borrow_mut();
                    state.preview = Some(preview);
                    state.phase = Phase::Preview;
                    state.controller = None;
                    source.maintenance.set(false);
                    view.phase(Phase::Preview);
                }
                Ok(None) => finish(&source, &state, &view, Ok(false), Key::BackupRestored),
                Err(message) => finish(&source, &state, &view, Err(message), Key::BackupRestored),
            }
        });
    });
    for (button, mode) in [
        (&view.merge, RestoreMode::Merge),
        (&view.replace, RestoreMode::Replace),
    ] {
        let weak_view = Rc::downgrade(&view);
        let state = Rc::clone(&model);
        let source = host.clone();
        button.connect_clicked(move |_| {
            let Some(view) = weak_view.upgrade() else {
                return;
            };
            if mode == RestoreMode::Replace {
                let view = Rc::clone(&view);
                let state = Rc::clone(&state);
                let source = source.clone();
                crate::detail::confirm(
                    &view.dialog.clone(),
                    i18n::text(Key::Replace),
                    i18n::text(Key::ReplaceHint),
                    i18n::text(Key::Replace),
                    true,
                    move || restore(source.clone(), Rc::clone(&state), Rc::clone(&view), mode),
                );
            } else {
                restore(source.clone(), Rc::clone(&state), view, mode);
            }
        });
    }
    let weak_view = Rc::downgrade(&view);
    let state = Rc::clone(&model);
    let library = host.library.clone();
    view.cancel.connect_clicked(move |_| {
        cancel_work(&state);
        if state.borrow().phase == Phase::Preview {
            let preview = state.borrow_mut().preview.take();
            let library = library.clone();
            let weak_view = weak_view.clone();
            let state = Rc::clone(&state);
            glib::spawn_future_local(async move {
                if let Some(preview) = preview
                    && let Ok(task) = library.discard_prepared_archive(preview)
                {
                    let _ = task.wait().await;
                }
                state.borrow_mut().phase = Phase::Idle;
                if let Some(view) = weak_view.upgrade() {
                    view.phase(Phase::Idle);
                    i18n::bind(&view.label, "label", Key::BackupHint);
                }
            });
        }
    });
    let weak = Rc::downgrade(&view);
    let state = Rc::clone(&model);
    i18n::on_language(&dialog, move || {
        if let Some(v) = weak.upgrade() {
            let state = state.borrow();
            if let Some(p) = &state.preview {
                v.label.set_text(&summary_text(p.summary()));
            } else if state.phase.busy() {
                v.label.set_text(i18n::text(Key::Working));
            }
        }
    });
    // The dialog owns the view until it closes; callbacks only keep weak references.
    let keep = Rc::new(RefCell::new(Some(view)));
    dialog.connect_closed(move |_| {
        keep.borrow_mut().take();
    });
    dialog.present(Some(&host.parent));
}
fn begin(host: &Host, model: &Rc<RefCell<Model>>, view: &View, phase: Phase) -> bool {
    if model.borrow().phase.busy() || host.maintenance.get() || (host.busy)() {
        return false;
    }
    model.borrow_mut().phase = phase;
    model.borrow().stop.store(false, Ordering::Relaxed);
    model.borrow_mut().file_cancel = gio::Cancellable::new();
    host.maintenance.set(true);
    view.phase(phase);
    true
}
fn cancel_work(model: &Rc<RefCell<Model>>) {
    let model = model.borrow();
    model.stop.store(true, Ordering::Relaxed);
    model.file_cancel.cancel();
    if let Some(c) = &model.controller {
        c.cancel();
    }
}
fn finish(
    host: &Host,
    model: &Rc<RefCell<Model>>,
    view: &View,
    result: Result<bool, String>,
    success: Key,
) {
    model.borrow_mut().phase = Phase::Idle;
    model.borrow_mut().controller = None;
    host.maintenance.set(false);
    view.phase(Phase::Idle);
    match result {
        Ok(true) => i18n::bind(&view.label, "label", success),
        Ok(false) => i18n::bind(&view.label, "label", Key::BackupHint),
        Err(message) => view.label.set_text(&message),
    }
}
async fn create_backup(
    host: &Host,
    model: &Rc<RefCell<Model>>,
    view: &View,
) -> Result<bool, String> {
    let task = host.library.create_backup().map_err(error)?;
    model.borrow_mut().controller = Some(task.controller());
    let backup = task.wait().await.map_err(error)?;
    let result = async {
        if model.borrow().stop.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let chooser = gtk4::FileDialog::new();
        chooser.set_initial_name(Some(&backup.file_name));
        i18n::bind(&chooser, "title", Key::CreateBackup);
        let destination = match chooser
            .save_future(view.dialog.root().and_downcast::<gtk4::Window>().as_ref())
            .await
        {
            Ok(f) => f,
            Err(e)
                if e.matches(gtk4::DialogError::Dismissed)
                    || e.matches(gtk4::DialogError::Cancelled) =>
            {
                return Ok(false);
            }
            Err(_) => return Err(i18n::text(Key::FailureIo).into()),
        };
        let source = files::local(&backup.path);
        let limit = backup.byte_size;
        let stop = Arc::clone(&model.borrow().stop);
        let cancellable = model.borrow().file_cancel.clone();
        gtk4::gio::spawn_blocking(move || {
            files::copy(&source, &destination, limit, &stop, &cancellable)
        })
        .await
        .map_err(|_| i18n::text(Key::FailureUnknown).to_owned())?
        .map_err(|_| i18n::text(Key::FailureIo).to_owned())?;
        Ok(true)
    }
    .await;
    host.library
        .discard_backup(backup)
        .map_err(error)?
        .wait()
        .await
        .map_err(error)?;
    result
}
async fn inspect(
    host: &Host,
    model: &Rc<RefCell<Model>>,
    view: &View,
) -> Result<Option<Arc<PreparedArchive>>, String> {
    let chooser = gtk4::FileDialog::new();
    i18n::bind(&chooser, "title", Key::RestoreBackup);
    let file = match chooser
        .open_future(view.dialog.root().and_downcast::<gtk4::Window>().as_ref())
        .await
    {
        Ok(file) => file,
        Err(e)
            if e.matches(gtk4::DialogError::Dismissed)
                || e.matches(gtk4::DialogError::Cancelled) =>
        {
            return Ok(None);
        }
        Err(_) => return Err(i18n::text(Key::FailureIo).into()),
    };
    let input = host
        .library
        .create_archive_input()
        .map_err(error)?
        .wait()
        .await
        .map_err(error)?;
    let destination = files::local(input.path());
    let stop = Arc::clone(&model.borrow().stop);
    let cancellable = model.borrow().file_cancel.clone();
    let copied = gio::spawn_blocking(move || {
        files::copy(
            &file,
            &destination,
            memedock_core::ResourceLimits::default().max_archive_bytes,
            &stop,
            &cancellable,
        )
    })
    .await;
    if !matches!(copied, Ok(Ok(()))) {
        host.library
            .discard_archive_input(input)
            .map_err(error)?
            .wait()
            .await
            .map_err(error)?;
        return Err(i18n::text(Key::FailureIo).into());
    }
    let task = host.library.inspect_archive(input).map_err(error)?;
    model.borrow_mut().controller = Some(task.controller());
    task.wait().await.map(Some).map_err(error)
}
fn restore(host: Host, model: Rc<RefCell<Model>>, view: Rc<View>, mode: RestoreMode) {
    if !begin(&host, &model, &view, Phase::Restoring) {
        return;
    }
    let preview = model.borrow_mut().preview.take();
    let Some(preview) = preview else {
        finish(&host, &model, &view, Ok(false), Key::BackupRestored);
        return;
    };
    glib::spawn_future_local(async move {
        let mut reconnect = false;
        let result = async {
            let task = host
                .library
                .restore_archive(Arc::clone(&preview), mode)
                .map_err(error)?;
            model.borrow_mut().controller = Some(task.controller());
            let prepared = task.wait().await.map_err(error)?;
            reconnect = mode == RestoreMode::Replace;
            host.library
                .complete_restore(prepared)
                .await
                .map_err(error)?;
            Ok(true)
        }
        .await;
        let _ = cleanup(&host.library, preview).await;
        finish(&host, &model, &view, result, Key::BackupRestored);
        if reconnect {
            host.maintenance.set(true);
            let _ = host.library.close().await;
            view.dialog.force_close();
            host.maintenance.set(false);
            (host.reopen)();
        }
    });
}
async fn cleanup(library: &Library, preview: Arc<PreparedArchive>) -> memedock_core::Result<()> {
    library.discard_prepared_archive(preview)?.wait().await
}
fn error(e: memedock_core::CoreError) -> String {
    i18n::core(e.code()).into()
}
fn summary_text(s: &memedock_core::RestoreSummary) -> String {
    format!(
        "{}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n\n{}",
        i18n::text(Key::BackupPreview),
        i18n::text(Key::TabStickers),
        s.stickers - s.deleted_stickers,
        i18n::text(Key::Trash),
        s.deleted_stickers,
        i18n::text(Key::TabCollections),
        s.collections,
        i18n::text(Key::TagsTitle),
        s.tags,
        i18n::text(Key::OriginalBytes),
        crate::settings::bytes(i64::try_from(s.original_bytes).unwrap_or(i64::MAX)),
        i18n::text(Key::RestoreHintNew)
    )
}
