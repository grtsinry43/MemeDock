use super::{Coordinator, State};
use crate::i18n::{self, Key};
use gtk4::{glib, prelude::*};
use libadwaita::{self as adw, prelude::*};
use std::{cell::Cell, rc::Rc};

struct View {
    dialog: adw::Dialog,
    origin: adw::EntryRow,
    token: adw::PasswordEntryRow,
    authorize: adw::PreferencesGroup,
    connect: gtk4::Button,
    status: adw::ActionRow,
    pending: adw::ActionRow,
    blocked: adw::ActionRow,
    last: adw::ActionRow,
    now: gtk4::Button,
    pause: gtk4::Button,
    reauthorize: gtk4::Button,
    disconnect: gtk4::Button,
    error: gtk4::Label,
    fallback: gtk4::Label,
    changing: Cell<bool>,
}
impl View {
    fn update(&self, state: &State) {
        let authorizing = self.changing.get() || state.needs_authorization();
        self.authorize.set_visible(authorizing);
        self.origin.set_sensitive(!state.busy);
        self.token.set_sensitive(!state.busy);
        self.connect.set_sensitive(
            !state.busy
                && !state.suspended
                && !self.origin.text().is_empty()
                && !self.token.text().is_empty(),
        );
        self.status.set_subtitle(i18n::text(if state.busy {
            Key::SyncRunning
        } else if state.needs_authorization() {
            Key::SyncAuthorize
        } else if state.status.paused {
            Key::SyncPaused
        } else {
            Key::SyncReady
        }));
        self.pending.set_subtitle(&state.status.pending.to_string());
        self.blocked.set_subtitle(&state.status.blocked.to_string());
        let last = state
            .status
            .last_success_ms
            .and_then(|time| glib::DateTime::from_unix_local(time / 1000).ok())
            .and_then(|time| time.format("%x %X").ok())
            .map_or_else(|| i18n::text(Key::SyncNever).into(), |s| s.to_string());
        self.last.set_subtitle(&last);
        self.now.set_visible(!authorizing && !state.status.paused);
        self.pause.set_visible(!authorizing);
        self.pause.set_label(i18n::text(if state.status.paused {
            Key::SyncResume
        } else {
            Key::SyncPause
        }));
        self.reauthorize
            .set_visible(state.status.connected && !authorizing);
        self.disconnect
            .set_visible(state.status.connected && state.credential);
        for button in [&self.now, &self.pause, &self.reauthorize, &self.disconnect] {
            button.set_sensitive(!state.busy && !state.suspended);
        }
        self.error
            .set_text(state.error.map(i18n::text).unwrap_or_else(|| {
                if state.status.blocked > 0 {
                    i18n::text(Key::SyncBlockedHint)
                } else {
                    ""
                }
            }));
        self.error.set_visible(!self.error.text().is_empty());
        self.fallback.set_visible(state.file_credential);
    }
}

pub fn present(parent: &impl IsA<gtk4::Widget>, coordinator: Rc<Coordinator>) -> adw::Dialog {
    let dialog = adw::Dialog::new();
    i18n::bind(&dialog, "title", Key::Sync);
    dialog.set_content_width(520);
    dialog.set_content_height(620);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let scroll = gtk4::ScrolledWindow::new();
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    body.set_margin_top(16);
    body.set_margin_bottom(20);
    body.set_margin_start(20);
    body.set_margin_end(20);
    let authorize = adw::PreferencesGroup::new();
    i18n::bind(&authorize, "description", Key::SyncHint);
    let origin = adw::EntryRow::new();
    i18n::bind(&origin, "title", Key::SyncServer);
    origin.set_widget_name("sync-server");
    origin.set_text(
        coordinator
            .snapshot()
            .status
            .origin
            .as_deref()
            .unwrap_or(""),
    );
    let token = adw::PasswordEntryRow::new();
    i18n::bind(&token, "title", Key::SyncToken);
    token.set_widget_name("sync-token");
    authorize.add(&origin);
    authorize.add(&token);
    let connect = i18n::button(Key::SyncConnect);
    connect.set_widget_name("sync-connect");
    connect.add_css_class("suggested-action");
    authorize.add(&connect);
    body.append(&authorize);
    let group = adw::PreferencesGroup::new();
    let status = crate::settings::value_row(Key::Sync);
    let pending = crate::settings::value_row(Key::SyncPending);
    let blocked = crate::settings::value_row(Key::SyncBlocked);
    let last = crate::settings::value_row(Key::SyncLastSuccess);
    for row in [&status, &pending, &blocked, &last] {
        group.add(row);
    }
    body.append(&group);
    let now = i18n::button(Key::SyncNow);
    now.set_widget_name("sync-now");
    now.add_css_class("suggested-action");
    let pause = i18n::button(Key::SyncPause);
    let reauthorize = i18n::button(Key::SyncReauthorize);
    let disconnect = i18n::button(Key::SyncDisconnect);
    disconnect.add_css_class("destructive-action");
    for button in [&now, &pause, &reauthorize, &disconnect] {
        body.append(button);
    }
    let error = gtk4::Label::new(None);
    error.set_wrap(true);
    error.add_css_class("error");
    let fallback = i18n::label(Key::FileTokenHint);
    fallback.set_wrap(true);
    body.append(&error);
    body.append(&fallback);
    let hint = i18n::label(Key::SyncMergeHint);
    hint.set_wrap(true);
    hint.add_css_class("caption");
    body.append(&hint);
    scroll.set_child(Some(&body));
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    let view = Rc::new(View {
        dialog: dialog.clone(),
        origin,
        token,
        authorize,
        connect,
        status,
        pending,
        blocked,
        last,
        now,
        pause,
        reauthorize,
        disconnect,
        error,
        fallback,
        changing: Cell::new(false),
    });
    let weak = Rc::downgrade(&view);
    let source = Rc::clone(&coordinator);
    view.connect.connect_clicked(move |_| {
        if let Some(view) = weak.upgrade() {
            let token = view.token.text().to_string();
            view.token.set_text("");
            view.changing.set(false);
            source.connect(view.origin.text().to_string(), token);
        }
    });
    for entry in [
        view.origin.clone().upcast::<gtk4::Widget>(),
        view.token.clone().upcast(),
    ] {
        let weak = Rc::downgrade(&view);
        let source = Rc::clone(&coordinator);
        entry.connect_notify_local(Some("text"), move |_, _| {
            if let Some(view) = weak.upgrade() {
                view.update(&source.snapshot());
            }
        });
    }
    let source = Rc::clone(&coordinator);
    view.now.connect_clicked(move |_| source.synchronize());
    let source = Rc::clone(&coordinator);
    view.pause
        .connect_clicked(move |_| source.pause(!source.snapshot().status.paused));
    let source = Rc::clone(&coordinator);
    let weak = Rc::downgrade(&view);
    view.reauthorize.connect_clicked(move |_| {
        if let Some(view) = weak.upgrade() {
            view.changing.set(true);
            source.pause(true);
            view.update(&source.snapshot());
        }
    });
    let source = Rc::clone(&coordinator);
    let weak = Rc::downgrade(&view);
    view.disconnect.connect_clicked(move |_| {
        if let Some(view) = weak.upgrade() {
            let source = Rc::clone(&source);
            crate::detail::confirm(
                &view.dialog,
                i18n::text(Key::SyncDisconnect),
                i18n::text(Key::SyncDisconnectHint),
                i18n::text(Key::SyncDisconnect),
                true,
                move || source.disconnect(),
            );
        }
    });
    let weak = Rc::downgrade(&view);
    coordinator.observe(&dialog, move |state| {
        if let Some(view) = weak.upgrade() {
            view.update(state);
        }
    });
    let weak = Rc::downgrade(&view);
    let source = coordinator;
    i18n::on_language(&dialog, move || {
        if let Some(view) = weak.upgrade() {
            view.update(&source.snapshot());
        }
    });
    let keep = std::cell::RefCell::new(Some(view));
    dialog.connect_closed(move |_| {
        if let Some(view) = keep.borrow_mut().take() {
            view.token.set_text("");
        }
    });
    dialog.present(Some(parent));
    dialog
}
