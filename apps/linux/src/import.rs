use crate::i18n::{self, Key};
use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use memedock_core::{
    CoreError, ImportInput, ImportOptions, ImportOutcome, ImportStatus, Library, ResourceLimits,
};
use memedock_domain::identity::{CollectionId, StickerId};
use memedock_domain::version::{Generation, Revision};
use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const BATCH_LIMIT: usize = 200;
const PREVIEW_EDGE: u32 = 192;
const TILE_PX: i32 = 72;
const TILE_GAP: i32 = 8;
const PREVIEW_COLUMNS: i32 = 6;
const PREVIEW_VIEW_WIDTH: i32 = PREVIEW_COLUMNS * TILE_PX + (PREVIEW_COLUMNS - 1) * TILE_GAP;
/// One full row, the gap, and half of the next row. The clipped row is the scroll cue.
const PREVIEW_VIEW_HEIGHT: i32 = TILE_PX + TILE_GAP + TILE_PX / 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceClass {
    File,
    Unreadable,
}

pub fn classify_source(empty: bool, is_dir: bool) -> SourceClass {
    if empty || is_dir {
        SourceClass::Unreadable
    } else {
        SourceClass::File
    }
}

pub fn batch_too_large(count: usize) -> bool {
    count > BATCH_LIMIT
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewDecision {
    Import,
    Discard,
}

pub fn review_decision(confirmed: bool) -> ReviewDecision {
    if confirmed {
        ReviewDecision::Import
    } else {
        ReviewDecision::Discard
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counted {
    Created,
    Reused,
    RestoreRequired,
    Failed,
    Cancelled,
    Pending,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub created: usize,
    pub reused: usize,
    pub restore: usize,
    pub failed: usize,
    pub cancelled: usize,
}

pub fn summarize(items: &[Counted]) -> Report {
    let mut report = Report::default();
    for item in items {
        match item {
            Counted::Created => report.created += 1,
            Counted::Reused => report.reused += 1,
            Counted::RestoreRequired => report.restore += 1,
            Counted::Failed => report.failed += 1,
            Counted::Cancelled => report.cancelled += 1,
            Counted::Pending => {}
        }
    }
    report
}

fn finished_message(report: &Report, stopped: bool) -> String {
    if stopped {
        return i18n::import_stopped(report.created);
    }
    if report.failed > 0 {
        return i18n::import_done_partial(report.created, report.failed);
    }
    if report.created > 0 && report.reused > 0 {
        return i18n::import_done_reused(report.created, report.reused);
    }
    if report.created > 0 {
        return i18n::import_done(report.created);
    }
    if report.reused > 0 {
        return i18n::text(Key::ImportDoneExisting).to_owned();
    }
    i18n::text(Key::FailureUnknown).to_owned()
}

fn reject_path(path: &Path) -> Option<&'static str> {
    if classify_source(path.as_os_str().is_empty(), false) == SourceClass::Unreadable {
        return Some(i18n::text(Key::FailureIo));
    }
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Some(i18n::text(Key::DirectorySkipped)),
        Ok(_) => None,
        Err(_) => Some(i18n::text(Key::FailureIo)),
    }
}

enum Origin {
    Path(PathBuf),
    Bytes(Vec<u8>),
    Missing,
}

struct Item {
    name: String,
    origin: Origin,
    input: Option<ImportInput>,
    counted: Counted,
    message: String,
    sticker: Option<(StickerId, Generation, Revision)>,
}

pub struct Batch {
    pub busy: bool,
    cancel: bool,
    shutting_down: bool,
    review_accepted: bool,
    generation: u64,
    chosen: Option<CollectionId>,
    items: Vec<Item>,
    orphans: Vec<ImportInput>,
    task: Option<glib::JoinHandle<()>>,
    dialog: Option<adw::Dialog>,
    problems: Option<adw::Dialog>,
}

pub struct Shutdown {
    pub review: Option<adw::Dialog>,
    pub problems: Option<adw::Dialog>,
    pub task: Option<glib::JoinHandle<()>>,
}

impl Batch {
    pub fn new() -> Self {
        Self {
            busy: false,
            cancel: false,
            shutting_down: false,
            review_accepted: false,
            generation: 0,
            chosen: None,
            items: Vec::new(),
            orphans: Vec::new(),
            task: None,
            dialog: None,
            problems: None,
        }
    }

    pub fn begin_shutdown(&mut self) -> Shutdown {
        self.shutting_down = true;
        self.cancel = true;
        Shutdown {
            review: self.dialog.take(),
            problems: self.problems.take(),
            task: self.task.take(),
        }
    }

    pub fn take_inputs(&mut self) -> Vec<ImportInput> {
        let mut inputs = std::mem::take(&mut self.orphans);
        inputs.extend(self.items.iter_mut().filter_map(|item| item.input.take()));
        inputs
    }
}

impl Default for Batch {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Controls {
    pub batch: Rc<RefCell<Batch>>,
    pub library: Library,
    pub toasts: adw::ToastOverlay,
    pub alive: Rc<Cell<bool>>,
    pub parent: gtk4::Widget,
    pub progress: gtk4::Box,
    pub progress_label: gtk4::Label,
}

pub async fn discard_held(library: &Library, inputs: Vec<ImportInput>) {
    for input in inputs {
        if let Ok(task) = library.discard_import_input(input) {
            let _ = task.wait().await;
        }
    }
}

pub fn bind(
    header: &adw::HeaderBar,
    overlay: &gtk4::Overlay,
    veil: &gtk4::Box,
    controls: Controls,
    collection: Rc<Cell<Option<CollectionId>>>,
    empty_add: &gtk4::Button,
) {
    let controls = Rc::new(controls);
    let import_button = gtk4::Button::with_label(i18n::text(Key::LibraryAdd));
    let paste_button = gtk4::Button::with_label(i18n::text(Key::Paste));
    paste_button.set_tooltip_text(Some(i18n::text(Key::PasteHint)));
    header.pack_end(&paste_button);
    header.pack_end(&import_button);

    let controls_for_import = Rc::clone(&controls);
    let collection_for_import = Rc::clone(&collection);
    let overlay_for_import = overlay.clone();
    import_button.connect_clicked(move |_| {
        begin_picker(
            Rc::clone(&controls_for_import),
            &overlay_for_import,
            collection_for_import.get(),
        );
    });
    let controls_for_empty = Rc::clone(&controls);
    let collection_for_empty = Rc::clone(&collection);
    let overlay_for_empty = overlay.clone();
    empty_add.connect_clicked(move |_| {
        begin_picker(
            Rc::clone(&controls_for_empty),
            &overlay_for_empty,
            collection_for_empty.get(),
        );
    });
    let controls_for_paste = Rc::clone(&controls);
    let collection_for_paste = Rc::clone(&collection);
    paste_button.connect_clicked(move |_| {
        begin_clipboard(Rc::clone(&controls_for_paste), collection_for_paste.get());
    });

    let keys = gtk4::EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let controls_for_keys = Rc::clone(&controls);
    let collection_for_keys = Rc::clone(&collection);
    keys.connect_key_pressed(move |_, key, _, state| {
        let chord = state.contains(gtk4::gdk::ModifierType::CONTROL_MASK)
            && state.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
        if chord && (key == gtk4::gdk::Key::v || key == gtk4::gdk::Key::V) {
            begin_clipboard(Rc::clone(&controls_for_keys), collection_for_keys.get());
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    overlay.add_controller(keys);

    let drop_target = gtk4::DropTarget::new(
        gtk4::gdk::FileList::static_type(),
        gtk4::gdk::DragAction::COPY,
    );
    let veil_for_enter = veil.clone();
    drop_target.connect_enter(move |_, _, _| {
        veil_for_enter.set_visible(true);
        gtk4::gdk::DragAction::COPY
    });
    let veil_for_leave = veil.clone();
    drop_target.connect_leave(move |_| veil_for_leave.set_visible(false));
    let controls_for_drop = Rc::clone(&controls);
    let collection_for_drop = Rc::clone(&collection);
    let veil_for_drop = veil.clone();
    drop_target.connect_drop(move |_, value, _, _| {
        veil_for_drop.set_visible(false);
        let Ok(list) = value.get::<gtk4::gdk::FileList>() else {
            return false;
        };
        begin_external(
            Rc::clone(&controls_for_drop),
            list.files(),
            collection_for_drop.get(),
        );
        true
    });
    overlay.add_controller(drop_target);
}

pub fn wire_stop(progress: &gtk4::Box, batch: Rc<RefCell<Batch>>) {
    let mut child = progress.first_child();
    while let Some(widget) = child {
        if let Ok(button) = widget.clone().downcast::<gtk4::Button>() {
            button.connect_clicked(move |_| batch.borrow_mut().cancel = true);
            return;
        }
        child = widget.next_sibling();
    }
}

fn toast(controls: &Controls, message: &str) {
    controls.toasts.add_toast(adw::Toast::new(message));
}

fn stopped(controls: &Controls) -> bool {
    !controls.alive.get() || controls.batch.borrow().cancel || controls.batch.borrow().shutting_down
}

fn claim(controls: &Controls, collection: Option<CollectionId>) -> bool {
    let mut batch = controls.batch.borrow_mut();
    if batch.shutting_down {
        return false;
    }
    if batch.busy {
        drop(batch);
        toast(controls, i18n::text(Key::ErrorBatchBusy));
        return false;
    }
    let problems = batch.problems.take();
    drop(batch);
    if let Some(dialog) = problems {
        dialog.close();
    }
    let mut batch = controls.batch.borrow_mut();
    if batch.busy || batch.shutting_down {
        return false;
    }
    batch.busy = true;
    batch.cancel = false;
    batch.review_accepted = false;
    batch.generation = batch.generation.wrapping_add(1);
    batch.chosen = collection;
    batch.items.clear();
    true
}

fn spawn_task(controls: &Rc<Controls>, task: glib::JoinHandle<()>) {
    if let Some(previous) = controls.batch.borrow_mut().task.replace(task) {
        previous.abort();
    }
}

pub fn begin_external(
    controls: Rc<Controls>,
    files: Vec<gio::File>,
    collection: Option<CollectionId>,
) {
    if files.is_empty() {
        toast(&controls, i18n::text(Key::FailureMissing));
        return;
    }
    if batch_too_large(files.len()) {
        toast(&controls, i18n::text(Key::FailureBatch));
        return;
    }
    if !claim(&controls, collection) {
        return;
    }
    let tracked = Rc::clone(&controls);
    let task = glib::spawn_future_local(async move {
        review_files(tracked, files).await;
    });
    spawn_task(&controls, task);
}

fn begin_clipboard(controls: Rc<Controls>, collection: Option<CollectionId>) {
    if controls.batch.borrow().busy {
        toast(&controls, i18n::text(Key::ErrorBatchBusy));
        return;
    }
    let clipboard = controls.parent.clipboard();
    glib::spawn_future_local(async move {
        if controls.batch.borrow().shutting_down {
            return;
        }
        if clipboard
            .formats()
            .contains_type(gtk4::gdk::FileList::static_type())
        {
            let files = match clipboard
                .read_value_future(gtk4::gdk::FileList::static_type(), glib::Priority::DEFAULT)
                .await
            {
                Ok(value) => value
                    .get::<gtk4::gdk::FileList>()
                    .ok()
                    .map(|list| list.files()),
                Err(_) => None,
            };
            match files {
                Some(files) => begin_external(controls, files, collection),
                None => toast(&controls, i18n::text(Key::FailureIo)),
            }
            return;
        }
        match clipboard.read_texture_future().await {
            Ok(Some(texture)) => {
                let bytes = texture.save_to_png_bytes().to_vec();
                if !claim(&controls, collection) {
                    return;
                }
                let tracked = Rc::clone(&controls);
                let task = glib::spawn_future_local(async move {
                    review_bytes(tracked, bytes).await;
                });
                spawn_task(&controls, task);
            }
            _ => toast(&controls, i18n::text(Key::ClipboardEmpty)),
        }
    });
}

fn begin_picker(controls: Rc<Controls>, overlay: &gtk4::Overlay, collection: Option<CollectionId>) {
    if !claim(&controls, collection) {
        return;
    }
    let Some(window) = overlay
        .root()
        .and_then(|root| root.downcast::<gtk4::Window>().ok())
    else {
        controls.batch.borrow_mut().busy = false;
        return;
    };
    let dialog = gtk4::FileDialog::new();
    dialog.set_title(i18n::text(Key::ImportPhotos));
    dialog.set_accept_label(Some(i18n::text(Key::LibraryAdd)));
    let tracked = Rc::clone(&controls);
    let task = glib::spawn_future_local(async move {
        match dialog.open_multiple_future(Some(&window)).await {
            Ok(model) => {
                let files = (0..model.n_items())
                    .filter_map(|index| model.item(index).and_then(|item| item.downcast().ok()))
                    .collect::<Vec<gio::File>>();
                if batch_too_large(files.len()) {
                    toast(&tracked, i18n::text(Key::FailureBatch));
                    tracked.batch.borrow_mut().busy = false;
                    return;
                }
                import_directly(tracked, files).await;
            }
            Err(_) => tracked.batch.borrow_mut().busy = false,
        }
    });
    spawn_task(&controls, task);
}

async fn review_files(controls: Rc<Controls>, files: Vec<gio::File>) {
    let names = files.iter().map(file_name).collect::<Vec<_>>();
    let ui = open_review(&controls, &names);
    if !attach_collections(&controls, &ui).await || stopped(&controls) {
        abandon(&controls).await;
        return;
    }
    let mut ready = 0usize;
    let mut failed = 0usize;
    for (index, file) in files.iter().enumerate() {
        if stopped(&controls) {
            break;
        }
        let name = names[index].clone();
        let origin = Origin::Path(file.path().unwrap_or_default());
        match stage_file(&controls.library, file).await {
            Ok(input) => {
                let path = input.path().to_path_buf();
                remember_staged(&controls, name, origin, input);
                let preview = preview_file(&path).await;
                show_tile(&ui.tiles[index], preview, true);
                ready += 1;
            }
            Err(message) => {
                push_failed(&controls, name, origin, message);
                show_tile(&ui.tiles[index], None, false);
                failed += 1;
                ui.note.set_text(&i18n::import_unreadable(failed));
                ui.note.set_visible(true);
            }
        }
    }
    if stopped(&controls) {
        abandon(&controls).await;
        return;
    }
    finish_review(&ui, ready);
}

async fn review_bytes(controls: Rc<Controls>, bytes: Vec<u8>) {
    let name = "剪贴板图片.png".to_owned();
    let ui = open_review(&controls, std::slice::from_ref(&name));
    if !attach_collections(&controls, &ui).await || stopped(&controls) {
        abandon(&controls).await;
        return;
    }
    let mut ready = 0usize;
    match stage_bytes(&controls.library, &bytes).await {
        Ok(input) => {
            let path = input.path().to_path_buf();
            remember_staged(&controls, name, Origin::Bytes(bytes), input);
            let preview = preview_file(&path).await;
            show_tile(&ui.tiles[0], preview, true);
            ready = 1;
        }
        Err(message) => {
            push_failed(&controls, name, Origin::Bytes(bytes), message);
            show_tile(&ui.tiles[0], None, false);
            ui.note.set_text(&i18n::import_unreadable(1));
            ui.note.set_visible(true);
        }
    }
    if stopped(&controls) {
        abandon(&controls).await;
        return;
    }
    finish_review(&ui, ready);
}

struct ReviewTile {
    picture: gtk4::Picture,
    spinner: adw::Spinner,
    failed: gtk4::Image,
}

struct ReviewUi {
    tiles: Vec<ReviewTile>,
    note: gtk4::Label,
    add: gtk4::Button,
    collections: gtk4::Box,
}

fn open_review(controls: &Rc<Controls>, names: &[String]) -> ReviewUi {
    let dialog = adw::Dialog::new();
    dialog.set_title(&i18n::import_review_title(names.len()));
    dialog.set_content_width(520);
    dialog.set_follows_content_size(false);
    let generation = controls.batch.borrow().generation;
    let batch = Rc::clone(&controls.batch);
    let library = controls.library.clone();
    dialog.connect_closed(move |_| {
        let discard = {
            let state = batch.borrow();
            if state.generation != generation || state.shutting_down {
                return;
            }
            review_decision(state.review_accepted) == ReviewDecision::Discard
        };
        if !discard {
            return;
        }
        batch.borrow_mut().cancel = true;
        let inputs = batch.borrow_mut().take_inputs();
        let library = library.clone();
        let batch = Rc::clone(&batch);
        glib::spawn_future_local(async move {
            discard_held(&library, inputs).await;
            let mut state = batch.borrow_mut();
            if state.generation == generation {
                state.busy = false;
                state.task = None;
            }
        });
    });
    let hint = gtk4::Label::new(Some(i18n::text(Key::ImportReviewHint)));
    hint.set_wrap(true);
    hint.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    hint.set_xalign(0.0);
    hint.set_hexpand(true);
    hint.set_max_width_chars(28);
    let flow = gtk4::FlowBox::new();
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_min_children_per_line(PREVIEW_COLUMNS as u32);
    flow.set_max_children_per_line(PREVIEW_COLUMNS as u32);
    flow.set_column_spacing(TILE_GAP as u32);
    flow.set_row_spacing(TILE_GAP as u32);
    flow.set_hexpand(true);
    flow.set_halign(gtk4::Align::Fill);
    let mut tiles = Vec::with_capacity(names.len());
    for name in names {
        let tile = placeholder_tile(name);
        flow.insert(&tile.overlay, -1);
        tiles.push(ReviewTile {
            picture: tile.picture,
            spinner: tile.spinner,
            failed: tile.failed,
        });
    }
    let pictures = gtk4::ScrolledWindow::new();
    pictures.set_child(Some(&flow));
    pictures.set_min_content_width(PREVIEW_VIEW_WIDTH);
    // Natural height stays at the minimum, so the viewport must be set here.
    // One row hides every following tile; one and a half rows leaves the next row cut off.
    let view_height = if names.len() > PREVIEW_COLUMNS as usize {
        PREVIEW_VIEW_HEIGHT
    } else {
        TILE_PX
    };
    pictures.set_min_content_height(view_height);
    pictures.set_max_content_height(view_height);
    pictures.set_propagate_natural_width(false);
    pictures.set_propagate_natural_height(false);
    pictures.set_hexpand(true);
    pictures.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    let note = gtk4::Label::new(None);
    note.add_css_class("error");
    note.set_wrap(true);
    note.set_xalign(0.0);
    note.set_halign(gtk4::Align::Start);
    note.set_hexpand(true);
    note.set_visible(false);
    let collections = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    collections.set_hexpand(true);
    let add = gtk4::Button::with_label(i18n::text(Key::ImportReadingFiles));
    add.add_css_class("suggested-action");
    add.set_hexpand(true);
    add.set_sensitive(false);
    let cancel = gtk4::Button::with_label(i18n::text(Key::ImportDiscard));
    cancel.set_hexpand(true);
    let controls_for_add = Rc::clone(controls);
    add.connect_clicked(move |_| {
        controls_for_add.batch.borrow_mut().review_accepted = true;
        if let Some(dialog) = controls_for_add.batch.borrow().dialog.clone() {
            dialog.close();
        }
        let tracked = Rc::clone(&controls_for_add);
        let task = glib::spawn_future_local(async move {
            commit_staged(tracked).await;
        });
        spawn_task(&controls_for_add, task);
    });
    let dialog_for_cancel = dialog.clone();
    cancel.connect_clicked(move |_| {
        dialog_for_cancel.close();
    });
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    column.set_hexpand(true);
    column.append(&hint);
    column.append(&pictures);
    column.append(&note);
    let heading = gtk4::Label::new(Some(i18n::text(Key::ImportCollection)));
    heading.set_halign(gtk4::Align::Start);
    column.append(&heading);
    column.append(&collections);
    column.append(&add);
    column.append(&cancel);
    dialog.set_child(Some(&column));
    dialog.present(Some(&controls.parent));
    controls.batch.borrow_mut().dialog = Some(dialog);
    ReviewUi {
        tiles,
        note,
        add,
        collections,
    }
}

struct Placeholder {
    overlay: gtk4::Overlay,
    picture: gtk4::Picture,
    spinner: adw::Spinner,
    failed: gtk4::Image,
}

fn placeholder_tile(name: &str) -> Placeholder {
    let picture = gtk4::Picture::new();
    picture.set_size_request(TILE_PX, TILE_PX);
    picture.set_content_fit(gtk4::ContentFit::Cover);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(18, 18);
    spinner.set_halign(gtk4::Align::Center);
    spinner.set_valign(gtk4::Align::Center);
    let failed = gtk4::Image::from_icon_name("dialog-error-symbolic");
    failed.set_visible(false);
    failed.set_halign(gtk4::Align::End);
    failed.set_valign(gtk4::Align::End);
    failed.set_margin_end(4);
    failed.set_margin_bottom(4);
    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("import-tile");
    overlay.set_size_request(TILE_PX, TILE_PX);
    overlay.set_halign(gtk4::Align::Fill);
    overlay.set_valign(gtk4::Align::Start);
    overlay.set_vexpand(false);
    overlay.set_child(Some(&picture));
    overlay.add_overlay(&spinner);
    overlay.add_overlay(&failed);
    overlay.set_tooltip_text(Some(name));
    Placeholder {
        overlay,
        picture,
        spinner,
        failed,
    }
}

fn show_tile(tile: &ReviewTile, preview: Option<gtk4::gdk::MemoryTexture>, readable: bool) {
    tile.spinner.set_visible(false);
    if let Some(texture) = preview {
        tile.picture.set_paintable(Some(&texture));
    }
    tile.failed.set_visible(!readable);
}

fn finish_review(ui: &ReviewUi, ready: usize) {
    ui.add.set_label(&i18n::import_add_count(ready));
    ui.add.set_sensitive(ready > 0);
}

async fn attach_collections(controls: &Rc<Controls>, ui: &ReviewUi) -> bool {
    let chosen = controls.batch.borrow().chosen;
    let collections = match controls.library.collections(false) {
        Ok(task) => match task.wait().await {
            Ok(collections) => collections,
            Err(error) => {
                toast(controls, i18n::core(error.code()));
                Vec::new()
            }
        },
        Err(error) => {
            toast(controls, i18n::core(error.code()));
            Vec::new()
        }
    };
    if stopped(controls) || controls.batch.borrow().dialog.is_none() {
        return false;
    }
    ui.collections.append(&collection_row(
        Rc::clone(&controls.batch),
        &collections,
        chosen,
    ));
    true
}

fn remember_staged(controls: &Controls, name: String, origin: Origin, input: ImportInput) {
    controls.batch.borrow_mut().items.push(Item {
        name,
        origin,
        input: Some(input),
        counted: Counted::Pending,
        message: String::new(),
        sticker: None,
    });
}

fn push_failed(controls: &Controls, name: String, origin: Origin, message: &str) {
    controls.batch.borrow_mut().items.push(Item {
        name,
        origin,
        input: None,
        counted: Counted::Failed,
        message: message.to_owned(),
        sticker: None,
    });
}

fn collection_row(
    batch: Rc<RefCell<Batch>>,
    collections: &[memedock_domain::collection::Collection],
    chosen: Option<CollectionId>,
) -> gtk4::ScrolledWindow {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let none = gtk4::ToggleButton::with_label(i18n::text(Key::NoCollection));
    none.set_active(chosen.is_none());
    let choices = collections
        .iter()
        .map(|collection| {
            let button = gtk4::ToggleButton::with_label(collection.name().as_str());
            button.set_group(Some(&none));
            button.set_active(chosen == Some(collection.id()));
            (button, collection.id())
        })
        .collect::<Vec<_>>();
    let batch_for_none = Rc::clone(&batch);
    none.connect_toggled(move |button| {
        if button.is_active() {
            batch_for_none.borrow_mut().chosen = None;
        }
    });
    row.append(&none);
    for (button, id) in choices {
        let batch_for_button = Rc::clone(&batch);
        button.connect_toggled(move |button| {
            if button.is_active() {
                batch_for_button.borrow_mut().chosen = Some(id);
            }
        });
        row.append(&button);
    }
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&row));
    scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    scroll.set_hexpand(true);
    scroll.set_propagate_natural_width(false);
    scroll
}

async fn commit_staged(controls: Rc<Controls>) {
    let total = controls
        .batch
        .borrow()
        .items
        .iter()
        .filter(|item| item.input.is_some())
        .count();
    show_progress(&controls, 0, total);
    let len = controls.batch.borrow().items.len();
    let mut done = 0;
    for index in 0..len {
        if stopped(&controls) {
            cancel_remaining(&controls, index).await;
            break;
        }
        let job = {
            let mut batch = controls.batch.borrow_mut();
            let chosen = batch.chosen;
            let item = &mut batch.items[index];
            item.input
                .take()
                .map(|input| (input, item.name.clone(), chosen))
        };
        let Some((input, name, collection)) = job else {
            continue;
        };
        done += 1;
        show_progress(&controls, done, total);
        let saved = import_staged(&controls, input, &name, collection).await;
        record(&controls, index, saved);
    }
    finish(&controls);
}

async fn import_directly(controls: Rc<Controls>, files: Vec<gio::File>) {
    let total = files.len();
    show_progress(&controls, 0, total);
    for (index, file) in files.iter().enumerate() {
        if stopped(&controls) {
            for skipped in files.iter().skip(index) {
                push_failed(
                    &controls,
                    file_name(skipped),
                    Origin::Path(skipped.path().unwrap_or_default()),
                    i18n::text(Key::ImportCancelled),
                );
                if let Some(item) = controls.batch.borrow_mut().items.last_mut() {
                    item.counted = Counted::Cancelled;
                }
            }
            break;
        }
        show_progress(&controls, index, total);
        let name = file_name(file);
        let origin = Origin::Path(file.path().unwrap_or_default());
        if let Some(path) = file.path()
            && let Some(message) = reject_message(&path).await
        {
            push_failed(&controls, name, origin, message);
            continue;
        }
        let chosen = controls.batch.borrow().chosen;
        let saved = if let Some(path) = file.path() {
            import_path(&controls, path, &name).await
        } else {
            match stage_file(&controls.library, file).await {
                Ok(input) => import_staged(&controls, input, &name, chosen).await,
                Err(message) => {
                    push_failed(&controls, name, origin, message);
                    continue;
                }
            }
        };
        let index = controls.batch.borrow().items.len();
        controls.batch.borrow_mut().items.push(Item {
            name,
            origin,
            input: None,
            counted: Counted::Pending,
            message: String::new(),
            sticker: None,
        });
        record(&controls, index, saved);
    }
    finish(&controls);
}

async fn cancel_remaining(controls: &Controls, from: usize) {
    let inputs = {
        let mut batch = controls.batch.borrow_mut();
        let mut inputs = Vec::new();
        for item in batch.items.iter_mut().skip(from) {
            if item.counted == Counted::Pending {
                item.counted = Counted::Cancelled;
                item.message = i18n::text(Key::ImportCancelled).to_owned();
            }
            if let Some(input) = item.input.take() {
                inputs.push(input);
            }
        }
        inputs
    };
    discard_held(&controls.library, inputs).await;
}

fn record(controls: &Controls, index: usize, saved: Result<ImportOutcome, CoreError>) {
    let mut batch = controls.batch.borrow_mut();
    let Some(item) = batch.items.get_mut(index) else {
        return;
    };
    match saved {
        Ok(outcome) => {
            item.sticker = Some((
                outcome.sticker.id(),
                outcome.sticker.lifecycle().generation(),
                outcome.sticker.lifecycle().revision(),
            ));
            match outcome.status {
                ImportStatus::Created => item.counted = Counted::Created,
                ImportStatus::Reused => item.counted = Counted::Reused,
                ImportStatus::RestoreRequired => {
                    item.counted = Counted::RestoreRequired;
                    item.message = i18n::text(Key::ImportRestoreRequired).to_owned();
                }
            }
        }
        Err(error) => {
            item.counted = Counted::Failed;
            item.message = explain(&error);
        }
    }
}

fn finish(controls: &Rc<Controls>) {
    controls.progress.set_visible(false);
    let stopped = controls.batch.borrow().cancel;
    let counted = controls
        .batch
        .borrow()
        .items
        .iter()
        .map(|item| item.counted)
        .collect::<Vec<_>>();
    toast(controls, &finished_message(&summarize(&counted), stopped));
    let unresolved = counted.iter().any(|item| {
        matches!(
            item,
            Counted::Failed | Counted::Cancelled | Counted::RestoreRequired
        )
    });
    {
        let mut batch = controls.batch.borrow_mut();
        batch.busy = false;
        batch.task = None;
    }
    if unresolved && !controls.batch.borrow().shutting_down {
        present_problems(Rc::clone(controls));
    }
}

fn present_problems(controls: Rc<Controls>) {
    let rows = {
        let batch = controls.batch.borrow();
        batch
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                matches!(
                    item.counted,
                    Counted::Failed | Counted::Cancelled | Counted::RestoreRequired
                )
            })
            .map(|(index, item)| (index, item.name.clone(), item.message.clone(), item.counted))
            .collect::<Vec<_>>()
    };
    let added = controls
        .batch
        .borrow()
        .items
        .iter()
        .filter(|item| matches!(item.counted, Counted::Created | Counted::Reused))
        .count();
    let dialog = adw::Dialog::new();
    dialog.set_title(&i18n::import_problems_title(rows.len()));
    dialog.set_content_width(480);
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    column.set_margin_top(12);
    column.set_margin_bottom(12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    if added > 0 {
        let hint = gtk4::Label::new(Some(&i18n::import_problems_hint(added)));
        hint.set_halign(gtk4::Align::Start);
        hint.set_wrap(true);
        column.append(&hint);
    }
    for (index, name, message, counted) in rows {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let text = if message.is_empty() {
            name
        } else {
            format!("{name}\n{message}")
        };
        let label = gtk4::Label::new(Some(&text));
        label.set_halign(gtk4::Align::Start);
        label.set_hexpand(true);
        label.set_wrap(true);
        row.append(&label);
        if counted == Counted::RestoreRequired {
            let controls_for_restore = Rc::clone(&controls);
            let restore = gtk4::Button::with_label(i18n::text(Key::Restore));
            restore.connect_clicked(move |_| restore_item(Rc::clone(&controls_for_restore), index));
            row.append(&restore);
        }
        column.append(&row);
    }
    let retryable = controls.batch.borrow().items.iter().any(|item| {
        matches!(item.counted, Counted::Failed | Counted::Cancelled)
            && !matches!(item.origin, Origin::Missing)
    });
    if retryable {
        let controls_for_retry = Rc::clone(&controls);
        let retry = gtk4::Button::with_label(i18n::text(Key::ImportRetryFailed));
        retry.add_css_class("suggested-action");
        retry.connect_clicked(move |_| {
            if let Some(dialog) = controls_for_retry.batch.borrow_mut().problems.take() {
                dialog.close();
            }
            let tracked = Rc::clone(&controls_for_retry);
            let task = glib::spawn_future_local(async move {
                retry_failed(tracked).await;
            });
            spawn_task(&controls_for_retry, task);
        });
        column.append(&retry);
    }
    let dialog_for_done = dialog.clone();
    let done = gtk4::Button::with_label(i18n::text(Key::Done));
    done.connect_clicked(move |_| {
        dialog_for_done.close();
    });
    column.append(&done);
    dialog.set_child(Some(&column));
    dialog.present(Some(&controls.parent));
    controls.batch.borrow_mut().problems = Some(dialog);
}

fn restore_item(controls: Rc<Controls>, index: usize) {
    let Some((id, generation, revision)) = controls
        .batch
        .borrow()
        .items
        .get(index)
        .and_then(|item| item.sticker)
    else {
        return;
    };
    let parent = controls.parent.clone();
    confirm(
        &parent,
        i18n::text(Key::RestoreStickerTitle),
        i18n::text(Key::RestoreHint),
        i18n::text(Key::Restore),
        move || {
            let controls = Rc::clone(&controls);
            glib::spawn_future_local(async move {
                let saved = match controls.library.restore_sticker(id, generation, revision) {
                    Ok(task) => task.wait().await,
                    Err(error) => Err(error),
                };
                match saved {
                    Ok(_) => {
                        if let Some(item) = controls.batch.borrow_mut().items.get_mut(index) {
                            item.counted = Counted::Created;
                            item.message = i18n::text(Key::Restored).to_owned();
                        }
                        toast(&controls, i18n::text(Key::Restored));
                    }
                    Err(error) => toast(&controls, &explain(&error)),
                }
            });
        },
    );
}

async fn retry_failed(controls: Rc<Controls>) {
    if !controls.batch.borrow().busy {
        controls.batch.borrow_mut().busy = true;
        controls.batch.borrow_mut().cancel = false;
    }
    let indexes = controls
        .batch
        .borrow()
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            matches!(item.counted, Counted::Failed | Counted::Cancelled)
                && !matches!(item.origin, Origin::Missing)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let total = indexes.len();
    show_progress(&controls, 0, total);
    for (done, index) in indexes.into_iter().enumerate() {
        if stopped(&controls) {
            break;
        }
        show_progress(&controls, done, total);
        let origin = {
            let mut batch = controls.batch.borrow_mut();
            std::mem::replace(&mut batch.items[index].origin, Origin::Missing)
        };
        let name = controls.batch.borrow().items[index].name.clone();
        let staged = match &origin {
            Origin::Path(path) => stage_file(&controls.library, &gio::File::for_path(path)).await,
            Origin::Bytes(bytes) => stage_bytes(&controls.library, bytes).await,
            Origin::Missing => Err(i18n::text(Key::FailureIo)),
        };
        controls.batch.borrow_mut().items[index].origin = origin;
        match staged {
            Ok(input) => {
                let collection = controls.batch.borrow().chosen;
                let saved = import_staged(&controls, input, &name, collection).await;
                record(&controls, index, saved);
            }
            Err(message) => {
                let mut batch = controls.batch.borrow_mut();
                batch.items[index].counted = Counted::Failed;
                batch.items[index].message = message.to_owned();
            }
        }
    }
    finish(&controls);
}

async fn abandon(controls: &Rc<Controls>) {
    controls.progress.set_visible(false);
    let inputs = controls.batch.borrow_mut().take_inputs();
    discard_held(&controls.library, inputs).await;
    let dialog = controls.batch.borrow_mut().dialog.take();
    controls.batch.borrow_mut().busy = false;
    controls.batch.borrow_mut().task = None;
    if let Some(dialog) = dialog {
        dialog.close();
    }
}

fn show_progress(controls: &Controls, done: usize, total: usize) {
    controls
        .progress_label
        .set_text(&i18n::import_running_progress(done, total));
    controls.progress.set_visible(true);
}

async fn import_path(
    controls: &Controls,
    path: PathBuf,
    name: &str,
) -> Result<ImportOutcome, CoreError> {
    let options = ImportOptions {
        original_name: name.to_owned(),
        title: None,
        collection: controls.batch.borrow().chosen,
    };
    match controls.library.import_file(path, options) {
        Ok(task) => task.wait().await,
        Err(error) => Err(error),
    }
}

async fn import_staged(
    controls: &Controls,
    input: ImportInput,
    name: &str,
    collection: Option<CollectionId>,
) -> Result<ImportOutcome, CoreError> {
    let options = ImportOptions {
        original_name: name.to_owned(),
        title: None,
        collection,
    };
    match controls.library.import_staged(input, options) {
        Ok(task) => task.wait().await,
        Err(error) => Err(error),
    }
}

async fn stage_file(library: &Library, file: &gio::File) -> Result<ImportInput, &'static str> {
    if let Some(path) = file.path()
        && let Some(message) = reject_message(&path).await
    {
        return Err(message);
    }
    let input = create_slot(library).await?;
    let destination = input.path().to_path_buf();
    let limit = file_limit();
    let copied = if let Some(path) = file.path() {
        gio::spawn_blocking(move || copy_path(&path, &destination, limit)).await
    } else {
        let file = file.clone();
        gio::spawn_blocking(move || copy_gio(&file, &destination, limit)).await
    };
    let copied = match copied {
        Ok(result) => result,
        Err(_) => Err(i18n::text(Key::FailureIo)),
    };
    if let Err(message) = copied {
        discard_held(library, vec![input]).await;
        return Err(message);
    }
    Ok(input)
}

async fn stage_bytes(library: &Library, bytes: &[u8]) -> Result<ImportInput, &'static str> {
    if bytes.len() as u64 > file_limit() {
        return Err(i18n::text(Key::FailureLimit));
    }
    let input = create_slot(library).await?;
    let destination = input.path().to_path_buf();
    let bytes = bytes.to_vec();
    let copied = gio::spawn_blocking(move || write_bytes(&destination, &bytes)).await;
    let copied = match copied {
        Ok(result) => result,
        Err(_) => Err(i18n::text(Key::FailureIo)),
    };
    if let Err(message) = copied {
        discard_held(library, vec![input]).await;
        return Err(message);
    }
    Ok(input)
}

async fn create_slot(library: &Library) -> Result<ImportInput, &'static str> {
    match library.create_import_input() {
        Ok(task) => task.wait().await.map_err(|_| i18n::text(Key::FailureIo)),
        Err(_) => Err(i18n::text(Key::FailureIo)),
    }
}

async fn reject_message(path: &Path) -> Option<&'static str> {
    let path = path.to_path_buf();
    gio::spawn_blocking(move || reject_path(&path))
        .await
        .ok()
        .flatten()
}

fn copy_path(source: &Path, destination: &Path, limit: u64) -> Result<(), &'static str> {
    if let Some(message) = reject_path(source) {
        return Err(message);
    }
    let reader = std::fs::File::open(source).map_err(|_| i18n::text(Key::FailureIo))?;
    copy_reader(reader, destination, limit)
}

fn copy_gio(file: &gio::File, destination: &Path, limit: u64) -> Result<(), &'static str> {
    let stream = file
        .read(None::<&gio::Cancellable>)
        .map_err(|_| i18n::text(Key::FailureIo))?;
    copy_reader(stream.into_read(), destination, limit)
}

fn write_bytes(destination: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    copy_reader(std::io::Cursor::new(bytes), destination, file_limit())
}

fn copy_reader(mut reader: impl Read, destination: &Path, limit: u64) -> Result<(), &'static str> {
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .open(destination)
        .map_err(|_| i18n::text(Key::FailureIo))?;
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| i18n::text(Key::FailureIo))?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        if total > limit {
            return Err(i18n::text(Key::FailureLimit));
        }
        output
            .write_all(&buffer[..count])
            .map_err(|_| i18n::text(Key::FailureIo))?;
    }
    output.sync_all().map_err(|_| i18n::text(Key::FailureIo))?;
    Ok(())
}

fn file_limit() -> u64 {
    ResourceLimits::default().max_file_bytes
}

async fn preview_file(path: &Path) -> Option<gtk4::gdk::MemoryTexture> {
    let path = path.to_path_buf();
    let decoded = gio::spawn_blocking(move || decode_preview(&path))
        .await
        .ok()?;
    let (width, height, bytes) = decoded?;
    texture_from_rgba(width, height, bytes)
}

fn decode_preview(path: &Path) -> Option<(i32, i32, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = image::ImageReader::new(std::io::BufReader::new(file));
    reader = reader.with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(ResourceLimits::default().max_decode_bytes);
    reader.limits(limits);
    let image = reader.decode().ok()?.thumbnail(PREVIEW_EDGE, PREVIEW_EDGE);
    let rgba = image.to_rgba8();
    let width = i32::try_from(rgba.width()).ok()?;
    let height = i32::try_from(rgba.height()).ok()?;
    Some((width, height, rgba.into_raw()))
}

fn texture_from_rgba(width: i32, height: i32, bytes: Vec<u8>) -> Option<gtk4::gdk::MemoryTexture> {
    let stride = usize::try_from(width).ok()?.checked_mul(4)?;
    let rows = usize::try_from(height).ok()?;
    if stride == 0 || bytes.len() != stride.checked_mul(rows)? {
        return None;
    }
    Some(gtk4::gdk::MemoryTexture::new(
        width,
        height,
        gtk4::gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(bytes),
        stride,
    ))
}

fn explain(error: &CoreError) -> String {
    i18n::core(error.code()).to_owned()
}

fn file_name(file: &gio::File) -> String {
    file.basename()
        .and_then(|name| name.to_str().map(str::to_owned))
        .unwrap_or_else(|| i18n::text(Key::UntitledImage).to_owned())
}

fn confirm(
    parent: &gtk4::Widget,
    heading: &str,
    body: &str,
    accept: &str,
    then: impl Fn() + 'static,
) {
    let dialog = adw::AlertDialog::new(Some(heading), Some(body));
    dialog.add_response("cancel", i18n::text(Key::Cancel));
    dialog.add_response("accept", accept);
    dialog.set_close_response("cancel");
    dialog.set_default_response(Some("cancel"));
    dialog.connect_response(None, move |_, response| {
        if response == "accept" {
            then();
        }
    });
    dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use super::{
        BATCH_LIMIT, Counted, ReviewDecision, SourceClass, batch_too_large, classify_source,
        review_decision, summarize,
    };

    #[test]
    fn more_than_two_hundred_files_are_rejected() {
        assert!(!batch_too_large(0));
        assert!(!batch_too_large(BATCH_LIMIT));
        assert!(batch_too_large(BATCH_LIMIT + 1));
    }

    #[test]
    fn directories_and_empty_paths_are_unreadable() {
        assert_eq!(classify_source(true, false), SourceClass::Unreadable);
        assert_eq!(classify_source(false, true), SourceClass::Unreadable);
        assert_eq!(classify_source(false, false), SourceClass::File);
    }

    #[test]
    fn closing_the_review_discards_staging() {
        assert_eq!(review_decision(false), ReviewDecision::Discard);
        assert_eq!(review_decision(true), ReviewDecision::Import);
    }

    #[test]
    fn summary_counts_each_outcome() {
        let report = summarize(&[
            Counted::Created,
            Counted::Created,
            Counted::Reused,
            Counted::RestoreRequired,
            Counted::Failed,
            Counted::Cancelled,
            Counted::Pending,
        ]);
        assert_eq!(report.created, 2);
        assert_eq!(report.reused, 1);
        assert_eq!(report.restore, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.cancelled, 1);
    }
}
