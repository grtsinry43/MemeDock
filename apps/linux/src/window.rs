use crate::i18n::{self, Key};
use crate::import::{self, Batch};
use crate::output::Hold;
use crate::paths::{self, PathError};
use gtk4::prelude::*;
use gtk4::{gio, glib, glib::Propagation};
use libadwaita as adw;
use libadwaita::prelude::*;
use memedock_core::{CoreError, Library, LibraryConfig};
use std::cell::RefCell;
use std::rc::Rc;

struct WindowState {
    toolbar: adw::ToolbarView,
    page: adw::StatusPage,
    spinner: adw::Spinner,
    retry: gtk4::Button,
    header: adw::HeaderBar,
    library: Option<Library>,
    open_task: Option<glib::JoinHandle<()>>,
    browse_task: Option<glib::JoinHandle<()>>,
    imports: Rc<RefCell<Batch>>,
    clipboard: Rc<RefCell<Hold>>,
    closing: bool,
    window: glib::WeakRef<adw::ApplicationWindow>,
    pending_files: Vec<gio::File>,
    importer: Option<Rc<import::Controls>>,
}

enum OpenFailure {
    Paths(PathError),
    Library(CoreError),
}

impl OpenFailure {
    fn message(&self) -> &'static str {
        match self {
            Self::Paths(error) => error.message(),
            Self::Library(error) => i18n::core(error.code()),
        }
    }
}

pub fn connect(app: &adw::Application) {
    let current = Rc::new(RefCell::new(None));
    let for_activate = Rc::clone(&current);
    app.connect_activate(move |app| activate(app, &for_activate, &[]));
    app.connect_open(move |app, files, _| activate(app, &current, files));
}

fn activate(
    app: &adw::Application,
    current: &Rc<RefCell<Option<Rc<RefCell<WindowState>>>>>,
    files: &[gio::File],
) {
    let existing = current.borrow().clone();
    if let Some(session) = existing {
        if let Some(window) = session.borrow().window.upgrade() {
            window.present();
        }
        accept_files(&session, files);
        return;
    }

    let page = adw::StatusPage::new();
    let spinner = adw::Spinner::new();
    spinner.set_halign(gtk4::Align::Center);
    spinner.set_valign(gtk4::Align::Center);
    spinner.set_size_request(32, 32);
    let retry = gtk4::Button::with_label(i18n::text(Key::Retry));
    retry.add_css_class("suggested-action");
    retry.set_halign(gtk4::Align::Center);

    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));

    let session = Rc::new(RefCell::new(WindowState {
        toolbar: toolbar.clone(),
        header,
        page: page.clone(),
        spinner: spinner.clone(),
        retry: retry.clone(),
        library: None,
        open_task: None,
        browse_task: None,
        imports: Rc::new(RefCell::new(Batch::new())),
        clipboard: Rc::new(RefCell::new(Hold::new())),
        closing: false,
        window: glib::WeakRef::new(),
        pending_files: Vec::new(),
        importer: None,
    }));

    let retry_session = Rc::clone(&session);
    retry.connect_clicked(move |_| start_open(Rc::clone(&retry_session)));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("MemeDock")
        .default_width(960)
        .default_height(640)
        .content(&toolbar)
        .build();
    session.borrow_mut().window.set(Some(&window));
    current.borrow_mut().replace(Rc::clone(&session));
    let for_destroy = Rc::clone(current);
    window.connect_destroy(move |_| {
        for_destroy.borrow_mut().take();
    });

    let close_session = Rc::clone(&session);
    window.connect_close_request(move |window| {
        let mut state = close_session.borrow_mut();
        if state.closing {
            return Propagation::Stop;
        }
        state.closing = true;
        let imports = Rc::clone(&state.imports);
        let shutdown = imports.borrow_mut().begin_shutdown();
        drop(state);
        if let Some(task) = shutdown.task {
            task.abort();
        }
        if let Some(dialog) = shutdown.review {
            dialog.close();
        }
        if let Some(dialog) = shutdown.problems {
            dialog.close();
        }
        let inputs = imports.borrow_mut().take_inputs();
        let mut state = close_session.borrow_mut();
        if let Some(task) = state.browse_task.take() {
            task.abort();
        }
        if let Some(task) = state.open_task.take() {
            task.abort();
        }
        let library = state.library.take();
        let clipboard = Rc::clone(&state.clipboard);
        state.importer.take();
        state.pending_files.clear();
        drop(state);
        let output_task = clipboard.borrow_mut().begin_shutdown();
        crate::clipboard::begin_shutdown(&clipboard.borrow().clipboard);
        if let Some(task) = output_task {
            task.abort();
        }
        if let Some(library) = library {
            let window = window.clone();
            glib::spawn_future_local(async move {
                let _ = glib::future_with_timeout(
                    std::time::Duration::from_secs(2),
                    gtk4::prelude::WidgetExt::display(&window)
                        .clipboard()
                        .store_future(glib::Priority::DEFAULT),
                )
                .await;
                clipboard.borrow_mut().release();
                import::discard_held(&library, inputs).await;
                let _ = library.close().await;
                window.destroy();
            });
            // Keep the window until staged files are discarded and the library closes.
            Propagation::Stop
        } else {
            Propagation::Proceed
        }
    });

    window.present();
    accept_files(&session, files);
    start_open(session);
}

fn accept_files(session: &Rc<RefCell<WindowState>>, files: &[gio::File]) {
    if files.is_empty() {
        return;
    }
    let (importer, window, error) = {
        let mut state = session.borrow_mut();
        let error = if state.closing {
            Some(Key::FailureBusy)
        } else if import::batch_too_large(files.len()) {
            Some(Key::FailureBatch)
        } else if state.importer.is_none() {
            queue_files(&mut state.pending_files, files)
                .err()
                .map(|()| Key::FailureBatch)
        } else {
            None
        };
        (state.importer.clone(), state.window.upgrade(), error)
    };
    if let Some(error) = error {
        if let Some(window) = window {
            let dialog =
                adw::AlertDialog::new(Some(i18n::text(Key::FailureInput)), Some(i18n::text(error)));
            dialog.add_response("close", i18n::text(Key::Done));
            dialog.present(Some(&window));
        }
    } else if let Some(importer) = importer {
        import::begin_external(importer, files.to_vec(), None);
    }
}

fn queue_files(pending: &mut Vec<gio::File>, files: &[gio::File]) -> Result<(), ()> {
    let mut unique = pending.clone();
    for file in files {
        if !unique.iter().any(|existing| existing.equal(file)) {
            unique.push(file.clone());
        }
        if import::batch_too_large(unique.len()) {
            return Err(());
        }
    }
    *pending = unique;
    Ok(())
}

fn start_open(session: Rc<RefCell<WindowState>>) {
    {
        let state = session.borrow();
        if state.closing || state.library.is_some() {
            return;
        }
    }
    {
        let mut state = session.borrow_mut();
        if let Some(task) = state.open_task.take() {
            task.abort();
        }
        show_opening(&state);
    }

    let session_for_open = Rc::clone(&session);
    let task = glib::spawn_future_local(async move {
        let opened = match paths::from_env() {
            Ok(paths) => Library::open(LibraryConfig::new(
                paths.data_dir,
                paths.cache_dir,
                paths.export_dir,
            ))
            .await
            .map_err(OpenFailure::Library),
            Err(error) => Err(OpenFailure::Paths(error)),
        };
        let closing = session_for_open.borrow().closing;
        if closing {
            if let Ok(library) = opened {
                let _ = library.close().await;
            }
            return;
        }
        match opened {
            Ok(library) => {
                let mut state = session_for_open.borrow_mut();
                state.open_task = None;
                let header = state.header.clone();
                let imports = Rc::clone(&state.imports);
                let clipboard = Rc::clone(&state.clipboard);
                let (handle, importer) = crate::library_view::attach(
                    &state.toolbar,
                    &header,
                    library.clone(),
                    imports,
                    clipboard,
                );
                state.library = Some(library);
                state.browse_task = Some(handle);
                state.importer = Some(Rc::clone(&importer));
                let files = std::mem::take(&mut state.pending_files);
                drop(state);
                if !files.is_empty() {
                    import::begin_external(importer, files, None);
                }
            }
            Err(error) => {
                let mut state = session_for_open.borrow_mut();
                state.open_task = None;
                show_failed(&state, error.message());
            }
        }
    });
    session.borrow_mut().open_task = Some(task);
}

fn show_opening(state: &WindowState) {
    state.page.set_title(i18n::text(Key::ImportPreparing));
    state.page.set_description(None);
    state.page.set_icon_name(None);
    state.page.set_child(Some(&state.spinner));
}

fn show_failed(state: &WindowState, message: &str) {
    let escaped = glib::markup_escape_text(message);
    state.page.set_title(i18n::text(Key::FailureLibrary));
    state.page.set_description(Some(escaped.as_str()));
    state.page.set_icon_name(Some("dialog-error-symbolic"));
    state.page.set_child(Some(&state.retry));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_requests_are_deduplicated_and_overflow_preserves_the_pending_batch() {
        let first = gio::File::for_path("/tmp/one sticker.png");
        let mut pending = vec![first.clone()];
        assert!(queue_files(&mut pending, &[gio::File::for_path("/tmp/one sticker.png")]).is_ok());
        assert_eq!(pending.len(), 1);
        let overflow: Vec<_> = (0..200)
            .map(|i| gio::File::for_path(format!("/tmp/{i}.png")))
            .collect();
        assert!(queue_files(&mut pending, &overflow).is_err());
        assert_eq!(pending.len(), 1);
    }
}
