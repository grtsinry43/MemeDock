use crate::i18n::{self, Key};
use crate::import::{self, Batch};
use crate::output::Hold;
use crate::paths::{self, PathError};
use gtk4::prelude::*;
use gtk4::{glib, glib::Propagation};
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

pub fn activate(app: &adw::Application) {
    if let Some(window) = app.active_window() {
        crate::output::log_process("激活已有窗口");
        window.present();
        return;
    }
    crate::output::log_process("创建新窗口");

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

    let close_session = Rc::clone(&session);
    window.connect_close_request(move |window| {
        let mut state = close_session.borrow_mut();
        if state.closing {
            return Propagation::Proceed;
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
        drop(state);
        if let Some(library) = library {
            let window = window.clone();
            glib::spawn_future_local(async move {
                let _ = gtk4::prelude::WidgetExt::display(&window)
                    .clipboard()
                    .store_future(glib::Priority::DEFAULT)
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
    start_open(session);
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
        let mut state = session_for_open.borrow_mut();
        state.open_task = None;
        match opened {
            Ok(library) => {
                let header = state.header.clone();
                let imports = Rc::clone(&state.imports);
                let clipboard = Rc::clone(&state.clipboard);
                let handle = crate::library_view::attach(
                    &state.toolbar,
                    &header,
                    library.clone(),
                    imports,
                    clipboard,
                );
                state.library = Some(library);
                state.browse_task = Some(handle);
            }
            Err(error) => show_failed(&state, error.message()),
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
