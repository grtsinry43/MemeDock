use crate::detail::{self, DetailRefresh};
use crate::i18n::{self, Key};
use crate::output::{self, CopyKind, Hold, Request};
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use memedock_core::Library;
use memedock_domain::asset::ImageFormat;
use memedock_domain::change::{FieldPatch, StickerPatch};
use memedock_domain::export::ExportPreset;
use memedock_domain::identity::StickerId;
use memedock_domain::version::{Generation, Revision};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuRow {
    Share,
    ShareFirstFrame,
    CopyImage,
    CopyFile,
    SaveAs,
    Star,
    Unstar,
    Organize,
    Delete,
    Restore,
}

pub fn menu_rows(deleted: bool, starred: bool, flattens: bool) -> &'static [MenuRow] {
    if deleted {
        &[MenuRow::Restore]
    } else if flattens && starred {
        &[
            MenuRow::Share,
            MenuRow::ShareFirstFrame,
            MenuRow::CopyImage,
            MenuRow::CopyFile,
            MenuRow::SaveAs,
            MenuRow::Unstar,
            MenuRow::Organize,
            MenuRow::Delete,
        ]
    } else if flattens {
        &[
            MenuRow::Share,
            MenuRow::ShareFirstFrame,
            MenuRow::CopyImage,
            MenuRow::CopyFile,
            MenuRow::SaveAs,
            MenuRow::Star,
            MenuRow::Organize,
            MenuRow::Delete,
        ]
    } else if starred {
        &[
            MenuRow::Share,
            MenuRow::CopyImage,
            MenuRow::CopyFile,
            MenuRow::SaveAs,
            MenuRow::Unstar,
            MenuRow::Organize,
            MenuRow::Delete,
        ]
    } else {
        &[
            MenuRow::Share,
            MenuRow::CopyImage,
            MenuRow::CopyFile,
            MenuRow::SaveAs,
            MenuRow::Star,
            MenuRow::Organize,
            MenuRow::Delete,
        ]
    }
}

#[derive(Clone, Copy)]
pub struct OpenTarget {
    pub id: StickerId,
    pub animated: bool,
    pub source: ImageFormat,
}

#[derive(Clone)]
pub struct Target {
    pub id: StickerId,
    pub generation: Generation,
    pub revision: Revision,
    pub animated: bool,
    pub starred: bool,
    pub deleted: bool,
    pub source: ImageFormat,
}

#[derive(Clone)]
pub struct Host {
    pub library: Library,
    pub clipboard: Rc<RefCell<Hold>>,
    pub preset: Rc<Cell<ExportPreset>>,
    pub toasts: adw::ToastOverlay,
    pub navigation: adw::NavigationView,
    pub open_id: Rc<Cell<Option<OpenTarget>>>,
    pub detail_sync: DetailRefresh,
    pub menu: Rc<RefCell<Option<gtk4::Popover>>>,
    pub show_trash: Rc<dyn Fn()>,
}

pub fn popup(host: &Host, anchor: &gtk4::Widget, target: Target) {
    dismiss(&host.menu);
    let popover = gtk4::Popover::new();
    popover.set_parent(anchor);
    popover.set_position(gtk4::PositionType::Bottom);
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    column.set_margin_top(6);
    column.set_margin_bottom(6);
    let flattens = output::flattens(target.animated, host.preset.get());
    for row in menu_rows(target.deleted, target.starred, flattens) {
        column.append(&row_button(host, &popover, &target, *row, flattens));
    }
    popover.set_child(Some(&column));
    let menu = Rc::clone(&host.menu);
    popover.connect_closed(move |popover| {
        if popover.parent().is_some() {
            popover.unparent();
        }
        let current = menu.borrow().clone();
        if current.as_ref() == Some(popover) {
            menu.borrow_mut().take();
        }
    });
    host.menu.borrow_mut().replace(popover.clone());
    popover.popup();
}

pub fn dismiss(menu: &RefCell<Option<gtk4::Popover>>) {
    let Some(popover) = menu.borrow_mut().take() else {
        return;
    };
    popover.popdown();
    if popover.parent().is_some() {
        popover.unparent();
    }
}

fn row_button(
    host: &Host,
    popover: &gtk4::Popover,
    target: &Target,
    row: MenuRow,
    flattens: bool,
) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.add_css_class("flat");
    button.set_hexpand(true);
    if row == MenuRow::Share && flattens {
        let column = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let title = gtk4::Label::new(Some(label(row)));
        title.set_halign(gtk4::Align::Start);
        let hint = gtk4::Label::new(Some(i18n::text(Key::QuickAnimatedOriginal)));
        hint.set_halign(gtk4::Align::Start);
        hint.set_wrap(true);
        hint.add_css_class("caption");
        hint.add_css_class("dim-label");
        column.append(&title);
        column.append(&hint);
        button.set_child(Some(&column));
    } else {
        button.set_label(label(row));
    }
    if row == MenuRow::Delete {
        button.add_css_class("destructive-action");
    }
    let host = host.clone();
    let target = target.clone();
    let popover = popover.clone();
    button.connect_clicked(move |_| {
        popover.popdown();
        run(&host, &target, row);
    });
    button
}

fn label(row: MenuRow) -> &'static str {
    match row {
        MenuRow::Share => i18n::text(Key::Share),
        MenuRow::ShareFirstFrame => i18n::text(Key::ShareFirstFrame),
        MenuRow::CopyImage => i18n::text(Key::CopyImage),
        MenuRow::CopyFile => i18n::text(Key::CopyFile),
        MenuRow::SaveAs => i18n::text(Key::SaveAs),
        MenuRow::Star => i18n::text(Key::Favorite),
        MenuRow::Unstar => i18n::text(Key::Unfavorite),
        MenuRow::Organize => i18n::text(Key::Organize),
        MenuRow::Delete => i18n::text(Key::Delete),
        MenuRow::Restore => i18n::text(Key::Restore),
    }
}

fn run(host: &Host, target: &Target, row: MenuRow) {
    match row {
        MenuRow::Share => {
            output::spawn_share(
                host.library.clone(),
                Rc::clone(&host.clipboard),
                &host.navigation,
                request(host, target, false),
                notify(host),
            );
        }
        MenuRow::ShareFirstFrame => {
            output::spawn_share(
                host.library.clone(),
                Rc::clone(&host.clipboard),
                &host.navigation,
                request(host, target, true),
                notify(host),
            );
        }
        MenuRow::CopyImage => {
            output::spawn_copy(
                host.library.clone(),
                Rc::clone(&host.clipboard),
                &host.navigation,
                request(host, target, false),
                CopyKind::Image,
                notify(host),
            );
        }
        MenuRow::CopyFile => {
            output::spawn_copy(
                host.library.clone(),
                Rc::clone(&host.clipboard),
                &host.navigation,
                request(host, target, false),
                CopyKind::File,
                notify(host),
            );
        }
        MenuRow::SaveAs => {
            output::spawn_save(
                host.library.clone(),
                Rc::clone(&host.clipboard),
                &host.navigation,
                request(host, target, false),
                notify(host),
            );
        }
        MenuRow::Star | MenuRow::Unstar => star(host, target),
        MenuRow::Organize => organize(host, target),
        MenuRow::Delete => confirm_delete(host, target),
        MenuRow::Restore => confirm_restore(host, target),
    }
}

fn request(host: &Host, target: &Target, first_frame: bool) -> Request {
    Request {
        id: target.id,
        animated: target.animated,
        preset: host.preset.get(),
        first_frame,
        source: target.source,
    }
}

fn notify(host: &Host) -> impl Fn(Option<&str>) + 'static {
    let toasts = host.toasts.clone();
    move |message| {
        if let Some(message) = message {
            toasts.add_toast(adw::Toast::new(message));
        }
    }
}

fn star(host: &Host, target: &Target) {
    let Ok(patch) = StickerPatch::new(
        FieldPatch::Missing,
        FieldPatch::Missing,
        FieldPatch::Set(!target.starred),
    ) else {
        toast(host, i18n::text(Key::FailureUnknown));
        return;
    };
    let host = host.clone();
    let id = target.id;
    let generation = target.generation;
    glib::spawn_future_local(async move {
        let saved = match host.library.patch_sticker(id, generation, patch) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        match saved {
            Ok(_) => {
                if host.open_id.get().is_some_and(|open| open.id == id)
                    && let Some(sync) = host.detail_sync.borrow().as_ref().map(Rc::clone)
                {
                    sync();
                }
            }
            Err(error) => toast(&host, i18n::core(error.code())),
        }
    });
}

fn organize(host: &Host, target: &Target) {
    let host = host.clone();
    let id = target.id;
    let toasts = host.toasts.clone();
    detail::open_organize(
        host.navigation.upcast_ref::<gtk4::Widget>(),
        host.library.clone(),
        id,
        move |message| {
            toasts.add_toast(adw::Toast::new(message));
        },
        move || {
            if host.open_id.get().is_some_and(|open| open.id == id)
                && let Some(sync) = host.detail_sync.borrow().as_ref().map(Rc::clone)
            {
                sync();
            }
        },
    );
}

fn confirm_delete(host: &Host, target: &Target) {
    let navigation = host.navigation.clone();
    let host = host.clone();
    let target = target.clone();
    detail::confirm(
        &navigation,
        i18n::text(Key::Delete),
        i18n::text(Key::DeleteHint),
        i18n::text(Key::Delete),
        true,
        move || delete(host.clone(), target.clone()),
    );
}

fn confirm_restore(host: &Host, target: &Target) {
    let navigation = host.navigation.clone();
    let host = host.clone();
    let target = target.clone();
    detail::confirm(
        &navigation,
        i18n::text(Key::RestoreStickerTitle),
        i18n::text(Key::RestoreHint),
        i18n::text(Key::Restore),
        false,
        move || restore(host.clone(), target.clone()),
    );
}

fn delete(host: Host, target: Target) {
    glib::spawn_future_local(async move {
        let deleted = match host.library.delete_sticker(target.id, target.generation) {
            Ok(task) => task.wait().await,
            Err(error) => Err(error),
        };
        match deleted {
            Ok(_) => {
                close_detail(&host, target.id);
                let toast = adw::Toast::new(i18n::text(Key::StickerDeleted));
                toast.set_button_label(Some(i18n::text(Key::View)));
                let show_trash = Rc::clone(&host.show_trash);
                toast.connect_button_clicked(move |_| show_trash());
                host.toasts.add_toast(toast);
            }
            Err(error) => toast(&host, i18n::core(error.code())),
        }
    });
}

fn restore(host: Host, target: Target) {
    glib::spawn_future_local(async move {
        let restored =
            match host
                .library
                .restore_sticker(target.id, target.generation, target.revision)
            {
                Ok(task) => task.wait().await,
                Err(error) => Err(error),
            };
        match restored {
            Ok(_) => {
                close_detail(&host, target.id);
                toast(&host, i18n::text(Key::Restored));
            }
            Err(error) => toast(&host, i18n::core(error.code())),
        }
    });
}

fn close_detail(host: &Host, id: StickerId) {
    if host.open_id.get().is_some_and(|open| open.id == id) {
        host.navigation.pop();
    }
}

fn toast(host: &Host, message: &str) {
    host.toasts.add_toast(adw::Toast::new(message));
}

#[cfg(test)]
mod tests {
    use super::{MenuRow, menu_rows};

    #[test]
    fn trash_only_restores_and_a_library_item_lists_the_desktop_actions() {
        assert_eq!(menu_rows(true, false, false), &[MenuRow::Restore]);
        assert_eq!(menu_rows(true, true, true), &[MenuRow::Restore]);
        assert_eq!(
            menu_rows(false, false, false),
            &[
                MenuRow::Share,
                MenuRow::CopyImage,
                MenuRow::CopyFile,
                MenuRow::SaveAs,
                MenuRow::Star,
                MenuRow::Organize,
                MenuRow::Delete,
            ]
        );
        assert_eq!(
            menu_rows(false, true, false),
            &[
                MenuRow::Share,
                MenuRow::CopyImage,
                MenuRow::CopyFile,
                MenuRow::SaveAs,
                MenuRow::Unstar,
                MenuRow::Organize,
                MenuRow::Delete,
            ]
        );
        assert_eq!(
            menu_rows(false, false, true),
            &[
                MenuRow::Share,
                MenuRow::ShareFirstFrame,
                MenuRow::CopyImage,
                MenuRow::CopyFile,
                MenuRow::SaveAs,
                MenuRow::Star,
                MenuRow::Organize,
                MenuRow::Delete,
            ]
        );
    }
}
