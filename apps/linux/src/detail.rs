use crate::actions::{self, Host};
use crate::i18n::{self, Key};
use crate::library_view::sticker::StickerObject;
use crate::output::{self, CopyKind, Request};
use crate::playback::{Clip, Event, PlayFault, Playback};
use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use memedock_core::{Library, StickerDetail};
use memedock_domain::asset::ImageFormat;
use memedock_domain::change::{FieldPatch, StickerPatch};
use memedock_domain::export::ExportPreset;
use memedock_domain::identity::{CollectionId, StickerId, TagId};
use memedock_domain::tag::Name;
use memedock_domain::version::{Generation, Revision};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

pub type DetailRefresh = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

pub struct DetailLinks {
    pub playback: Rc<RefCell<Option<Playback>>>,
    pub refresh: DetailRefresh,
    pub sync: DetailRefresh,
}

#[derive(Clone)]
pub struct DetailChrome {
    pub title: gtk4::Label,
    pub star: gtk4::Button,
    star_handler: Rc<RefCell<Option<glib::SignalHandlerId>>>,
}

impl DetailChrome {
    pub fn new(title: gtk4::Label, star: gtk4::Button) -> Self {
        Self {
            title,
            star,
            star_handler: Rc::new(RefCell::new(None)),
        }
    }
}

#[derive(Clone)]
struct Relation<T> {
    id: T,
    generation: Generation,
    name: String,
    checked: bool,
}

struct Model {
    id: StickerId,
    generation: Generation,
    revision: Revision,
    deleted: bool,
    title: String,
    note: String,
    starred: bool,
    collections: Vec<Relation<CollectionId>>,
    tags: Vec<Relation<TagId>>,
    busy: bool,
    filling: bool,
    text_dirty: bool,
    relations_dirty: bool,
    pending_collection: Option<CollectionId>,
    pending_tag: Option<TagId>,
    epoch: u64,
}

struct Surface {
    toasts: adw::ToastOverlay,
    title_label: gtk4::Label,
    title_entry: gtk4::Entry,
    note_label: gtk4::Label,
    note_entry: gtk4::Entry,
    note_add: gtk4::Button,
    star: gtk4::Button,
    page: adw::NavigationPage,
    chips: gtk4::FlowBox,
    organize: gtk4::Button,
    editor: gtk4::Box,
    deleted_hint: gtk4::Label,
    header_title: gtk4::Label,
}

pub fn open(
    navigation: &adw::NavigationView,
    library: Library,
    object: &StickerObject,
    links: &DetailLinks,
    chrome: &DetailChrome,
    host: &Host,
) {
    let Ok(id) = object.identity().parse::<StickerId>() else {
        return;
    };
    host.open_id.set(Some(actions::OpenTarget {
        id,
        animated: object.animated(),
    }));
    let page = adw::NavigationPage::new(&gtk4::Spinner::new(), i18n::text(Key::TabStickers));
    page.set_tag(Some("detail"));
    let chrome = chrome.clone();
    let title_for_page = chrome.title.clone();
    page.connect_title_notify(move |page| {
        title_for_page.set_text(&page.title());
    });
    let links = links.clone_slots();
    let host = host.clone();
    let page_for_load = page.clone();
    let thumb = object.thumbnail();
    let thumb = if thumb.is_empty() {
        None
    } else {
        Some(PathBuf::from(thumb))
    };
    glib::spawn_future_local(async move {
        let loaded = match library.sticker_detail(id) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        let detail = match loaded {
            Ok(detail) => detail,
            Err(error) => {
                let status = adw::StatusPage::new();
                i18n::bind(&status, "title", Key::DetailLoadTitle);
                status.set_description(Some(i18n::core(error.code())));
                page_for_load.set_child(Some(&status));
                return;
            }
        };
        present(page_for_load, library, detail, links, thumb, chrome, host);
    });
    navigation.push(&page);
}

impl DetailLinks {
    fn clone_slots(&self) -> Self {
        Self {
            playback: Rc::clone(&self.playback),
            refresh: Rc::clone(&self.refresh),
            sync: Rc::clone(&self.sync),
        }
    }
}

fn present(
    page: adw::NavigationPage,
    library: Library,
    detail: StickerDetail,
    links: DetailLinks,
    thumb: Option<PathBuf>,
    chrome: DetailChrome,
    host: Host,
) {
    stop_playback(&links);
    let sticker = &detail.sticker;
    let model = Rc::new(RefCell::new(Model {
        id: sticker.id(),
        generation: sticker.lifecycle().generation(),
        revision: sticker.lifecycle().revision(),
        deleted: !sticker.lifecycle().is_active(),
        title: sticker.title().to_owned(),
        note: sticker.note().to_owned(),
        starred: sticker.starred(),
        collections: Vec::new(),
        tags: Vec::new(),
        busy: false,
        filling: false,
        text_dirty: false,
        relations_dirty: false,
        pending_collection: None,
        pending_tag: None,
        epoch: 0,
    }));
    let removed = !sticker.lifecycle().is_active();
    let width = detail.asset.width();
    let height = detail.asset.height();
    let bytes = u64::try_from(detail.asset.byte_size().get()).unwrap_or(0);
    let format_name = format_label(detail.asset.format());
    let original_name = sticker.original_name().to_owned();
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(24);
    column.set_margin_start(24);
    column.set_margin_end(24);
    let picture = gtk4::Picture::new();
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_can_shrink(true);
    picture.set_size_request(280, 280);
    column.append(&picture);
    let status = gtk4::Label::new(None);
    status.set_wrap(true);
    status.set_halign(gtk4::Align::Start);
    status.set_visible(false);
    column.append(&status);
    if detail.asset.format() == ImageFormat::Png && detail.asset.animated() {
        i18n::bind(&status, "label", Key::ApngFirstFrame);
        status.set_visible(true);
    }
    let title_label = gtk4::Label::new(Some(sticker.title()));
    title_label.add_css_class("title-1");
    title_label.set_halign(gtk4::Align::Start);
    title_label.set_hexpand(true);
    title_label.set_wrap(true);
    title_label.set_xalign(0.0);
    let title_entry = gtk4::Entry::new();
    title_entry.set_text(sticker.title());
    i18n::bind(&title_entry, "placeholder-text", Key::Name);
    title_entry.set_hexpand(true);
    title_entry.set_visible(false);
    let play = gtk4::Button::from_icon_name("media-playback-pause-symbolic");
    i18n::bind(&play, "tooltip-text", Key::PauseAnimation);
    play.set_visible(false);
    let title_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    title_row.append(&title_label);
    title_row.append(&title_entry);
    title_row.append(&play);
    column.append(&title_row);
    let note_label = gtk4::Label::new(Some(sticker.note()));
    note_label.set_halign(gtk4::Align::Start);
    note_label.set_wrap(true);
    note_label.set_xalign(0.0);
    note_label.add_css_class("body");
    note_label.set_visible(!sticker.note().is_empty());
    let note_entry = gtk4::Entry::new();
    note_entry.set_text(sticker.note());
    i18n::bind(&note_entry, "placeholder-text", Key::AddNote);
    note_entry.set_visible(false);
    let note_add = i18n::button(Key::AddNote);
    note_add.add_css_class("flat");
    note_add.add_css_class("note-add");
    note_add.set_halign(gtk4::Align::Start);
    note_add.set_visible(sticker.note().is_empty() && !removed);
    column.append(&note_label);
    column.append(&note_entry);
    column.append(&note_add);
    let deleted_hint = i18n::label(Key::RestoreHint);
    deleted_hint.set_wrap(true);
    deleted_hint.set_halign(gtk4::Align::Start);
    deleted_hint.set_xalign(0.0);
    deleted_hint.set_visible(removed);
    column.append(&deleted_hint);
    let chips = gtk4::FlowBox::new();
    chips.set_selection_mode(gtk4::SelectionMode::None);
    chips.set_column_spacing(8);
    chips.set_row_spacing(8);
    chips.set_visible(!removed);
    column.append(&chips);
    let organize = i18n::button(Key::OrganizeHint);
    organize.set_halign(gtk4::Align::Start);
    organize.set_visible(!removed);
    column.append(&organize);
    let editor = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let file_group = adw::PreferencesGroup::new();
    i18n::bind(&file_group, "title", Key::DetailFileTitle);
    let size_row = adw::ActionRow::new();
    i18n::bind(&size_row, "title", Key::DetailSize);
    size_row.set_subtitle(&i18n::detail_dimensions(width, height));
    let bytes_row = adw::ActionRow::new();
    i18n::bind(&bytes_row, "title", Key::DetailBytes);
    bytes_row.set_subtitle(&i18n::file_size(bytes));
    let format_row = adw::ActionRow::new();
    i18n::bind(&format_row, "title", Key::DetailFormat);
    format_row.set_subtitle(&format_name);
    let name_row = adw::ActionRow::new();
    i18n::bind(&name_row, "title", Key::DetailOriginalName);
    name_row.set_subtitle(&original_name);
    file_group.add(&size_row);
    file_group.add(&bytes_row);
    file_group.add(&format_row);
    file_group.add(&name_row);
    column.append(&file_group);
    let action = gtk4::Button::with_label(if removed {
        i18n::text(Key::Restore)
    } else {
        i18n::text(Key::Delete)
    });
    if !removed {
        action.add_css_class("destructive-action");
    }
    action.set_halign(gtk4::Align::Start);
    column.append(&action);
    let motion = detail.asset.animated();
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&column));
    scroll.set_vexpand(true);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    body.append(&scroll);
    let toasts = adw::ToastOverlay::new();
    if !removed {
        body.append(&output_dock(&host, &toasts, sticker.id(), motion));
    }
    toasts.set_child(Some(&body));
    toasts.set_vexpand(true);
    page.set_child(Some(&toasts));
    page.set_title(sticker.title());
    sync_star(&chrome.star, sticker.starred());
    chrome.star.set_visible(!removed);
    chrome.star.set_sensitive(!removed);
    let surface = Rc::new(Surface {
        toasts,
        title_label: title_label.clone(),
        title_entry: title_entry.clone(),
        note_label: note_label.clone(),
        note_entry: note_entry.clone(),
        note_add: note_add.clone(),
        star: chrome.star.clone(),
        page: page.clone(),
        chips,
        organize: organize.clone(),
        editor,
        deleted_hint,
        header_title: chrome.title.clone(),
    });

    if let Some(path) = thumb {
        show_file(&picture, path);
    }
    let animated = matches!(detail.asset.format(), ImageFormat::Gif | ImageFormat::WebP)
        && detail.asset.animated();
    let media = Rc::new(RefCell::new(None));
    let playing = Rc::new(Cell::new(false));
    if let Some(path) = detail.original_path.clone() {
        if animated {
            let clip = if detail.asset.format() == ImageFormat::Gif {
                Clip::Gif
            } else {
                Clip::WebP
            };
            *media.borrow_mut() = Some((path.clone(), clip));
            playing.set(true);
            play.set_visible(true);
            start_playback(&links, path, clip, picture.clone(), status.clone());
        } else {
            request_preview(
                library.clone(),
                sticker.id(),
                picture.clone(),
                status.clone(),
            );
        }
    } else {
        i18n::bind(&status, "label", Key::FailureMissing);
        status.set_visible(true);
    }

    let title_label_for_click = title_label.clone();
    let title_entry_for_click = title_entry.clone();
    let model_for_click = Rc::clone(&model);
    let title_click = gtk4::GestureClick::new();
    title_click.connect_released(move |_, _, _, _| {
        if model_for_click.borrow().deleted {
            return;
        }
        title_entry_for_click.set_text(&title_label_for_click.text());
        title_label_for_click.set_visible(false);
        title_entry_for_click.set_visible(true);
        title_entry_for_click.grab_focus();
    });
    title_label.add_controller(title_click);
    let library_for_title = library.clone();
    let model_for_title = Rc::clone(&model);
    let surface_for_title = Rc::clone(&surface);
    title_entry.connect_activate(move |entry| {
        let text = entry.text().to_string();
        show_title(&surface_for_title, &text);
        commit_text(
            Rc::clone(&model_for_title),
            library_for_title.clone(),
            Rc::clone(&surface_for_title),
            text,
            true,
        );
    });
    let library_for_leave = library.clone();
    let model_for_leave = Rc::clone(&model);
    let surface_for_leave = Rc::clone(&surface);
    let title_for_leave = title_entry.clone();
    let focus = gtk4::EventControllerFocus::new();
    focus.connect_leave(move |_| {
        let text = title_for_leave.text().to_string();
        show_title(&surface_for_leave, &text);
        commit_text(
            Rc::clone(&model_for_leave),
            library_for_leave.clone(),
            Rc::clone(&surface_for_leave),
            text,
            true,
        );
    });
    title_entry.add_controller(focus);

    let note_label_for_click = note_label.clone();
    let note_entry_for_click = note_entry.clone();
    let model_for_note_click = Rc::clone(&model);
    let note_click = gtk4::GestureClick::new();
    note_click.connect_released(move |_, _, _, _| {
        if model_for_note_click.borrow().deleted {
            return;
        }
        note_entry_for_click.set_text(&note_label_for_click.text());
        note_label_for_click.set_visible(false);
        note_entry_for_click.set_visible(true);
        note_entry_for_click.grab_focus();
    });
    note_label.add_controller(note_click);
    let note_entry_for_add = note_entry.clone();
    let note_add_for_click = note_add.clone();
    let model_for_add = Rc::clone(&model);
    note_add.connect_clicked(move |_| {
        if model_for_add.borrow().deleted {
            return;
        }
        note_add_for_click.set_visible(false);
        note_entry_for_add.set_text("");
        note_entry_for_add.set_visible(true);
        note_entry_for_add.grab_focus();
    });
    let library_for_note = library.clone();
    let model_for_note = Rc::clone(&model);
    let surface_for_note = Rc::clone(&surface);
    note_entry.connect_activate(move |entry| {
        let text = entry.text().to_string();
        show_note(&surface_for_note, &text);
        commit_text(
            Rc::clone(&model_for_note),
            library_for_note.clone(),
            Rc::clone(&surface_for_note),
            text,
            false,
        );
    });
    let library_for_note_leave = library.clone();
    let model_for_note_leave = Rc::clone(&model);
    let surface_for_note_leave = Rc::clone(&surface);
    let note_for_leave = note_entry.clone();
    let note_focus = gtk4::EventControllerFocus::new();
    note_focus.connect_leave(move |_| {
        let text = note_for_leave.text().to_string();
        show_note(&surface_for_note_leave, &text);
        commit_text(
            Rc::clone(&model_for_note_leave),
            library_for_note_leave.clone(),
            Rc::clone(&surface_for_note_leave),
            text,
            false,
        );
    });
    note_entry.add_controller(note_focus);

    let page_for_organize = page.clone();
    let library_for_organize = library.clone();
    let model_for_organize = Rc::clone(&model);
    let surface_for_organize = Rc::clone(&surface);
    organize.connect_clicked(move |_| {
        let id = model_for_organize.borrow().id;
        let library = library_for_organize.clone();
        let surface = Rc::clone(&surface_for_organize);
        let model = Rc::clone(&model_for_organize);
        let page = page_for_organize.clone();
        let notify_surface = Rc::clone(&surface);
        open_organize(
            &page,
            library.clone(),
            id,
            move |message| toast(&notify_surface, message),
            move || {
                reload_relations(
                    Rc::clone(&model),
                    library.clone(),
                    Rc::clone(&surface),
                    true,
                );
            },
        );
    });

    let links_for_play = links.clone_slots();
    let media_for_play = Rc::clone(&media);
    let playing_for_play = Rc::clone(&playing);
    let picture_for_play = picture.clone();
    let status_for_play = status.clone();
    play.connect_clicked(move |button| {
        if playing_for_play.get() {
            links_for_play.playback.borrow_mut().take();
            playing_for_play.set(false);
            button.set_icon_name("media-playback-start-symbolic");
            i18n::bind(button, "tooltip-text", Key::PlayAnimation);
            return;
        }
        let Some((path, clip)) = media_for_play.borrow().clone() else {
            return;
        };
        playing_for_play.set(true);
        button.set_icon_name("media-playback-pause-symbolic");
        i18n::bind(button, "tooltip-text", Key::PauseAnimation);
        start_playback(
            &links_for_play,
            path,
            clip,
            picture_for_play.clone(),
            status_for_play.clone(),
        );
    });

    let library_for_star = library.clone();
    let model_for_star = Rc::clone(&model);
    let surface_for_star = Rc::clone(&surface);
    let star_button = chrome.star.clone();
    if let Some(id) = chrome.star_handler.borrow_mut().take() {
        chrome.star.disconnect(id);
    }
    let handler = chrome.star.connect_clicked(move |_| {
        let starred = {
            let model = model_for_star.borrow();
            !model.starred
        };
        sync_star(&star_button, starred);
        let patch = match StickerPatch::new(
            FieldPatch::Missing,
            FieldPatch::Missing,
            FieldPatch::Set(starred),
        ) {
            Ok(patch) => patch,
            Err(_) => return,
        };
        queue_patch(
            Rc::clone(&model_for_star),
            library_for_star.clone(),
            Rc::clone(&surface_for_star),
            patch,
        );
    });
    *chrome.star_handler.borrow_mut() = Some(handler);

    let library_for_action = library.clone();
    let model_for_action = Rc::clone(&model);
    let surface_for_action = Rc::clone(&surface);
    action.connect_clicked(move |_| {
        let deleted = model_for_action.borrow().deleted;
        let heading = if deleted {
            i18n::text(Key::RestoreStickerTitle)
        } else {
            i18n::text(Key::Delete)
        };
        let body = if deleted {
            i18n::text(Key::RestoreHint)
        } else {
            i18n::text(Key::DeleteHint)
        };
        let accept = if deleted {
            i18n::text(Key::Restore)
        } else {
            i18n::text(Key::Delete)
        };
        let library = library_for_action.clone();
        let model = Rc::clone(&model_for_action);
        let surface = Rc::clone(&surface_for_action);
        let page = surface.page.clone();
        confirm(&page, heading, body, accept, !deleted, move || {
            if deleted {
                restore_one(Rc::clone(&model), library.clone(), Rc::clone(&surface));
            } else {
                delete_one(Rc::clone(&model), library.clone(), Rc::clone(&surface));
            }
        });
    });

    let library_for_lists = library.clone();
    let model_for_lists = Rc::clone(&model);
    let surface_for_lists = Rc::clone(&surface);
    let refresh = Rc::new(move || {
        reload_relations(
            Rc::clone(&model_for_lists),
            library_for_lists.clone(),
            Rc::clone(&surface_for_lists),
            false,
        );
    });
    *links.refresh.borrow_mut() = Some(Rc::clone(&refresh) as Rc<dyn Fn()>);
    let library_for_sync = library.clone();
    let model_for_sync = Rc::clone(&model);
    let surface_for_sync = Rc::clone(&surface);
    let sync = Rc::new(move || {
        reload_relations(
            Rc::clone(&model_for_sync),
            library_for_sync.clone(),
            Rc::clone(&surface_for_sync),
            true,
        );
    });
    *links.sync.borrow_mut() = Some(sync);
    refresh();
    let menu_host = host;
    let menu_model = Rc::clone(&model);
    let menu_animated = detail.asset.animated();
    let menu_click = gtk4::GestureClick::new();
    menu_click.set_button(3);
    menu_click.connect_pressed(move |gesture, _, _, _| {
        let _ = gesture.set_state(gtk4::EventSequenceState::Claimed);
        let state = menu_model.borrow();
        let target = actions::Target {
            id: state.id,
            generation: state.generation,
            revision: state.revision,
            animated: menu_animated,
            starred: state.starred,
            deleted: state.deleted,
        };
        drop(state);
        let Some(anchor) = gesture.widget() else {
            return;
        };
        actions::popup(&menu_host, &anchor, target);
    });
    picture.add_controller(menu_click);
}

fn request_preview(library: Library, id: StickerId, picture: gtk4::Picture, status: gtk4::Label) {
    glib::spawn_future_local(async move {
        let loaded = match library.request_preview(id) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        match loaded {
            Ok(preview) => show_file(&picture, preview.path),
            Err(_) => {
                i18n::bind(&status, "label", Key::PreviewUnavailable);
                status.set_visible(true);
            }
        }
    });
}

fn show_file(picture: &gtk4::Picture, path: PathBuf) {
    let picture = picture.clone();
    glib::spawn_future_local(async move {
        let read = gio::spawn_blocking(move || std::fs::read(path)).await;
        let Ok(Ok(bytes)) = read else {
            return;
        };
        let bytes = glib::Bytes::from_owned(bytes);
        if let Ok(texture) = gtk4::gdk::Texture::from_bytes(&bytes) {
            picture.set_paintable(Some(&texture));
        }
    });
}

fn start_playback(
    links: &DetailLinks,
    path: PathBuf,
    clip: Clip,
    picture: gtk4::Picture,
    status: gtk4::Label,
) {
    let (playback, receiver) = Playback::start(path, clip);
    *links.playback.borrow_mut() = Some(playback);
    glib::timeout_add_local(
        std::time::Duration::from_millis(16),
        move || match receiver.try_recv() {
            Ok(Event::Frame {
                width,
                height,
                bytes,
            }) => {
                if let Some(texture) = texture_from_rgba(width, height, bytes) {
                    picture.set_paintable(Some(&texture));
                }
                glib::ControlFlow::Continue
            }
            Ok(Event::Failed(fault)) => {
                let key = match fault {
                    PlayFault::Limit => Key::FailureLimit,
                    PlayFault::Broken => Key::FailureImage,
                };
                status.set_text(i18n::text(key));
                status.set_visible(true);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        },
    );
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

fn stop_playback(links: &DetailLinks) {
    links.playback.borrow_mut().take();
}

fn toast(surface: &Surface, message: &str) {
    surface.toasts.add_toast(adw::Toast::new(message));
}

fn sync_star(star: &gtk4::Button, starred: bool) {
    star.set_icon_name(if starred {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    });
    star.set_tooltip_text(Some(if starred {
        i18n::text(Key::Unfavorite)
    } else {
        i18n::text(Key::Favorite)
    }));
}

fn show_title(surface: &Surface, title: &str) {
    surface.title_label.set_text(title);
    surface.title_entry.set_text(title);
    surface.title_label.set_visible(true);
    surface.title_entry.set_visible(false);
    surface.page.set_title(title);
    surface.header_title.set_text(title);
}

fn show_note(surface: &Surface, note: &str) {
    let empty = note.is_empty();
    let deleted = surface.deleted_hint.is_visible();
    surface.note_label.set_text(note);
    surface.note_entry.set_text(note);
    surface.note_entry.set_visible(false);
    surface.note_label.set_visible(!empty);
    surface.note_add.set_visible(empty && !deleted);
}

fn format_label(format: ImageFormat) -> String {
    match format.mime().rsplit_once('/') {
        Some((_, subtype)) => subtype.to_uppercase(),
        None => format.mime().to_uppercase(),
    }
}

fn show_saved_text(surface: &Surface, title: &str, note: &str, starred: bool) {
    show_title(surface, title);
    show_note(surface, note);
    sync_star(&surface.star, starred);
}

fn commit_text(
    model: Rc<RefCell<Model>>,
    library: Library,
    surface: Rc<Surface>,
    text: String,
    title: bool,
) {
    let unchanged = {
        let model = model.borrow();
        if model.deleted {
            return;
        }
        if title {
            model.title == text
        } else {
            model.note == text
        }
    };
    if unchanged {
        return;
    }
    let patch = if title {
        StickerPatch::new(
            FieldPatch::Set(text),
            FieldPatch::Missing,
            FieldPatch::Missing,
        )
    } else {
        StickerPatch::new(
            FieldPatch::Missing,
            FieldPatch::Set(text),
            FieldPatch::Missing,
        )
    };
    let Ok(patch) = patch else {
        return;
    };
    queue_patch(model, library, surface, patch);
}

fn queue_patch(
    model: Rc<RefCell<Model>>,
    library: Library,
    surface: Rc<Surface>,
    patch: StickerPatch,
) {
    if model.borrow().deleted {
        return;
    }
    apply_patch_fields(&model, &patch);
    ensure_pump(model, library, surface);
}

fn apply_patch_fields(model: &Rc<RefCell<Model>>, patch: &StickerPatch) {
    let mut state = model.borrow_mut();
    if let FieldPatch::Set(value) = patch.title() {
        state.title.clone_from(value);
    }
    if let FieldPatch::Set(value) = patch.note() {
        state.note.clone_from(value);
    }
    if let FieldPatch::Set(value) = patch.starred() {
        state.starred = *value;
    }
    state.text_dirty = true;
    state.epoch = state.epoch.saturating_add(1);
}

enum Edit {
    Text {
        id: StickerId,
        generation: Generation,
        title: String,
        note: String,
        starred: bool,
    },
    Relations {
        id: StickerId,
        generation: Generation,
        collection: Option<(CollectionId, Generation)>,
        tags: Vec<(TagId, Generation)>,
    },
}

fn ensure_pump(model: Rc<RefCell<Model>>, library: Library, surface: Rc<Surface>) {
    if model.borrow().busy {
        return;
    }
    let pending = {
        let state = model.borrow();
        state.text_dirty || state.relations_dirty
    };
    if !pending {
        return;
    }
    model.borrow_mut().busy = true;
    glib::spawn_future_local(async move {
        loop {
            let job = {
                let mut state = model.borrow_mut();
                if state.text_dirty {
                    state.text_dirty = false;
                    Some(Edit::Text {
                        id: state.id,
                        generation: state.generation,
                        title: state.title.clone(),
                        note: state.note.clone(),
                        starred: state.starred,
                    })
                } else if state.relations_dirty {
                    state.relations_dirty = false;
                    Some(Edit::Relations {
                        id: state.id,
                        generation: state.generation,
                        collection: state
                            .collections
                            .iter()
                            .find(|item| item.checked)
                            .map(|item| (item.id, item.generation)),
                        tags: state
                            .tags
                            .iter()
                            .filter(|item| item.checked)
                            .map(|item| (item.id, item.generation))
                            .collect(),
                    })
                } else {
                    state.busy = false;
                    None
                }
            };
            let Some(job) = job else {
                return;
            };
            match job {
                Edit::Text {
                    id,
                    generation,
                    title,
                    note,
                    starred,
                } => {
                    let patch = match StickerPatch::new(
                        FieldPatch::Set(title),
                        FieldPatch::Set(note),
                        FieldPatch::Set(starred),
                    ) {
                        Ok(patch) => patch,
                        Err(error) => {
                            abandon(Rc::clone(&model), library, surface, error_text(&error));
                            return;
                        }
                    };
                    let saved = match library.patch_sticker(id, generation, patch) {
                        Ok(task) => task.wait().await,
                        Err(error) => Err(error),
                    };
                    match saved {
                        Ok(sticker) => {
                            let title = sticker.title().to_owned();
                            let note = sticker.note().to_owned();
                            let starred = sticker.starred();
                            {
                                let mut state = model.borrow_mut();
                                state.generation = sticker.lifecycle().generation();
                                state.revision = sticker.lifecycle().revision();
                                state.title.clone_from(&title);
                                state.note.clone_from(&note);
                                state.starred = starred;
                            }
                            show_saved_text(&surface, &title, &note, starred);
                        }
                        Err(error) => {
                            abandon(
                                Rc::clone(&model),
                                library,
                                surface,
                                i18n::core(error.code()),
                            );
                            return;
                        }
                    }
                }
                Edit::Relations {
                    id,
                    generation,
                    collection,
                    tags,
                } => {
                    let saved = match library.set_sticker_organization(
                        id,
                        generation,
                        Some(collection),
                        Some(tags),
                    ) {
                        Ok(task) => task.wait().await,
                        Err(error) => Err(error),
                    };
                    if let Err(error) = saved {
                        abandon(
                            Rc::clone(&model),
                            library,
                            surface,
                            i18n::core(error.code()),
                        );
                        return;
                    }
                }
            }
        }
    });
}

fn error_text(error: &memedock_domain::error::DomainError) -> &'static str {
    match error {
        memedock_domain::error::DomainError::InvalidValue("name") => i18n::text(Key::FailureInput),
        _ => i18n::text(Key::FailureUnknown),
    }
}

fn abandon(model: Rc<RefCell<Model>>, library: Library, surface: Rc<Surface>, message: &str) {
    toast(&surface, message);
    {
        let mut state = model.borrow_mut();
        state.busy = false;
        state.text_dirty = false;
        state.relations_dirty = false;
        state.pending_collection = None;
        state.pending_tag = None;
        state.epoch = state.epoch.saturating_add(1);
    }
    reload_relations(model, library, surface, true);
}

fn reload_relations(
    model: Rc<RefCell<Model>>,
    library: Library,
    surface: Rc<Surface>,
    replace: bool,
) {
    let epoch = model.borrow().epoch;
    glib::spawn_future_local(async move {
        let id = model.borrow().id;
        let detail = match library.sticker_detail(id) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        let detail = match detail {
            Ok(detail) => detail,
            Err(error) => {
                toast(&surface, i18n::core(error.code()));
                return;
            }
        };
        let collections = match library.collections(false) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        let collections = match collections {
            Ok(collections) => collections,
            Err(error) => {
                toast(&surface, i18n::core(error.code()));
                return;
            }
        };
        let tags = match library.tags(false) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        let tags = match tags {
            Ok(tags) => tags,
            Err(error) => {
                toast(&surface, i18n::core(error.code()));
                return;
            }
        };
        if replace && model.borrow().epoch != epoch {
            return;
        }
        let selected_collections: Vec<_> = detail.collection.iter().map(|item| item.id()).collect();
        let selected_tags: Vec<_> = detail.tags.iter().map(|item| item.id()).collect();
        let (title, note, starred, link_new) = {
            let mut state = model.borrow_mut();
            state.filling = true;
            state.generation = detail.sticker.lifecycle().generation();
            state.revision = detail.sticker.lifecycle().revision();
            if replace {
                state.title = detail.sticker.title().to_owned();
                state.note = detail.sticker.note().to_owned();
                state.starred = detail.sticker.starred();
                state.deleted = !detail.sticker.lifecycle().is_active();
            }
            let previous_collections = if replace {
                Vec::new()
            } else {
                state.collections.clone()
            };
            let previous_tags = if replace {
                Vec::new()
            } else {
                state.tags.clone()
            };
            let pending_collection = state.pending_collection;
            let pending_tag = state.pending_tag;
            let mut link_new = false;
            state.collections = collections
                .iter()
                .map(|item| {
                    let pending = pending_collection == Some(item.id());
                    if pending {
                        link_new = true;
                    }
                    let kept = previous_collections
                        .iter()
                        .find(|row| row.id == item.id())
                        .map(|row| row.checked);
                    Relation {
                        id: item.id(),
                        generation: item.lifecycle().generation(),
                        name: item.name().as_str().to_owned(),
                        checked: if pending_collection.is_some() {
                            pending
                        } else {
                            kept.unwrap_or_else(|| selected_collections.contains(&item.id()))
                        },
                    }
                })
                .collect();
            state.tags = tags
                .iter()
                .map(|item| {
                    let pending = pending_tag == Some(item.id());
                    if pending {
                        link_new = true;
                    }
                    let kept = previous_tags
                        .iter()
                        .find(|row| row.id == item.id())
                        .map(|row| row.checked);
                    Relation {
                        id: item.id(),
                        generation: item.lifecycle().generation(),
                        name: item.name().as_str().to_owned(),
                        checked: kept.unwrap_or_else(|| selected_tags.contains(&item.id()))
                            || pending,
                    }
                })
                .collect();
            if link_new {
                state.pending_collection = None;
                state.pending_tag = None;
            }
            (
                state.title.clone(),
                state.note.clone(),
                state.starred,
                link_new,
            )
        };
        if replace {
            show_saved_text(&surface, &title, &note, starred);
        }
        fill_relations(Rc::clone(&model), library.clone(), Rc::clone(&surface));
        if link_new {
            queue_relations(model, library, surface);
        }
    });
}

fn fill_relations(model: Rc<RefCell<Model>>, library: Library, surface: Rc<Surface>) {
    let relations = &surface.editor;
    model.borrow_mut().filling = true;
    while let Some(child) = relations.first_child() {
        relations.remove(&child);
    }
    while let Some(child) = surface.chips.first_child() {
        surface.chips.remove(&child);
    }
    let heading = i18n::label(Key::ImportCollection);
    heading.set_halign(gtk4::Align::Start);
    relations.append(&heading);
    let collections = model.borrow().collections.clone();
    let none = gtk4::CheckButton::with_label(i18n::text(Key::NoCollection));
    none.set_active(!collections.iter().any(|item| item.checked));
    let tracked = Rc::clone(&model);
    let source = library.clone();
    let view = Rc::clone(&surface);
    none.connect_toggled(move |button| {
        if !button.is_active() || tracked.borrow().filling {
            return;
        }
        for row in &mut tracked.borrow_mut().collections {
            row.checked = false;
        }
        queue_relations(Rc::clone(&tracked), source.clone(), Rc::clone(&view));
    });
    relations.append(&none);
    for item in collections {
        if item.checked {
            add_chip(&surface.chips, &item.name);
        }
        let button = gtk4::CheckButton::with_label(&item.name);
        button.set_group(Some(&none));
        button.set_active(item.checked);
        let model_for_toggle = Rc::clone(&model);
        let library_for_toggle = library.clone();
        let surface_for_toggle = Rc::clone(&surface);
        let id = item.id;
        button.connect_toggled(move |button| {
            if model_for_toggle.borrow().filling {
                return;
            }
            if !button.is_active() {
                return;
            }
            for row in &mut model_for_toggle.borrow_mut().collections {
                row.checked = row.id == id;
            }
            queue_relations(
                Rc::clone(&model_for_toggle),
                library_for_toggle.clone(),
                Rc::clone(&surface_for_toggle),
            );
        });
        relations.append(&button);
    }
    let create = i18n::button(Key::NewCollection);
    create.set_halign(gtk4::Align::Start);
    let model_for_create = Rc::clone(&model);
    let library_for_create = library.clone();
    let surface_for_create = Rc::clone(&surface);
    create.connect_clicked(move |_| {
        ask_name(
            i18n::text(Key::NewCollection),
            Rc::clone(&model_for_create),
            library_for_create.clone(),
            Rc::clone(&surface_for_create),
            true,
        );
    });
    relations.append(&create);
    let tags_heading = i18n::label(Key::TagsTitle);
    tags_heading.set_halign(gtk4::Align::Start);
    relations.append(&tags_heading);
    let tags = model.borrow().tags.clone();
    for item in tags {
        if item.checked {
            add_chip(&surface.chips, &item.name);
        }
        let button = gtk4::CheckButton::with_label(&item.name);
        button.set_active(item.checked);
        let model_for_toggle = Rc::clone(&model);
        let library_for_toggle = library.clone();
        let surface_for_toggle = Rc::clone(&surface);
        let id = item.id;
        button.connect_toggled(move |button| {
            if model_for_toggle.borrow().filling {
                return;
            }
            if let Some(row) = model_for_toggle
                .borrow_mut()
                .tags
                .iter_mut()
                .find(|row| row.id == id)
            {
                row.checked = button.is_active();
            }
            queue_relations(
                Rc::clone(&model_for_toggle),
                library_for_toggle.clone(),
                Rc::clone(&surface_for_toggle),
            );
        });
        relations.append(&button);
    }
    let create_tag = i18n::button(Key::NewTag);
    create_tag.set_halign(gtk4::Align::Start);
    let model_for_tag = Rc::clone(&model);
    let library_for_tag = library.clone();
    let surface_for_tag = Rc::clone(&surface);
    create_tag.connect_clicked(move |_| {
        ask_name(
            i18n::text(Key::NewTag),
            Rc::clone(&model_for_tag),
            library_for_tag.clone(),
            Rc::clone(&surface_for_tag),
            false,
        );
    });
    relations.append(&create_tag);
    let collections_chosen = model.borrow().collections.iter().any(|item| item.checked);
    let tags_chosen = model.borrow().tags.iter().any(|item| item.checked);
    surface
        .organize
        .set_label(if collections_chosen || tags_chosen {
            i18n::text(Key::Organize)
        } else {
            i18n::text(Key::OrganizeHint)
        });
    model.borrow_mut().filling = false;
}

fn add_chip(chips: &gtk4::FlowBox, name: &str) {
    let chip = gtk4::Label::new(Some(name));
    chip.add_css_class("relation-chip");
    chips.insert(&chip, -1);
}

fn queue_relations(model: Rc<RefCell<Model>>, library: Library, surface: Rc<Surface>) {
    if model.borrow().deleted || model.borrow().filling {
        return;
    }
    {
        let mut state = model.borrow_mut();
        state.relations_dirty = true;
        state.epoch = state.epoch.saturating_add(1);
    }
    ensure_pump(model, library, surface);
}

fn ask_name(
    heading: &str,
    model: Rc<RefCell<Model>>,
    library: Library,
    surface: Rc<Surface>,
    collection: bool,
) {
    let entry = gtk4::Entry::new();
    i18n::bind(&entry, "placeholder-text", Key::Name);
    let dialog = adw::AlertDialog::new(Some(heading), None);
    dialog.set_extra_child(Some(&entry));
    dialog.add_response("cancel", i18n::text(Key::Cancel));
    dialog.add_response("create", i18n::text(Key::Done));
    dialog.set_default_response(Some("create"));
    dialog.set_close_response("cancel");
    let page = surface.page.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "create" {
            return;
        }
        let Ok(name) = Name::new(entry.text().to_string()) else {
            toast(&surface, i18n::text(Key::FailureInput));
            return;
        };
        let library = library.clone();
        let model = Rc::clone(&model);
        let surface = Rc::clone(&surface);
        glib::spawn_future_local(async move {
            if collection {
                let created = match library.create_collection(name, None) {
                    Ok(task) => task.wait().await,
                    Err(error) => Err(error),
                };
                match created {
                    Ok(item) => {
                        model.borrow_mut().pending_collection = Some(item.id());
                        reload_relations(model, library, surface, false);
                    }
                    Err(error) => toast(&surface, i18n::core(error.code())),
                }
            } else {
                let created = match library.create_tag(name) {
                    Ok(task) => task.wait().await,
                    Err(error) => Err(error),
                };
                match created {
                    Ok(item) => {
                        model.borrow_mut().pending_tag = Some(item.id());
                        reload_relations(model, library, surface, false);
                    }
                    Err(error) => toast(&surface, i18n::core(error.code())),
                }
            }
        });
    });
    dialog.present(Some(&page));
}

pub fn open_organize(
    parent: &impl IsA<gtk4::Widget>,
    library: Library,
    id: StickerId,
    notify: impl Fn(&str) + 'static,
    saved: impl Fn() + 'static,
) {
    let parent = parent.upcast_ref::<gtk4::Widget>().clone();
    let notify = Rc::new(notify);
    let saved = Rc::new(saved);
    glib::spawn_future_local(async move {
        let detail = match library.sticker_detail(id) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        let detail = match detail {
            Ok(detail) => detail,
            Err(error) => {
                notify(i18n::core(error.code()));
                return;
            }
        };
        if !detail.sticker.lifecycle().is_active() {
            return;
        }
        let collections = match library.collections(false) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        let collections = match collections {
            Ok(collections) => collections,
            Err(error) => {
                notify(i18n::core(error.code()));
                return;
            }
        };
        let tags = match library.tags(false) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        let tags = match tags {
            Ok(tags) => tags,
            Err(error) => {
                notify(i18n::core(error.code()));
                return;
            }
        };
        let selected_collections: Vec<_> = detail.collection.iter().map(|item| item.id()).collect();
        let selected_tags: Vec<_> = detail.tags.iter().map(|item| item.id()).collect();
        let state = Rc::new(RefCell::new(Organize {
            library,
            id,
            generation: detail.sticker.lifecycle().generation(),
            collections: collections
                .iter()
                .map(|item| OrganizeRow {
                    id: item.id(),
                    generation: item.lifecycle().generation(),
                    name: item.name().as_str().to_owned(),
                    checked: selected_collections.contains(&item.id()),
                })
                .collect(),
            tags: tags
                .iter()
                .map(|item| OrganizeRow {
                    id: item.id(),
                    generation: item.lifecycle().generation(),
                    name: item.name().as_str().to_owned(),
                    checked: selected_tags.contains(&item.id()),
                })
                .collect(),
            busy: false,
            dirty: false,
            filling: false,
            notify,
            saved,
            parent: parent.clone(),
        }));
        let column = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        column.set_margin_top(12);
        column.set_margin_bottom(12);
        column.set_margin_start(16);
        column.set_margin_end(16);
        fill_organize(&state, &column);
        let dialog = adw::Dialog::new();
        i18n::bind(&dialog, "title", Key::Organize);
        dialog.set_content_width(420);
        dialog.set_child(Some(&column));
        dialog.present(Some(&parent));
    });
}

#[derive(Clone)]
struct OrganizeRow<T> {
    id: T,
    generation: Generation,
    name: String,
    checked: bool,
}

struct Organize {
    library: Library,
    id: StickerId,
    generation: Generation,
    collections: Vec<OrganizeRow<CollectionId>>,
    tags: Vec<OrganizeRow<TagId>>,
    busy: bool,
    dirty: bool,
    filling: bool,
    notify: Rc<dyn Fn(&str)>,
    saved: Rc<dyn Fn()>,
    parent: gtk4::Widget,
}

fn fill_organize(state: &Rc<RefCell<Organize>>, column: &gtk4::Box) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
    state.borrow_mut().filling = true;
    let heading = i18n::label(Key::ImportCollection);
    heading.set_halign(gtk4::Align::Start);
    column.append(&heading);
    let collections = state.borrow().collections.clone();
    let none = gtk4::CheckButton::with_label(i18n::text(Key::NoCollection));
    none.set_active(!collections.iter().any(|item| item.checked));
    let tracked = Rc::clone(state);
    none.connect_toggled(move |button| {
        if !button.is_active() || tracked.borrow().filling {
            return;
        }
        {
            let mut state = tracked.borrow_mut();
            for row in &mut state.collections {
                row.checked = false;
            }
            state.dirty = true;
        }
        pump_organize(Rc::clone(&tracked));
    });
    column.append(&none);
    for item in collections {
        let button = collection_check(state, item.name, item.id);
        button.set_group(Some(&none));
        button.set_active(item.checked);
        column.append(&button);
    }
    let create = i18n::button(Key::NewCollection);
    create.set_halign(gtk4::Align::Start);
    let state_for_create = Rc::clone(state);
    let column_for_create = column.clone();
    create.connect_clicked(move |_| {
        ask_organize_name(
            Rc::clone(&state_for_create),
            column_for_create.clone(),
            true,
        );
    });
    column.append(&create);
    let tags_heading = i18n::label(Key::TagsTitle);
    tags_heading.set_halign(gtk4::Align::Start);
    column.append(&tags_heading);
    let tags = state.borrow().tags.clone();
    for item in tags {
        column.append(&tag_check(state, item.name, item.id));
    }
    let create_tag = i18n::button(Key::NewTag);
    create_tag.set_halign(gtk4::Align::Start);
    let state_for_tag = Rc::clone(state);
    let column_for_tag = column.clone();
    create_tag.connect_clicked(move |_| {
        ask_organize_name(Rc::clone(&state_for_tag), column_for_tag.clone(), false);
    });
    column.append(&create_tag);
    state.borrow_mut().filling = false;
}

fn collection_check(
    state: &Rc<RefCell<Organize>>,
    name: String,
    id: CollectionId,
) -> gtk4::CheckButton {
    let checked = state
        .borrow()
        .collections
        .iter()
        .find(|row| row.id == id)
        .is_some_and(|row| row.checked);
    let button = gtk4::CheckButton::with_label(&name);
    button.set_active(checked);
    let state = Rc::clone(state);
    button.connect_toggled(move |button| {
        if state.borrow().filling {
            return;
        }
        {
            let mut organize = state.borrow_mut();
            if !button.is_active() {
                return;
            }
            for row in &mut organize.collections {
                row.checked = row.id == id;
            }
            organize.dirty = true;
        }
        pump_organize(Rc::clone(&state));
    });
    button
}

fn tag_check(state: &Rc<RefCell<Organize>>, name: String, id: TagId) -> gtk4::CheckButton {
    let checked = state
        .borrow()
        .tags
        .iter()
        .find(|row| row.id == id)
        .is_some_and(|row| row.checked);
    let button = gtk4::CheckButton::with_label(&name);
    button.set_active(checked);
    let state = Rc::clone(state);
    button.connect_toggled(move |button| {
        if state.borrow().filling {
            return;
        }
        {
            let mut organize = state.borrow_mut();
            if let Some(row) = organize.tags.iter_mut().find(|row| row.id == id) {
                row.checked = button.is_active();
            }
            organize.dirty = true;
        }
        pump_organize(Rc::clone(&state));
    });
    button
}

fn pump_organize(state: Rc<RefCell<Organize>>) {
    let busy = state.borrow().busy;
    let dirty = state.borrow().dirty;
    if busy || !dirty {
        return;
    }
    glib::spawn_future_local(async move {
        loop {
            let (library, id, generation, collections, tags) = {
                let mut organize = state.borrow_mut();
                if organize.busy || !organize.dirty {
                    return;
                }
                organize.dirty = false;
                organize.busy = true;
                (
                    organize.library.clone(),
                    organize.id,
                    organize.generation,
                    organize
                        .collections
                        .iter()
                        .find(|row| row.checked)
                        .map(|row| (row.id, row.generation)),
                    organize
                        .tags
                        .iter()
                        .filter(|row| row.checked)
                        .map(|row| (row.id, row.generation))
                        .collect::<Vec<_>>(),
                )
            };
            let saved = match library.set_sticker_organization(
                id,
                generation,
                Some(collections),
                Some(tags),
            ) {
                Ok(task) => task.wait().await,
                Err(error) => Err(error),
            };
            let mut organize = state.borrow_mut();
            organize.busy = false;
            match saved {
                Ok(_) => {
                    let again = organize.dirty;
                    let saved = Rc::clone(&organize.saved);
                    drop(organize);
                    if again {
                        continue;
                    }
                    saved();
                    return;
                }
                Err(error) => {
                    let message = i18n::core(error.code());
                    let notify = Rc::clone(&organize.notify);
                    organize.dirty = false;
                    drop(organize);
                    notify(message);
                    return;
                }
            }
        }
    });
}

fn ask_organize_name(state: Rc<RefCell<Organize>>, column: gtk4::Box, collection: bool) {
    let heading = if collection {
        i18n::text(Key::NewCollection)
    } else {
        i18n::text(Key::NewTag)
    };
    let entry = gtk4::Entry::new();
    i18n::bind(&entry, "placeholder-text", Key::Name);
    let dialog = adw::AlertDialog::new(Some(heading), None);
    dialog.set_extra_child(Some(&entry));
    dialog.add_response("cancel", i18n::text(Key::Cancel));
    dialog.add_response("create", i18n::text(Key::Done));
    dialog.set_default_response(Some("create"));
    dialog.set_close_response("cancel");
    let parent = state.borrow().parent.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "create" {
            return;
        }
        let Ok(name) = Name::new(entry.text().to_string()) else {
            (state.borrow().notify)(i18n::text(Key::FailureInput));
            return;
        };
        let state = Rc::clone(&state);
        let column = column.clone();
        glib::spawn_future_local(async move {
            let library = state.borrow().library.clone();
            let failed = if collection {
                match library.create_collection(name, None) {
                    Ok(task) => match task.wait().await {
                        Ok(item) => {
                            state.borrow_mut().collections.push(OrganizeRow {
                                id: item.id(),
                                generation: item.lifecycle().generation(),
                                name: item.name().as_str().to_owned(),
                                checked: true,
                            });
                            None
                        }
                        Err(error) => Some(error),
                    },
                    Err(error) => Some(error),
                }
            } else {
                match library.create_tag(name) {
                    Ok(task) => match task.wait().await {
                        Ok(item) => {
                            state.borrow_mut().tags.push(OrganizeRow {
                                id: item.id(),
                                generation: item.lifecycle().generation(),
                                name: item.name().as_str().to_owned(),
                                checked: true,
                            });
                            None
                        }
                        Err(error) => Some(error),
                    },
                    Err(error) => Some(error),
                }
            };
            if let Some(error) = failed {
                (state.borrow().notify)(i18n::core(error.code()));
                return;
            }
            state.borrow_mut().dirty = true;
            fill_organize(&state, &column);
            pump_organize(state);
        });
    });
    dialog.present(Some(&parent));
}

pub(crate) fn confirm(
    parent: &impl IsA<gtk4::Widget>,
    heading: &str,
    body: &str,
    accept: &str,
    destructive: bool,
    then: impl Fn() + 'static,
) {
    let dialog = adw::AlertDialog::new(Some(heading), Some(body));
    dialog.add_response("cancel", i18n::text(Key::Cancel));
    dialog.add_response("accept", accept);
    dialog.set_close_response("cancel");
    dialog.set_default_response(Some("cancel"));
    if destructive {
        dialog.set_response_appearance("accept", adw::ResponseAppearance::Destructive);
    }
    dialog.connect_response(None, move |_, response| {
        if response == "accept" {
            then();
        }
    });
    dialog.present(Some(parent.upcast_ref()));
}

fn delete_one(model: Rc<RefCell<Model>>, library: Library, surface: Rc<Surface>) {
    glib::spawn_future_local(async move {
        let (id, generation) = {
            let state = model.borrow();
            (state.id, state.generation)
        };
        let deleted = match library.delete_sticker(id, generation) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        match deleted {
            Ok(_) => pop_detail(&surface.page),
            Err(error) => {
                toast(&surface, i18n::core(error.code()));
                reload_relations(model, library, surface, true);
            }
        }
    });
}

fn restore_one(model: Rc<RefCell<Model>>, library: Library, surface: Rc<Surface>) {
    glib::spawn_future_local(async move {
        let (id, generation, revision) = {
            let state = model.borrow();
            (state.id, state.generation, state.revision)
        };
        let restored = match library.restore_sticker(id, generation, revision) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        match restored {
            Ok(_) => pop_detail(&surface.page),
            Err(error) => {
                toast(&surface, i18n::core(error.code()));
                reload_relations(model, library, surface, true);
            }
        }
    });
}

fn pop_detail(page: &adw::NavigationPage) {
    if let Some(navigation) = page
        .parent()
        .and_then(|parent| parent.downcast::<adw::NavigationView>().ok())
    {
        navigation.pop();
    }
}

type OutputSettled = Rc<dyn Fn(Option<&str>)>;

#[derive(Clone, Copy)]
struct OutputItem {
    id: StickerId,
    animated: bool,
}

#[derive(Clone)]
struct Dock {
    format: gtk4::Button,
    copy: gtk4::Button,
    save: gtk4::Button,
    share: gtk4::Button,
    first: gtk4::Button,
}

fn output_dock(
    host: &Host,
    toasts: &adw::ToastOverlay,
    id: StickerId,
    animated: bool,
) -> gtk4::Box {
    let dock = Dock {
        format: gtk4::Button::new(),
        copy: gtk4::Button::from_icon_name("edit-copy-symbolic"),
        save: gtk4::Button::from_icon_name("document-save-symbolic"),
        share: gtk4::Button::new(),
        first: gtk4::Button::new(),
    };
    i18n::bind(&dock.copy, "tooltip-text", Key::CopyImage);
    i18n::bind(&dock.save, "tooltip-text", Key::SaveImage);
    dock.format
        .set_tooltip_text(Some(i18n::text(Key::ExportChoose)));
    dock.share.add_css_class("suggested-action");
    dock.share.set_hexpand(true);
    dock.first.add_css_class("flat");
    dock.first.set_hexpand(true);
    let preset = Rc::clone(&host.preset);
    apply_dock(&dock, animated, preset.get());

    let popover = gtk4::Popover::new();
    popover.set_parent(&dock.format);
    popover.set_position(gtk4::PositionType::Top);
    let choices = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    choices.set_margin_top(6);
    choices.set_margin_bottom(6);
    for choice in [
        ExportPreset::Original,
        ExportPreset::CompatiblePng,
        ExportPreset::WhiteBackground,
        ExportPreset::SmallJpeg,
    ] {
        choices.append(&preset_choice(
            &popover,
            choice,
            Rc::clone(&preset),
            dock.clone(),
            animated,
        ));
    }
    popover.set_child(Some(&choices));
    let popover_toggle = popover.clone();
    dock.format.connect_clicked(move |_| {
        if popover_toggle.is_visible() {
            popover_toggle.popdown();
        } else {
            popover_toggle.popup();
        }
    });

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&dock.format);
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    row.append(&spacer);
    row.append(&dock.copy);
    row.append(&dock.save);
    let bar = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    bar.set_margin_top(12);
    bar.set_margin_bottom(16);
    bar.set_margin_start(24);
    bar.set_margin_end(24);
    bar.append(&row);
    bar.append(&dock.share);
    bar.append(&dock.first);
    let shell = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    shell.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    shell.append(&bar);

    let toasts = toasts.clone();
    let dock_settled = dock.clone();
    let preset_settled = Rc::clone(&preset);
    let settled: OutputSettled = Rc::new(move |message| {
        set_dock_busy(&dock_settled, false);
        apply_dock(&dock_settled, animated, preset_settled.get());
        if let Some(message) = message {
            toasts.add_toast(adw::Toast::new(message));
        }
    });
    let item = OutputItem { id, animated };
    bind_copy(&dock, host, item, &preset, &settled);
    bind_save(&dock, host, item, &preset, &settled);
    bind_share(&dock, host, item, &preset, &settled, false);
    bind_share(&dock, host, item, &preset, &settled, true);
    shell
}

fn preset_choice(
    popover: &gtk4::Popover,
    choice: ExportPreset,
    preset: Rc<Cell<ExportPreset>>,
    dock: Dock,
    animated: bool,
) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.add_css_class("flat");
    button.set_hexpand(true);
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    column.set_margin_start(6);
    column.set_margin_end(6);
    let title = gtk4::Label::new(Some(output::preset_name(choice)));
    title.set_halign(gtk4::Align::Start);
    let hint = gtk4::Label::new(Some(output::preset_hint(choice)));
    hint.set_halign(gtk4::Align::Start);
    hint.set_wrap(true);
    hint.set_max_width_chars(36);
    hint.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    hint.add_css_class("caption");
    hint.add_css_class("dim-label");
    column.append(&title);
    column.append(&hint);
    button.set_child(Some(&column));
    let popover = popover.clone();
    button.connect_clicked(move |_| {
        preset.set(choice);
        apply_dock(&dock, animated, choice);
        popover.popdown();
    });
    button
}

fn apply_dock(dock: &Dock, animated: bool, preset: ExportPreset) {
    let name = output::preset_name(preset);
    dock.format.set_label(name);
    let share = if preset == ExportPreset::Original || output::flattens(animated, preset) {
        i18n::text(Key::ShareOriginal)
    } else {
        i18n::text(Key::Share)
    };
    dock.share.set_label(share);
    let first = i18n::share_first_frame(name);
    dock.first.set_label(&first);
    dock.first.set_visible(output::flattens(animated, preset));
}

fn set_dock_busy(dock: &Dock, busy: bool) {
    for button in [
        &dock.format,
        &dock.copy,
        &dock.save,
        &dock.share,
        &dock.first,
    ] {
        button.set_sensitive(!busy);
    }
    if busy {
        i18n::bind(&dock.share, "label", Key::SharePreparing);
    }
}

fn bind_copy(
    dock: &Dock,
    host: &Host,
    item: OutputItem,
    preset: &Rc<Cell<ExportPreset>>,
    settled: &OutputSettled,
) {
    let library = host.library.clone();
    let hold = Rc::clone(&host.clipboard);
    let anchor = host.navigation.clone();
    let preset = Rc::clone(preset);
    let settled = Rc::clone(settled);
    let dock = dock.clone();
    let button = dock.copy.clone();
    button.connect_clicked(move |_| {
        let settled = Rc::clone(&settled);
        let started = output::spawn_copy(
            library.clone(),
            Rc::clone(&hold),
            &anchor,
            Request {
                id: item.id,
                animated: item.animated,
                preset: preset.get(),
                first_frame: false,
            },
            CopyKind::Image,
            move |message| settled(message),
        );
        if started {
            set_dock_busy(&dock, true);
        }
    });
}

fn bind_save(
    dock: &Dock,
    host: &Host,
    item: OutputItem,
    preset: &Rc<Cell<ExportPreset>>,
    settled: &OutputSettled,
) {
    let library = host.library.clone();
    let hold = Rc::clone(&host.clipboard);
    let anchor = host.navigation.clone();
    let preset = Rc::clone(preset);
    let settled = Rc::clone(settled);
    let dock = dock.clone();
    let button = dock.save.clone();
    button.connect_clicked(move |_| {
        let settled = Rc::clone(&settled);
        let started = output::spawn_save(
            library.clone(),
            Rc::clone(&hold),
            &anchor,
            Request {
                id: item.id,
                animated: item.animated,
                preset: preset.get(),
                first_frame: false,
            },
            move |message| settled(message),
        );
        if started {
            set_dock_busy(&dock, true);
        }
    });
}

fn bind_share(
    dock: &Dock,
    host: &Host,
    item: OutputItem,
    preset: &Rc<Cell<ExportPreset>>,
    settled: &OutputSettled,
    first_frame: bool,
) {
    let library = host.library.clone();
    let hold = Rc::clone(&host.clipboard);
    let anchor = host.navigation.clone();
    let preset = Rc::clone(preset);
    let settled = Rc::clone(settled);
    let dock = dock.clone();
    let button = if first_frame {
        dock.first.clone()
    } else {
        dock.share.clone()
    };
    button.connect_clicked(move |_| {
        let settled = Rc::clone(&settled);
        let started = output::spawn_share(
            library.clone(),
            Rc::clone(&hold),
            &anchor,
            Request {
                id: item.id,
                animated: item.animated,
                preset: preset.get(),
                first_frame,
            },
            move |message| settled(message),
        );
        if started {
            set_dock_busy(&dock, true);
        }
    });
}
