pub mod sticker;

use crate::actions::{self, Host};
use crate::browse::{self, BrowseFilter, EmptyCopy};
use crate::detail::{self, DetailChrome, DetailLinks};
use crate::i18n::{self, Key};
use crate::output::Hold;
use gtk4::gdk::ModifierType;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use memedock_core::events::{ChangeKind, Notification};
use memedock_core::tasks::Priority;
use memedock_core::{CoreError, Library, QueryCursor, QueryRequest, RequestId};
use memedock_domain::asset::ImageFormat;
use memedock_domain::export::ExportPreset;
use memedock_domain::identity::{CollectionId, StickerId, TagId};
use memedock_domain::version::{Generation, Revision};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use sticker::StickerObject;

const SEARCH_DELAY: Duration = Duration::from_millis(250);

struct Session {
    library: Library,
    alive: Rc<Cell<bool>>,
    filter: BrowseFilter,
    filters: Vec<BrowseFilter>,
    search: String,
    generation: u64,
    cursor: Option<QueryCursor>,
    loading: bool,
    rebuilding: bool,
    sidebar: adw::Sidebar,
    store: gio::ListStore,
    grid: gtk4::GridView,
    stack: gtk4::Stack,
    search_entry: gtk4::SearchEntry,
    empty_page: adw::StatusPage,
    error_page: adw::StatusPage,
    toasts: adw::ToastOverlay,
    navigation: adw::NavigationView,
    batch: gtk4::Button,
    empty_add: gtk4::Button,
    empty_clear: gtk4::Button,
    selected_collection: Rc<Cell<Option<CollectionId>>>,
    playback: Rc<RefCell<Option<crate::playback::Playback>>>,
    detail_refresh: detail::DetailRefresh,
    detail_sync: detail::DetailRefresh,
    open_id: Rc<Cell<Option<actions::OpenTarget>>>,
    clipboard: Rc<RefCell<Hold>>,
    preset: Rc<Cell<ExportPreset>>,
    menu: Rc<RefCell<Option<gtk4::Popover>>>,
    focused: Rc<Cell<u32>>,
    query_task: Option<glib::JoinHandle<()>>,
    debounce_task: Option<glib::JoinHandle<()>>,
}

struct Stop {
    alive: Rc<Cell<bool>>,
    session: Rc<RefCell<Session>>,
}

impl Drop for Stop {
    fn drop(&mut self) {
        self.alive.set(false);
        let mut state = self.session.borrow_mut();
        if let Some(task) = state.query_task.take() {
            task.abort();
        }
        if let Some(task) = state.debounce_task.take() {
            task.abort();
        }
        state.playback.borrow_mut().take();
        state.detail_refresh.borrow_mut().take();
        state.detail_sync.borrow_mut().take();
    }
}

pub fn attach(
    toolbar: &adw::ToolbarView,
    header: &adw::HeaderBar,
    library: Library,
    imports: Rc<RefCell<crate::import::Batch>>,
    clipboard: Rc<RefCell<Hold>>,
) -> glib::JoinHandle<()> {
    install_style();
    let alive = Rc::new(Cell::new(true));
    let ui = build_ui();
    let session = Rc::new(RefCell::new(Session {
        library,
        alive: Rc::clone(&alive),
        filter: BrowseFilter::Recent,
        filters: Vec::new(),
        search: String::new(),
        generation: 0,
        cursor: None,
        loading: false,
        rebuilding: false,
        sidebar: ui.sidebar.clone(),
        store: ui.store.clone(),
        grid: ui.grid.clone(),
        stack: ui.stack.clone(),
        search_entry: ui.search.clone(),
        empty_page: ui.empty_page.clone(),
        error_page: ui.error_page.clone(),
        toasts: ui.toasts.clone(),
        navigation: ui.navigation.clone(),
        batch: ui.batch.clone(),
        empty_add: ui.empty_add.clone(),
        empty_clear: ui.empty_clear.clone(),
        selected_collection: Rc::new(Cell::new(None)),
        playback: Rc::new(RefCell::new(None)),
        detail_refresh: Rc::new(RefCell::new(None)),
        detail_sync: Rc::new(RefCell::new(None)),
        open_id: Rc::new(Cell::new(None)),
        clipboard,
        preset: Rc::new(Cell::new(ExportPreset::Original)),
        menu: Rc::new(RefCell::new(None)),
        focused: Rc::new(Cell::new(gtk4::INVALID_LIST_POSITION)),
        query_task: None,
        debounce_task: None,
    }));
    populate_sidebar(&session, &[], &[]);
    wire_search(Rc::clone(&session));
    wire_sidebar(Rc::clone(&session));
    wire_items(&ui.factory, Rc::clone(&session));
    wire_menu(&ui.factory, Rc::clone(&session));
    wire_copy(toolbar, Rc::clone(&session));
    let back = gtk4::Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some(i18n::text(Key::Back)));
    back.set_visible(false);
    header.pack_start(&back);
    let header_title = gtk4::Label::new(Some(i18n::text(Key::TabStickers)));
    header_title.add_css_class("title");
    header.set_title_widget(Some(&header_title));
    let star = gtk4::Button::new();
    star.set_visible(false);
    let chrome = DetailChrome::new(header_title.clone(), star.clone());
    wire_open(Rc::clone(&session), back, header_title, chrome);
    wire_batch(Rc::clone(&session));
    wire_retry(Rc::clone(&session));
    let selected_collection = Rc::clone(&session.borrow().selected_collection);
    crate::import::wire_stop(&ui.progress, Rc::clone(&imports));
    crate::import::bind(
        header,
        &ui.overlay,
        &ui.veil,
        crate::import::Controls {
            batch: imports,
            library: session.borrow().library.clone(),
            toasts: ui.toasts.clone(),
            alive: Rc::clone(&alive),
            parent: ui.overlay.clone().upcast(),
            progress: ui.progress.clone(),
            progress_label: ui.progress_label.clone(),
        },
        selected_collection,
        &session.borrow().empty_add,
    );
    header.pack_end(&star);
    toolbar.set_content(Some(&ui.overlay));
    let search = ui.search.clone();
    let clear_search = ui.search.clone();
    ui.empty_clear.connect_clicked(move |_| {
        clear_search.set_text("");
    });
    glib::idle_add_local_once(move || {
        search.grab_focus();
    });

    let session_for_events = Rc::clone(&session);
    glib::spawn_future_local(async move {
        let _stop = Stop {
            alive,
            session: Rc::clone(&session_for_events),
        };
        let library = session_for_events.borrow().library.clone();
        let mut subscription = match library.subscribe() {
            Ok(subscription) => subscription,
            Err(error) => {
                show_error(&session_for_events, i18n::core(error.code()));
                return;
            }
        };
        spawn_reload(Rc::clone(&session_for_events), true);
        loop {
            if !session_for_events.borrow().alive.get() {
                break;
            }
            match subscription.next().await {
                Ok(Notification::Closed) | Err(_) => break,
                Ok(Notification::Changed(event)) => match event.kind {
                    ChangeKind::ThumbnailChanged(id) => {
                        let session = Rc::clone(&session_for_events);
                        glib::spawn_future_local(async move {
                            refresh_thumbnail(session, id).await;
                        });
                    }
                    ChangeKind::CollectionsChanged | ChangeKind::TagsChanged => {
                        spawn_reload(Rc::clone(&session_for_events), true);
                        let refresh = session_for_events
                            .borrow()
                            .detail_refresh
                            .borrow()
                            .as_ref()
                            .map(Rc::clone);
                        if let Some(refresh) = refresh {
                            refresh();
                        }
                    }
                    _ => spawn_reload(Rc::clone(&session_for_events), true),
                },
                Ok(Notification::ReloadRequired { .. }) => {
                    spawn_reload(Rc::clone(&session_for_events), true);
                }
            }
        }
    })
}

struct Built {
    sidebar: adw::Sidebar,
    store: gio::ListStore,
    grid: gtk4::GridView,
    factory: gtk4::SignalListItemFactory,
    stack: gtk4::Stack,
    search: gtk4::SearchEntry,
    empty_page: adw::StatusPage,
    error_page: adw::StatusPage,
    toasts: adw::ToastOverlay,
    navigation: adw::NavigationView,
    batch: gtk4::Button,
    empty_add: gtk4::Button,
    empty_clear: gtk4::Button,
    overlay: gtk4::Overlay,
    veil: gtk4::Box,
    progress: gtk4::Box,
    progress_label: gtk4::Label,
}

fn build_ui() -> Built {
    let sidebar = adw::Sidebar::new();
    sidebar.set_size_request(220, -1);
    let sidebar_page = adw::NavigationPage::new(&sidebar, i18n::text(Key::TabStickers));

    let search = gtk4::SearchEntry::new();
    search.set_placeholder_text(Some(i18n::text(Key::LibrarySearch)));
    search.set_hexpand(true);
    let batch = gtk4::Button::with_label(i18n::text(Key::Delete));
    batch.set_visible(false);
    batch.set_valign(gtk4::Align::Center);
    let search_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    search_bar.set_margin_top(12);
    search_bar.set_margin_bottom(6);
    search_bar.set_margin_start(12);
    search_bar.set_margin_end(12);
    search_bar.append(&search);
    search_bar.append(&batch);

    let loading = adw::StatusPage::new();
    loading.set_title(i18n::text(Key::ImportPreparing));
    let spinner = adw::Spinner::new();
    spinner.set_size_request(32, 32);
    spinner.set_halign(gtk4::Align::Center);
    loading.set_child(Some(&spinner));

    let empty_page = adw::StatusPage::new();
    empty_page.set_icon_name(Some("image-x-generic-symbolic"));
    empty_page.set_title(i18n::text(Key::LibraryEmpty));
    empty_page.set_description(Some(i18n::text(Key::LibraryEmptyHint)));
    let empty_add = gtk4::Button::with_label(i18n::text(Key::ImportPhotos));
    empty_add.add_css_class("suggested-action");
    empty_add.set_halign(gtk4::Align::Center);
    let empty_clear = gtk4::Button::with_label(i18n::text(Key::ClearSearch));
    empty_clear.set_halign(gtk4::Align::Center);
    empty_page.set_child(Some(&empty_add));

    let error_page = adw::StatusPage::new();
    error_page.set_icon_name(Some("dialog-error-symbolic"));
    error_page.set_title(i18n::text(Key::LibraryLoadTitle));
    let retry = gtk4::Button::with_label(i18n::text(Key::Retry));
    retry.add_css_class("suggested-action");
    retry.set_halign(gtk4::Align::Center);
    retry.set_widget_name("library-retry");
    error_page.set_child(Some(&retry));

    let store = gio::ListStore::new::<StickerObject>();
    let selection = gtk4::MultiSelection::new(Some(store.clone()));
    let factory = item_factory();
    let grid = gtk4::GridView::new(Some(selection), Some(factory.clone()));
    grid.set_single_click_activate(false);
    grid.set_enable_rubberband(false);
    grid.set_min_columns(2);
    grid.set_max_columns(8);
    grid.set_vexpand(true);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&grid));
    scroll.set_vexpand(true);

    let stack = gtk4::Stack::new();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&empty_page, Some("empty"));
    stack.add_named(&error_page, Some("error"));
    stack.add_named(&scroll, Some("grid"));
    stack.set_vexpand(true);
    stack.set_visible_child_name("loading");

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&stack));
    toasts.set_vexpand(true);

    let progress_label = gtk4::Label::new(None);
    progress_label.set_hexpand(true);
    progress_label.set_halign(gtk4::Align::Start);
    let stop_import = gtk4::Button::with_label(i18n::text(Key::ImportCancelRemaining));
    let progress = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    progress.set_visible(false);
    progress.set_margin_start(12);
    progress.set_margin_end(12);
    progress.set_margin_bottom(6);
    progress.append(&progress_label);
    progress.append(&stop_import);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&search_bar);
    content.append(&progress);
    content.append(&toasts);
    let browser_page = adw::NavigationPage::new(&content, i18n::text(Key::TabStickers));
    let navigation = adw::NavigationView::new();
    navigation.add(&browser_page);
    let content_page = adw::NavigationPage::new(&navigation, i18n::text(Key::TabStickers));

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&sidebar_page));
    split.set_content(Some(&content_page));
    split.set_vexpand(true);
    split.set_hexpand(true);
    split.set_sidebar_width_fraction(0.28);

    let veil_label = gtk4::Label::new(Some(i18n::text(Key::DropToImport)));
    veil_label.set_halign(gtk4::Align::Center);
    veil_label.set_valign(gtk4::Align::Center);
    let veil = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    veil.add_css_class("import-veil");
    veil.set_visible(false);
    veil.set_can_target(false);
    veil.set_halign(gtk4::Align::Fill);
    veil.set_valign(gtk4::Align::Fill);
    veil.set_hexpand(true);
    veil.set_vexpand(true);
    veil.append(&veil_label);
    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&split));
    overlay.add_overlay(&veil);
    overlay.set_hexpand(true);
    overlay.set_vexpand(true);

    Built {
        sidebar,
        store,
        grid,
        factory,
        stack,
        search,
        empty_page,
        error_page,
        toasts,
        navigation,
        batch,
        empty_add,
        empty_clear,
        overlay,
        veil,
        progress,
        progress_label,
    }
}

fn remember_collection(state: &Session) {
    let selected = match state.filter {
        BrowseFilter::Collection(id) => Some(id),
        _ => None,
    };
    state.selected_collection.set(selected);
}

fn item_factory() -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let Some(list_item) = item.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let picture = gtk4::Picture::new();
        picture.set_content_fit(gtk4::ContentFit::ScaleDown);
        picture.set_can_shrink(true);
        let badge = gtk4::Label::new(Some(i18n::text(Key::AnimatedImage)));
        badge.add_css_class("sticker-badge");
        badge.set_halign(gtk4::Align::End);
        badge.set_valign(gtk4::Align::Start);
        badge.set_visible(false);
        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&picture));
        overlay.add_overlay(&badge);
        overlay.add_css_class("sticker-tile");
        overlay.set_size_request(148, 148);
        list_item.set_child(Some(&overlay));
    });
    factory
}

fn wire_items(factory: &gtk4::SignalListItemFactory, session: Rc<RefCell<Session>>) {
    let session_for_bind = Rc::clone(&session);
    factory.connect_bind(move |_, item| {
        let Some(list_item) = item.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let Some(object) = list_item
            .item()
            .and_then(|item| item.downcast::<StickerObject>().ok())
        else {
            return;
        };
        let Some(overlay) = list_item
            .child()
            .and_then(|child| child.downcast::<gtk4::Overlay>().ok())
        else {
            return;
        };
        let Some(picture) = find_descendant::<gtk4::Picture>(&overlay) else {
            return;
        };
        if let Some(badge) = find_descendant::<gtk4::Label>(&overlay) {
            badge.set_visible(object.animated());
        }
        picture.set_paintable(None::<&gtk4::gdk::Texture>);
        object.watch_picture(&picture);
        let token = object.generation().saturating_add(1);
        object.set_generation(token);
        let stored = object.thumbnail();
        if stored.is_empty() {
            request_missing_thumbnail(Rc::clone(&session_for_bind), object, token);
        } else {
            load_file(picture, PathBuf::from(stored), token, object);
        }
        let position = list_item.position();
        let count = session_for_bind.borrow().store.n_items();
        if browse::near_end(position, count) {
            let session = Rc::clone(&session_for_bind);
            glib::idle_add_local_once(move || spawn_append(session));
        }
    });
    let session_for_unbind = Rc::clone(&session);
    factory.connect_unbind(move |_, item| {
        let Some(list_item) = item.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let Some(object) = list_item
            .item()
            .and_then(|item| item.downcast::<StickerObject>().ok())
        else {
            return;
        };
        object.set_generation(object.generation().saturating_add(1));
        object.clear_picture();
        if let Some(overlay) = list_item.child()
            && let Some(picture) = find_descendant::<gtk4::Picture>(&overlay)
        {
            picture.set_paintable(None::<&gtk4::gdk::Texture>);
        }
        let menu = Rc::clone(&session_for_unbind.borrow().menu);
        let current = menu.borrow().clone();
        if let Some(popover) = current
            && popover.parent().as_ref() == list_item.child().as_ref()
        {
            actions::dismiss(&menu);
        }
    });
}

fn wire_menu(factory: &gtk4::SignalListItemFactory, session: Rc<RefCell<Session>>) {
    factory.connect_setup(move |_, item| {
        let Some(list_item) = item.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let Some(overlay) = list_item
            .child()
            .and_then(|child| child.downcast::<gtk4::Overlay>().ok())
        else {
            return;
        };
        let click = gtk4::GestureClick::new();
        click.set_button(3);
        let session_for_click = Rc::clone(&session);
        let list_item_for_click = list_item.clone();
        click.connect_pressed(move |gesture, _, _, _| {
            let _ = gesture.set_state(gtk4::EventSequenceState::Claimed);
            let Some(object) = list_item_for_click
                .item()
                .and_then(|item| item.downcast::<StickerObject>().ok())
            else {
                return;
            };
            let Ok(id) = object.identity().parse::<StickerId>() else {
                return;
            };
            let Ok(generation) = Generation::new(object.entity_generation()) else {
                return;
            };
            let Ok(revision) = Revision::new(object.entity_revision()) else {
                return;
            };
            let deleted = session_for_click.borrow().filter == BrowseFilter::Trash;
            let target = actions::Target {
                id,
                generation,
                revision,
                animated: object.animated(),
                starred: object.starred(),
                deleted,
                source: object.source_format(),
            };
            let host = host_of(&session_for_click);
            let Some(anchor) = gesture.widget() else {
                return;
            };
            actions::popup(&host, &anchor, target);
        });
        overlay.add_controller(click);
        let focus = gtk4::EventControllerFocus::new();
        let focused = Rc::clone(&session.borrow().focused);
        let list_item_for_focus = list_item.clone();
        focus.connect_enter(move |_| {
            focused.set(list_item_for_focus.position());
        });
        overlay.add_controller(focus);
    });
}

fn wire_copy(toolbar: &adw::ToolbarView, session: Rc<RefCell<Session>>) {
    let controller = gtk4::ShortcutController::new();
    controller.set_scope(gtk4::ShortcutScope::Global);
    let Some(trigger) = gtk4::ShortcutTrigger::parse_string("<Control>c") else {
        return;
    };
    let action = gtk4::CallbackAction::new(move |widget, _| {
        if text_focused(widget) {
            return glib::Propagation::Proceed;
        }
        copy_focused(&session);
        glib::Propagation::Stop
    });
    controller.add_shortcut(gtk4::Shortcut::new(Some(trigger), Some(action)));
    toolbar.add_controller(controller);
}

fn text_focused(widget: &gtk4::Widget) -> bool {
    let Some(focus) = widget.root().and_then(|root| root.focus()) else {
        return false;
    };
    focus.is::<gtk4::Text>()
        || focus.is::<gtk4::TextView>()
        || focus.is::<gtk4::Entry>()
        || focus.is::<gtk4::SearchEntry>()
}

fn copy_focused(session: &Rc<RefCell<Session>>) {
    let host = host_of(session);
    let on_detail = {
        let state = session.borrow();
        state
            .navigation
            .visible_page()
            .and_then(|page| page.tag())
            .as_deref()
            == Some("detail")
    };
    let open = session.borrow().open_id.get();
    if on_detail && let Some(open) = open {
        output_copy(&host, open.id, open.animated, open.source);
        return;
    }
    let Some(object) = focused_sticker(session) else {
        return;
    };
    let Ok(id) = object.identity().parse::<StickerId>() else {
        return;
    };
    output_copy(&host, id, object.animated(), object.source_format());
}

fn output_copy(host: &Host, id: StickerId, animated: bool, source: ImageFormat) {
    crate::output::spawn_copy(
        host.library.clone(),
        Rc::clone(&host.clipboard),
        &host.navigation,
        crate::output::Request {
            id,
            animated,
            preset: host.preset.get(),
            first_frame: false,
            source,
        },
        crate::output::CopyKind::Image,
        {
            let toasts = host.toasts.clone();
            move |message| {
                if let Some(message) = message {
                    toasts.add_toast(adw::Toast::new(message));
                }
            }
        },
    );
}

fn focused_sticker(session: &Rc<RefCell<Session>>) -> Option<StickerObject> {
    let state = session.borrow();
    let model = state.grid.model()?;
    let position = state.focused.get();
    if position != gtk4::INVALID_LIST_POSITION
        && let Some(object) = model.item(position).and_then(|item| item.downcast().ok())
    {
        return Some(object);
    }
    let selected = selected_stickers(&state.grid);
    if selected.len() == 1 {
        selected.into_iter().next()
    } else {
        None
    }
}

fn host_of(session: &Rc<RefCell<Session>>) -> Host {
    let state = session.borrow();
    let show = Rc::clone(session);
    Host {
        library: state.library.clone(),
        clipboard: Rc::clone(&state.clipboard),
        preset: Rc::clone(&state.preset),
        toasts: state.toasts.clone(),
        navigation: state.navigation.clone(),
        open_id: Rc::clone(&state.open_id),
        detail_sync: Rc::clone(&state.detail_sync),
        menu: Rc::clone(&state.menu),
        show_trash: Rc::new(move || show_trash(&show)),
    }
}

fn show_trash(session: &Rc<RefCell<Session>>) {
    let (sidebar, index) = {
        let state = session.borrow();
        let Some(index) = state
            .filters
            .iter()
            .position(|filter| *filter == BrowseFilter::Trash)
        else {
            return;
        };
        (state.sidebar.clone(), u32::try_from(index).unwrap_or(0))
    };
    sidebar.set_selected(index);
}

fn wire_search(session: Rc<RefCell<Session>>) {
    let entry = session.borrow().search_entry.clone();
    let grid = session.borrow().grid.clone();
    let keys = gtk4::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, state| {
        if key == gtk4::gdk::Key::Down && state == ModifierType::empty() {
            grid.grab_focus();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    entry.add_controller(keys);

    let session_for_search = Rc::clone(&session);
    entry.connect_search_changed(move |entry| {
        let text = entry.text().to_string();
        let session = Rc::clone(&session_for_search);
        if let Some(task) = session.borrow_mut().debounce_task.take() {
            task.abort();
        }
        let task_session = Rc::clone(&session);
        let task = glib::spawn_future_local(async move {
            glib::timeout_future(SEARCH_DELAY).await;
            if !task_session.borrow().alive.get() {
                return;
            }
            let current = task_session.borrow().search_entry.text().to_string();
            if current != text {
                return;
            }
            let unchanged = task_session.borrow().search == text;
            if unchanged {
                return;
            }
            task_session.borrow_mut().search = text;
            spawn_reload(task_session, false);
        });
        session.borrow_mut().debounce_task = Some(task);
    });
}

fn wire_sidebar(session: Rc<RefCell<Session>>) {
    let sidebar = session.borrow().sidebar.clone();
    sidebar.connect_activated(move |_, index| {
        let changed = {
            let mut state = session.borrow_mut();
            if state.rebuilding {
                return;
            }
            let Some(filter) = state.filters.get(index as usize).cloned() else {
                return;
            };
            if filter == state.filter {
                return;
            }
            state.filter = filter;
            remember_collection(&state);
            true
        };
        if changed {
            spawn_reload(Rc::clone(&session), false);
        }
    });
}

fn wire_open(
    session: Rc<RefCell<Session>>,
    back: gtk4::Button,
    header_title: gtk4::Label,
    chrome: DetailChrome,
) {
    let grid = session.borrow().grid.clone();
    let navigation = session.borrow().navigation.clone();
    let navigation_for_back = navigation.clone();
    back.connect_clicked(move |_| {
        navigation_for_back.pop();
    });
    let back_for_push = back.clone();
    navigation.connect_pushed(move |navigation| {
        let detail = navigation
            .visible_page()
            .and_then(|page| page.tag())
            .as_deref()
            == Some("detail");
        back_for_push.set_visible(detail);
    });
    let session_for_pop = Rc::clone(&session);
    let back_for_pop = back.clone();
    let title_for_pop = header_title.clone();
    let star_for_pop = chrome.star.clone();
    navigation.connect_popped(move |_, page| {
        if page.tag().as_deref() != Some("detail") {
            return;
        }
        back_for_pop.set_visible(false);
        star_for_pop.set_visible(false);
        title_for_pop.set_text(i18n::text(Key::TabStickers));
        let state = session_for_pop.borrow();
        state.playback.borrow_mut().take();
        state.detail_refresh.borrow_mut().take();
        state.detail_sync.borrow_mut().take();
        state.open_id.set(None);
    });
    let session_for_activate = Rc::clone(&session);
    grid.connect_activate(move |grid, position| {
        let Some(model) = grid.model() else {
            return;
        };
        let Some(object) = model
            .item(position)
            .and_then(|item| item.downcast::<StickerObject>().ok())
        else {
            return;
        };
        let (navigation, library, links) = {
            let state = session_for_activate.borrow();
            (
                state.navigation.clone(),
                state.library.clone(),
                DetailLinks {
                    playback: Rc::clone(&state.playback),
                    refresh: Rc::clone(&state.detail_refresh),
                    sync: Rc::clone(&state.detail_sync),
                },
            )
        };
        let host = host_of(&session_for_activate);
        detail::open(&navigation, library, &object, &links, &chrome, &host);
    });
}

fn wire_batch(session: Rc<RefCell<Session>>) {
    let grid = session.borrow().grid.clone();
    let Some(model) = grid.model() else {
        return;
    };
    let session_for_selection = Rc::clone(&session);
    model.connect_selection_changed(move |_, _, _| update_batch(&session_for_selection));
    let batch = session.borrow().batch.clone();
    batch.connect_clicked(move |_| start_batch(Rc::clone(&session)));
}

fn update_batch(session: &Rc<RefCell<Session>>) {
    let state = session.borrow();
    let count = selected_stickers(&state.grid).len();
    state.batch.set_visible(count > 1);
    state
        .batch
        .set_label(if state.filter == BrowseFilter::Trash {
            i18n::text(Key::Restore)
        } else {
            i18n::text(Key::Delete)
        });
}

fn start_batch(session: Rc<RefCell<Session>>) {
    let (grid, filter, navigation) = {
        let state = session.borrow();
        (
            state.grid.clone(),
            state.filter.clone(),
            state.navigation.clone(),
        )
    };
    let items = selected_stickers(&grid);
    if items.len() < 2 {
        return;
    }
    let restore = filter == BrowseFilter::Trash;
    let heading = if restore {
        i18n::text(Key::RestoreStickerTitle)
    } else {
        i18n::text(Key::Delete)
    };
    let body = if restore {
        i18n::text(Key::RestoreHint)
    } else {
        i18n::text(Key::DeleteHint)
    };
    let accept = if restore {
        i18n::text(Key::Restore)
    } else {
        i18n::text(Key::Delete)
    };
    let Some(parent) = navigation.visible_page() else {
        return;
    };
    let session = Rc::clone(&session);
    let items = items.clone();
    detail::confirm(&parent, heading, body, accept, !restore, move || {
        let session = Rc::clone(&session);
        let items = items.clone();
        glib::spawn_future_local(async move {
            apply_batch(session, items, restore).await;
        });
    });
}

fn selected_stickers(grid: &gtk4::GridView) -> Vec<StickerObject> {
    let Some(model) = grid.model() else {
        return Vec::new();
    };
    let bitset = model.selection();
    let Some((iter, first)) = gtk4::BitsetIter::init_first(&bitset) else {
        return Vec::new();
    };
    let mut positions = vec![first];
    positions.extend(iter);
    positions
        .into_iter()
        .filter_map(|position| model.item(position).and_then(|item| item.downcast().ok()))
        .collect()
}

async fn apply_batch(session: Rc<RefCell<Session>>, items: Vec<StickerObject>, restore: bool) {
    let library = session.borrow().library.clone();
    for object in items {
        if !session.borrow().alive.get() {
            return;
        }
        let Ok(id) = object.identity().parse::<StickerId>() else {
            toast(&session, i18n::text(Key::FailureUnknown));
            break;
        };
        let Ok(generation) = Generation::new(object.entity_generation()) else {
            toast(&session, i18n::text(Key::FailureUnknown));
            break;
        };
        let result = if restore {
            let Ok(revision) = Revision::new(object.entity_revision()) else {
                toast(&session, i18n::text(Key::FailureUnknown));
                break;
            };
            match library.restore_sticker(id, generation, revision) {
                Ok(task) => task.wait().await,
                Err(error) => Err(error),
            }
        } else {
            match library.delete_sticker(id, generation) {
                Ok(task) => task.wait().await,
                Err(error) => Err(error),
            }
        };
        if let Err(error) = result {
            toast(&session, i18n::core(error.code()));
            break;
        }
    }
    spawn_reload(session, false);
}

fn wire_retry(session: Rc<RefCell<Session>>) {
    let page = session.borrow().error_page.clone();
    let Some(button) = find_descendant::<gtk4::Button>(&page) else {
        return;
    };
    button.connect_clicked(move |_| spawn_reload(Rc::clone(&session), true));
}

fn spawn_reload(session: Rc<RefCell<Session>>, include_sidebar: bool) {
    let epoch = {
        let mut state = session.borrow_mut();
        if !state.alive.get() {
            return;
        }
        state.generation = state.generation.wrapping_add(1);
        state.cursor = None;
        state.loading = true;
        if let Some(task) = state.query_task.take() {
            task.abort();
        }
        state.generation
    };
    {
        let state = session.borrow();
        state.store.remove_all();
        state.stack.set_visible_child_name("loading");
    }
    let task_session = Rc::clone(&session);
    let task = glib::spawn_future_local(async move {
        if include_sidebar {
            refresh_sidebar(Rc::clone(&task_session), epoch).await;
        }
        if !still_current(&task_session, epoch) {
            return;
        }
        fetch_page(task_session, epoch, None).await;
    });
    session.borrow_mut().query_task = Some(task);
}

fn spawn_append(session: Rc<RefCell<Session>>) {
    let cursor = {
        let state = session.borrow();
        if !state.alive.get() || state.loading || state.cursor.is_none() {
            return;
        }
        state.cursor.clone()
    };
    let Some(cursor) = cursor else {
        return;
    };
    let epoch = {
        let mut state = session.borrow_mut();
        state.loading = true;
        if let Some(task) = state.query_task.take() {
            task.abort();
        }
        state.generation
    };
    let task_session = Rc::clone(&session);
    let task = glib::spawn_future_local(async move {
        fetch_page(task_session, epoch, Some(cursor)).await;
    });
    session.borrow_mut().query_task = Some(task);
}

async fn refresh_sidebar(session: Rc<RefCell<Session>>, epoch: u64) {
    let library = session.borrow().library.clone();
    let collections = match library.collections(false) {
        Ok(task) => task.wait().await,
        Err(error) => Err(error),
    };
    let tags = match library.tags(false) {
        Ok(task) => task.wait().await,
        Err(error) => Err(error),
    };
    if !still_current(&session, epoch) {
        return;
    }
    let (collections, tags) = match (collections, tags) {
        (Ok(collections), Ok(tags)) => (collections, tags),
        (Err(error), _) | (_, Err(error)) => {
            toast(&session, i18n::core(error.code()));
            return;
        }
    };
    let collection_rows: Vec<(CollectionId, String)> = collections
        .iter()
        .map(|collection| (collection.id(), collection.name().as_str().to_owned()))
        .collect();
    let tag_rows: Vec<(TagId, String)> = tags
        .iter()
        .map(|tag| (tag.id(), tag.name().as_str().to_owned()))
        .collect();
    if !still_current(&session, epoch) {
        return;
    }
    let mut state = session.borrow_mut();
    let filter_missing = match &state.filter {
        BrowseFilter::Collection(id) => !collection_rows.iter().any(|(item, _)| item == id),
        BrowseFilter::Tag(id) => !tag_rows.iter().any(|(item, _)| item == id),
        _ => false,
    };
    if filter_missing {
        state.filter = BrowseFilter::Recent;
        remember_collection(&state);
    }
    drop(state);
    populate_sidebar(&session, &collection_rows, &tag_rows);
}

fn populate_sidebar(
    session: &Rc<RefCell<Session>>,
    collections: &[(CollectionId, String)],
    tags: &[(TagId, String)],
) {
    session.borrow_mut().rebuilding = true;
    let sidebar = session.borrow().sidebar.clone();
    let current = session.borrow().filter.clone();
    sidebar.remove_all();
    let mut filters = Vec::new();
    let browse = adw::SidebarSection::new();
    for (title, filter) in [
        (i18n::text(Key::FilterRecent), BrowseFilter::Recent),
        (i18n::text(Key::FilterAll), BrowseFilter::All),
        (i18n::text(Key::FavoritesOnly), BrowseFilter::Starred),
    ] {
        browse.append(adw::SidebarItem::new(title));
        filters.push(filter);
    }
    sidebar.append(browse);

    let collection_section = adw::SidebarSection::new();
    collection_section.set_title(Some(i18n::text(Key::TabCollections)));
    for (id, name) in collections {
        collection_section.append(adw::SidebarItem::new(name));
        filters.push(BrowseFilter::Collection(*id));
    }
    sidebar.append(collection_section);

    let tag_section = adw::SidebarSection::new();
    tag_section.set_title(Some(i18n::text(Key::TagsTitle)));
    for (id, name) in tags {
        tag_section.append(adw::SidebarItem::new(name));
        filters.push(BrowseFilter::Tag(*id));
    }
    sidebar.append(tag_section);

    let trash = adw::SidebarSection::new();
    trash.append(adw::SidebarItem::new(i18n::text(Key::Trash)));
    filters.push(BrowseFilter::Trash);
    sidebar.append(trash);

    let selected = filters
        .iter()
        .position(|filter| filter == &current)
        .unwrap_or(0);
    let index = u32::try_from(selected).unwrap_or(0);
    session.borrow_mut().filters = filters;
    sidebar.set_selected(index);
    session.borrow_mut().rebuilding = false;
}

async fn fetch_page(session: Rc<RefCell<Session>>, epoch: u64, cursor: Option<QueryCursor>) {
    let (library, filter, text) = {
        let state = session.borrow();
        (
            state.library.clone(),
            state.filter.clone(),
            state.search.clone(),
        )
    };
    let request = QueryRequest {
        request_id: RequestId::new(),
        query: browse::sticker_query(&filter, &text),
        page_size: browse::PAGE_SIZE,
        cursor,
    };
    let response = match library.list_stickers(request) {
        Ok(task) => task.wait().await,
        Err(error) => Err(error),
    };
    if !still_current(&session, epoch) {
        return;
    }
    match response {
        Ok(page) => apply_page(&session, page),
        Err(error) => fail_page(&session, &error),
    }
}

fn apply_page(session: &Rc<RefCell<Session>>, page: memedock_core::QueryResponse) {
    let mut thumbs = std::collections::HashMap::new();
    for resource in page.resources {
        thumbs.insert(
            resource.asset.hash(),
            (
                resource.asset.animated(),
                resource.thumbnail_path,
                resource.asset.format().mime(),
            ),
        );
    }
    let mut objects = Vec::new();
    for sticker in page.stickers {
        let identity = sticker.id().to_string();
        let meta = thumbs.get(&sticker.id().content_hash());
        let animated = meta.is_some_and(|item| item.0);
        let thumbnail = meta.and_then(|item| item.1.clone());
        let mime = meta.map(|item| item.2).unwrap_or("image/png");
        let object = StickerObject::new(
            &identity,
            sticker.title(),
            animated,
            sticker.starred(),
            thumbnail.as_deref(),
            sticker.lifecycle().generation().get(),
            sticker.lifecycle().revision().get(),
        );
        object.set_source_mime(mime);
        objects.push(object);
    }
    let title = {
        let mut state = session.borrow_mut();
        state.cursor = page.next;
        state.loading = false;
        browse::empty_copy(&state.filter, &state.search)
    };
    let store = session.borrow().store.clone();
    for object in objects {
        store.append(&object);
    }
    let count = store.n_items();
    let state = session.borrow();
    if count == 0 {
        show_empty(&state, title);
        state.stack.set_visible_child_name("empty");
    } else {
        state.stack.set_visible_child_name("grid");
    }
}

fn fail_page(session: &Rc<RefCell<Session>>, error: &CoreError) {
    let mut state = session.borrow_mut();
    state.loading = false;
    state.cursor = None;
    let empty = state.store.n_items() == 0;
    if empty {
        let escaped = glib::markup_escape_text(i18n::core(error.code()));
        state.error_page.set_description(Some(escaped.as_str()));
        state.stack.set_visible_child_name("error");
    } else {
        drop(state);
        toast(session, i18n::core(error.code()));
    }
}

async fn refresh_thumbnail(session: Rc<RefCell<Session>>, id: StickerId) {
    let library = session.borrow().library.clone();
    let loaded = match library.request_thumbnail(id, Priority::Visible) {
        Ok(task) => task.wait().await.ok(),
        Err(_) => None,
    };
    if !session.borrow().alive.get() {
        return;
    }
    let Some(object) = find_sticker(&session.borrow().store, &id.to_string()) else {
        return;
    };
    let token = object.generation().saturating_add(1);
    object.set_generation(token);
    match loaded {
        Some(thumbnail) => {
            object.set_thumbnail(path_text(&thumbnail.path));
            if let Some(picture) = object.picture() {
                load_file(picture, thumbnail.path, token, object);
            }
        }
        None => {
            object.set_thumbnail(String::new());
            if let Some(picture) = object.picture() {
                picture.set_paintable(None::<&gtk4::gdk::Texture>);
            }
        }
    }
}

fn request_missing_thumbnail(session: Rc<RefCell<Session>>, object: StickerObject, token: u64) {
    let Ok(id) = object.identity().parse::<StickerId>() else {
        return;
    };
    glib::spawn_future_local(async move {
        let library = session.borrow().library.clone();
        let loaded = match library.request_thumbnail(id, Priority::Visible) {
            Ok(task) => task.wait().await.ok(),
            Err(_) => None,
        };
        if !session.borrow().alive.get() || object.generation() != token {
            return;
        }
        if let Some(thumbnail) = loaded {
            object.set_thumbnail(path_text(&thumbnail.path));
            if let Some(picture) = object.picture() {
                load_file(picture, thumbnail.path, token, object);
            }
        }
    });
}

fn load_file(picture: gtk4::Picture, path: PathBuf, token: u64, object: StickerObject) {
    let weak = picture.downgrade();
    glib::spawn_future_local(async move {
        let read = gio::spawn_blocking(move || std::fs::read(path)).await;
        let Ok(Ok(bytes)) = read else {
            return;
        };
        if object.generation() != token {
            return;
        }
        let Some(picture) = weak.upgrade() else {
            return;
        };
        // GdkTexture::from_bytes must run on the main thread.
        let gbytes = glib::Bytes::from_owned(bytes);
        if object.generation() == token
            && let Ok(texture) = gtk4::gdk::Texture::from_bytes(&gbytes)
        {
            picture.set_paintable(Some(&texture));
        }
    });
}

fn find_sticker(store: &gio::ListStore, id: &str) -> Option<StickerObject> {
    for index in 0..store.n_items() {
        let Some(object) = store
            .item(index)
            .and_then(|item| item.downcast::<StickerObject>().ok())
        else {
            continue;
        };
        if object.identity() == id {
            return Some(object);
        }
    }
    None
}

fn find_descendant<T>(root: &impl IsA<gtk4::Widget>) -> Option<T>
where
    T: IsA<gtk4::Widget>,
{
    let children = root.as_ref().observe_children();
    for index in 0..children.n_items() {
        let Some(child) = children
            .item(index)
            .and_then(|item| item.downcast::<gtk4::Widget>().ok())
        else {
            continue;
        };
        if let Ok(found) = child.clone().downcast::<T>() {
            return Some(found);
        }
        if let Some(found) = find_descendant::<T>(&child) {
            return Some(found);
        }
    }
    None
}

fn still_current(session: &Rc<RefCell<Session>>, epoch: u64) -> bool {
    let state = session.borrow();
    state.alive.get() && state.generation == epoch
}

fn show_empty(state: &Session, copy: EmptyCopy) {
    let (title, hint, action) = match copy {
        EmptyCopy::Library => (
            i18n::text(Key::LibraryEmpty),
            i18n::text(Key::LibraryEmptyHint),
            Some(&state.empty_add),
        ),
        EmptyCopy::Starred => (
            i18n::text(Key::StarredEmpty),
            i18n::text(Key::StarredEmptyHint),
            None,
        ),
        EmptyCopy::Collection => (
            i18n::text(Key::CollectionEmpty),
            i18n::text(Key::CollectionEmptyHint),
            None,
        ),
        EmptyCopy::Tag => (
            i18n::text(Key::TagEmpty),
            i18n::text(Key::TagEmptyHint),
            None,
        ),
        EmptyCopy::Trash => (
            i18n::text(Key::TrashEmpty),
            i18n::text(Key::TrashEmptyHint),
            None,
        ),
        EmptyCopy::NoMatches => (
            i18n::text(Key::LibraryNoMatches),
            i18n::text(Key::LibraryNoMatchesHint),
            Some(&state.empty_clear),
        ),
    };
    state.empty_page.set_title(title);
    state.empty_page.set_description(Some(hint));
    state.empty_page.set_child(action);
}

fn toast(session: &Rc<RefCell<Session>>, message: &str) {
    session.borrow().toasts.add_toast(adw::Toast::new(message));
}

fn show_error(session: &Rc<RefCell<Session>>, message: &str) {
    let state = session.borrow();
    let escaped = glib::markup_escape_text(message);
    state.error_page.set_description(Some(escaped.as_str()));
    state.stack.set_visible_child_name("error");
}

fn path_text(path: &std::path::Path) -> String {
    path.to_str().unwrap_or("").to_owned()
}

fn install_style() {
    use std::sync::Once;
    static STYLE: Once = Once::new();
    STYLE.call_once(|| {
        let Some(display) = gtk4::gdk::Display::default() else {
            return;
        };
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(
            ".sticker-tile {
                border-radius: 12px;
                transition: background-color 180ms ease;
                background-color: transparent;
            }
            .sticker-tile:hover {
                background-color: alpha(var(--window-fg-color), 0.08);
            }
            .sticker-badge {
                margin: 6px;
                padding: 2px 6px;
                border-radius: 999px;
                background-color: alpha(black, 0.65);
                color: white;
                font-size: 0.8em;
            }
            .import-veil {
                background-color: alpha(var(--window-bg-color), 0.86);
            }
            .import-tile {
                background-color: var(--card-bg-color);
                border-radius: 8px;
            }
            .relation-chip {
                padding: 6px 10px;
                border-radius: 999px;
                background-color: var(--card-bg-color);
            }
            .note-add {
                color: var(--accent-color);
            }",
        );
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
}
