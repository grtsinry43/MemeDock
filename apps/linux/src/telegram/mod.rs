mod preview;
mod state;
use crate::{
    credentials::Store,
    i18n::{self, Key},
    settings::Host,
};
use gtk4::{gio, glib, prelude::*};
use libadwaita::{self as adw, prelude::*};
use memedock_core::{
    sources::telegram::{
        TelegramImportController, TelegramImportOutcome, TelegramImportReport, TelegramPack,
        TelegramSticker,
    },
    tasks::TaskController,
};
use memedock_domain::source::{SourceImportState, SourceItemId};
use state::Selection;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc, time::Duration};
struct Model {
    alive: bool,
    credentials_busy: bool,
    credentials_failed: bool,
    selection: Selection,
    pack: Option<Arc<TelegramPack>>,
    stickers: BTreeMap<SourceItemId, TelegramSticker>,
    previews: Option<Rc<RefCell<preview::Queue>>>,
    credentials: Option<Rc<Store>>,
    loading: Option<TaskController>,
    importing: Option<TelegramImportController>,
    cells: BTreeMap<SourceItemId, glib::WeakRef<gtk4::Box>>,
    report: Option<TelegramImportReport>,
    error: Option<memedock_core::ErrorCode>,
    stopping: bool,
}
struct View {
    dialog: adw::Dialog,
    input: gtk4::Box,
    token: gtk4::PasswordEntry,
    name: gtk4::Entry,
    remember: gtk4::CheckButton,
    title: gtk4::Label,
    status: gtk4::Label,
    load: gtk4::Button,
    import: gtk4::Button,
    change: gtk4::Button,
    all: gtk4::Button,
    clear: gtk4::Button,
    cancel: gtk4::Button,
    forget: gtk4::Button,
    grid: gtk4::GridView,
    store: gio::ListStore,
}
fn new_view() -> Rc<View> {
    let dialog = adw::Dialog::new();
    i18n::bind(&dialog, "title", Key::Telegram);
    dialog.set_content_width(700);
    dialog.set_content_height(640);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    let input = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    input.set_margin_start(20);
    input.set_margin_end(20);
    let token = gtk4::PasswordEntry::new();
    token.set_show_peek_icon(true);
    i18n::bind(&token, "placeholder-text", Key::Token);
    let hint = i18n::label(Key::TokenHint);
    hint.set_wrap(true);
    hint.add_css_class("dim-label");
    let name = gtk4::Entry::new();
    i18n::bind(&name, "placeholder-text", Key::StickerPack);
    let remember = gtk4::CheckButton::new();
    i18n::bind(&remember, "label", Key::RememberToken);
    let forget = i18n::button(Key::ClearToken);
    let token_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    remember.set_hexpand(true);
    token_actions.append(&remember);
    token_actions.append(&forget);
    let load = i18n::button(Key::ViewStickers);
    load.add_css_class("suggested-action");
    input.append(&token);
    input.append(&hint);
    input.append(&token_actions);
    input.append(&name);
    input.append(&load);
    body.append(&input);
    let title = i18n::label(Key::ChooseStickers);
    title.set_wrap(true);
    title.set_margin_start(20);
    title.set_margin_end(20);
    title.add_css_class("title-2");
    body.append(&title);
    let all = i18n::button(Key::SelectNew);
    let clear = i18n::button(Key::ClearSelection);
    let change = i18n::button(Key::ChangePack);
    let tools = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    tools.set_margin_start(12);
    tools.set_margin_end(12);
    tools.append(&all);
    tools.append(&clear);
    tools.append(&change);
    body.append(&tools);
    let store = gio::ListStore::new::<gtk4::StringObject>();
    let grid = gtk4::GridView::new(
        Some(gtk4::NoSelection::new(Some(store.clone()))),
        None::<gtk4::SignalListItemFactory>,
    );
    grid.set_min_columns(2);
    grid.set_max_columns(8);
    grid.set_vexpand(true);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&grid));
    scroll.set_vexpand(true);
    body.append(&scroll);
    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    footer.set_margin_start(16);
    footer.set_margin_end(16);
    footer.set_margin_bottom(12);
    let status = gtk4::Label::new(None);
    status.set_wrap(true);
    status.set_hexpand(true);
    status.set_xalign(0.0);
    let import = i18n::button(Key::ImportSelected);
    import.add_css_class("suggested-action");
    let cancel = i18n::button(Key::Cancel);
    footer.append(&status);
    footer.append(&cancel);
    footer.append(&import);
    toolbar.add_bottom_bar(&footer);
    toolbar.set_content(Some(&body));
    dialog.set_child(Some(&toolbar));
    Rc::new(View {
        dialog: dialog.clone(),
        input,
        token,
        name,
        remember,
        title,
        status,
        load,
        import,
        change,
        all,
        clear,
        cancel,
        forget,
        grid,
        store,
    })
}
pub fn present(host: Host) {
    if host.maintenance.get() || (host.busy)() {
        return;
    }
    let view = new_view();
    let dialog = view.dialog.clone();
    let model = Rc::new(RefCell::new(Model {
        alive: true,
        credentials_busy: true,
        credentials_failed: false,
        selection: Selection::default(),
        pack: None,
        stickers: BTreeMap::new(),
        previews: None,
        credentials: None,
        loading: None,
        importing: None,
        cells: BTreeMap::new(),
        report: None,
        error: None,
        stopping: false,
    }));
    wire_cells(&view, &model);
    refresh(&view, &model);
    view.input.set_sensitive(false);
    let weak_view = Rc::downgrade(&view);
    let state = Rc::clone(&model);
    glib::spawn_future_local(async move {
        let result = async {
            let store = Rc::new(Store::open().await?);
            let token = store.load().await;
            Ok::<_, ()>((store, token))
        }
        .await;
        if let Some(view) = weak_view.upgrade() {
            match result {
                Ok((store, token)) => {
                    if let Ok(Some(token)) = &token {
                        view.token.set_text(token);
                        view.remember.set_active(true);
                    }
                    if store.is_file() {
                        i18n::bind(&view.status, "label", Key::FileTokenHint);
                    }
                    if token.is_err() {
                        state.borrow_mut().credentials_failed = true;
                        i18n::bind(&view.status, "label", Key::TokenFailed);
                    }
                    state.borrow_mut().credentials = Some(store);
                }
                Err(()) => {
                    state.borrow_mut().credentials_failed = true;
                    i18n::bind(&view.status, "label", Key::TokenFailed);
                }
            }
            view.input.set_sensitive(true);
            state.borrow_mut().credentials_busy = false;
            refresh(&view, &state);
        }
    });
    let weak_view = Rc::downgrade(&view);
    let state = Rc::clone(&model);
    let source = host.library.clone();
    view.load.connect_clicked(move |_| {
        let Some(view) = weak_view.upgrade() else {
            return;
        };
        if model_busy(&state) || view.token.text().is_empty() || view.name.text().is_empty() {
            return;
        }
        let task = match source.telegram_pack(
            view.token.text().trim().to_owned(),
            view.name.text().trim().to_owned(),
        ) {
            Ok(t) => t,
            Err(error) => {
                view.status.set_text(failure(error.code()));
                return;
            }
        };
        state.borrow_mut().loading = Some(task.controller());
        state.borrow_mut().selection.busy = true;
        state.borrow_mut().stopping = false;
        state.borrow_mut().error = None;
        refresh(&view, &state);
        i18n::bind(&view.status, "label", Key::Working);
        let state = Rc::clone(&state);
        let source = source.clone();
        glib::spawn_future_local(async move {
            let result = task.wait().await;
            if !state.borrow().alive {
                return;
            }
            match result {
                Ok(pack) => {
                    let credentials = state.borrow().credentials.clone();
                    if view.remember.is_active() {
                        if let Some(store) = credentials {
                            let failed = store
                                .save(view.token.text().trim().to_owned())
                                .await
                                .is_err();
                            state.borrow_mut().credentials_failed = failed;
                        } else {
                            state.borrow_mut().credentials_failed = true;
                        }
                    }
                    if !state.borrow().alive {
                        return;
                    }
                    let mut model = state.borrow_mut();
                    model.selection.states = pack
                        .stickers()
                        .iter()
                        .map(|s| (s.id.clone(), s.state))
                        .collect();
                    model.selection.selected.clear();
                    model.stickers = pack
                        .stickers()
                        .iter()
                        .map(|s| (s.id.clone(), s.clone()))
                        .collect();
                    model.report = None;
                    model.previews = Some(preview::Queue::new(source, Arc::clone(&pack)));
                    model.pack = Some(Arc::clone(&pack));
                    drop(model);
                    view.title.set_text(pack.title());
                    view.store.remove_all();
                    view.store.extend_from_slice(
                        &pack
                            .stickers()
                            .iter()
                            .map(|s| gtk4::StringObject::new(s.id.as_str()))
                            .collect::<Vec<_>>(),
                    );
                }
                Err(error) => {
                    state.borrow_mut().error = Some(error.code());
                    view.status.set_text(failure(error.code()));
                }
            }
            state.borrow_mut().loading = None;
            state.borrow_mut().selection.busy = false;
            state.borrow_mut().stopping = false;
            refresh(&view, &state);
        });
    });
    let weak_view = Rc::downgrade(&view);
    let state = Rc::clone(&model);
    let source = host.library.clone();
    view.import.connect_clicked(move |_| {
        let Some(view) = weak_view.upgrade() else {
            return;
        };
        if model_busy(&state) || state.borrow().selection.selected.is_empty() {
            return;
        }
        let pack = state.borrow().pack.clone();
        let Some(pack) = pack else {
            return;
        };
        let task = match source.import_telegram(
            pack,
            state.borrow().selection.selected.iter().cloned().collect(),
        ) {
            Ok(t) => t,
            Err(error) => {
                view.status.set_text(failure(error.code()));
                return;
            }
        };
        let controller = task.controller();
        state.borrow_mut().importing = Some(controller.clone());
        state.borrow_mut().selection.busy = true;
        state.borrow_mut().stopping = false;
        state.borrow_mut().error = None;
        refresh(&view, &state);
        let weak = Rc::downgrade(&view);
        let tick = glib::timeout_add_local(Duration::from_millis(150), move || {
            let Some(view) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let report = controller.snapshot();
            view.status.set_text(&i18n::import_running_progress(
                report
                    .items
                    .iter()
                    .filter(|i| i.outcome != TelegramImportOutcome::Pending)
                    .count(),
                report.items.len(),
            ));
            glib::ControlFlow::Continue
        });
        let state = Rc::clone(&state);
        glib::spawn_future_local(async move {
            let result = task.wait().await;
            tick.remove();
            if !state.borrow().alive {
                return;
            }
            match result {
                Ok(report) => {
                    state.borrow_mut().selection.finish(&report);
                    state.borrow_mut().report = Some(report);
                }
                Err(error) => {
                    state.borrow_mut().error = Some(error.code());
                    view.status.set_text(failure(error.code()));
                }
            }
            state.borrow_mut().importing = None;
            state.borrow_mut().selection.busy = false;
            state.borrow_mut().stopping = false;
            refresh(&view, &state);
        });
    });
    for (button, mode) in [(&view.all, 0), (&view.clear, 1), (&view.change, 2)] {
        let weak = Rc::downgrade(&view);
        let state = Rc::clone(&model);
        button.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            if model_busy(&state) {
                return;
            }
            let mut s = state.borrow_mut();
            s.report = None;
            s.error = None;
            if mode == 0 {
                s.selection.selected = s
                    .selection
                    .states
                    .iter()
                    .filter(|(_, v)| {
                        matches!(
                            v,
                            SourceImportState::Available | SourceImportState::OriginalMissing
                        )
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
            } else {
                s.selection.selected.clear();
            }
            if mode == 2 {
                if let Some(q) = s.previews.take() {
                    preview::Queue::stop(&q);
                }
                s.pack = None;
                s.stickers.clear();
                s.cells.clear();
            }
            drop(s);
            if mode == 2 {
                view.store.remove_all();
            }
            refresh(&view, &state);
        });
    }
    let state = Rc::clone(&model);
    view.cancel.connect_clicked(move |_| cancel_work(&state));
    let state = Rc::clone(&model);
    let weak = Rc::downgrade(&view);
    view.forget.connect_clicked(move |_| {
        clear_token(&state, &weak, true);
    });
    let state = Rc::clone(&model);
    let weak = Rc::downgrade(&view);
    view.remember.connect_toggled(move |button| {
        if !button.is_active() {
            clear_token(&state, &weak, false);
        }
    });
    let state = Rc::clone(&model);
    dialog.connect_close_attempt(move |dialog| {
        let state = Rc::clone(&state);
        let weak = dialog.downgrade();
        crate::detail::confirm(
            dialog,
            i18n::text(Key::LeaveImport),
            i18n::text(Key::LeaveImportHint),
            i18n::text(Key::Done),
            false,
            move || {
                cancel_work(&state);
                if let Some(d) = weak.upgrade() {
                    d.force_close();
                }
            },
        );
    });
    let state = Rc::clone(&model);
    dialog.connect_closed(move |_| {
        cancel_work(&state);
        state.borrow_mut().alive = false;
        if let Some(queue) = state.borrow_mut().previews.take() {
            preview::Queue::stop(&queue);
        }
        state.borrow_mut().pack = None;
        state.borrow_mut().stickers.clear();
    });
    let weak = Rc::downgrade(&view);
    let state = Rc::clone(&model);
    i18n::on_language(&dialog, move || {
        if let Some(view) = weak.upgrade() {
            refresh(&view, &state);
        }
    });
    let keep = Rc::new(RefCell::new(Some(view)));
    dialog.connect_closed(move |_| {
        keep.borrow_mut().take();
    });
    dialog.present(Some(&host.parent));
}
fn clear_token(model: &Rc<RefCell<Model>>, weak: &std::rc::Weak<View>, clear_input: bool) {
    if model_busy(model) {
        return;
    }
    let store = model.borrow().credentials.clone();
    let weak = weak.clone();
    model.borrow_mut().credentials_busy = true;
    let model = Rc::clone(model);
    if let Some(v) = weak.upgrade() {
        v.input.set_sensitive(false);
        if clear_input {
            v.token.set_text("");
            v.remember.set_active(false);
        }
    }
    glib::spawn_future_local(async move {
        let result = async {
            let store = match store {
                Some(store) => store,
                None => Rc::new(Store::open().await?),
            };
            store.clear().await?;
            Ok::<_, ()>(store)
        }
        .await;
        {
            let mut s = model.borrow_mut();
            s.credentials_busy = false;
            s.credentials_failed = result.is_err();
            if let Ok(store) = result {
                s.credentials = Some(store);
            }
        }
        if let Some(v) = weak.upgrade() {
            refresh(&v, &model);
            if model.borrow().credentials_failed {
                i18n::bind(&v.status, "label", Key::TokenFailed);
            } else if model.borrow().pack.is_none() {
                v.status.set_text("");
            }
        }
    });
}
fn model_busy(model: &Rc<RefCell<Model>>) -> bool {
    let s = model.borrow();
    s.credentials_busy || s.selection.busy || s.loading.is_some() || s.importing.is_some()
}
fn cancel_work(model: &Rc<RefCell<Model>>) {
    let mut s = model.borrow_mut();
    s.stopping = true;
    if let Some(t) = &s.loading {
        t.cancel();
    }
    if let Some(t) = &s.importing {
        let _ = t.cancel();
    }
}
fn refresh(view: &View, model: &Rc<RefCell<Model>>) {
    let busy = model_busy(model);
    // GTK property changes can synchronously bind/unbind grid cells. Release
    // state before touching widgets, including visibility and sensitivity.
    let (packed, selected, stopping, status, cells) = {
        let s = model.borrow();
        let packed = !s.stickers.is_empty();
        let mut status = if let Some(code) = s.error {
            Some(failure(code).to_owned())
        } else if packed && !busy {
            Some(
                s.report
                    .as_ref()
                    .map(summary)
                    .unwrap_or_else(|| i18n::selection_count(s.selection.selected.len())),
            )
        } else {
            None
        };
        if s.credentials_failed {
            status = Some(match status {
                Some(status) => format!("{}\n{status}", i18n::text(Key::TokenFailed)),
                None => i18n::text(Key::TokenFailed).to_owned(),
            });
        }
        (
            packed,
            s.selection.selected.len(),
            s.stopping,
            status,
            s.cells
                .iter()
                .map(|(id, cell)| (id.clone(), cell.clone()))
                .collect::<Vec<_>>(),
        )
    };
    view.dialog.set_can_close(!busy);
    view.input.set_visible(!packed);
    view.input.set_sensitive(!busy);
    view.grid.set_visible(packed);
    view.all.set_visible(packed);
    view.clear.set_visible(packed);
    view.change.set_visible(packed);
    view.import.set_visible(packed);
    view.all.set_sensitive(!busy);
    view.clear.set_sensitive(!busy);
    view.change.set_sensitive(!busy);
    view.import.set_sensitive(!busy && selected != 0);
    view.load.set_sensitive(!busy);
    view.cancel.set_visible(busy);
    view.cancel.set_sensitive(!stopping);
    if let Some(status) = status {
        view.status.set_text(&status);
    }
    for (id, weak) in cells {
        if let Some(cell) = weak.upgrade() {
            update_cell(&cell, &id, model);
        }
    }
}
fn summary(report: &TelegramImportReport) -> String {
    let added = report
        .items
        .iter()
        .filter(|i| i.outcome == TelegramImportOutcome::Created)
        .count();
    let failed = report
        .items
        .iter()
        .filter(|i| matches!(i.outcome, TelegramImportOutcome::Failed(_)))
        .count();
    let title = i18n::text(if report.stopped {
        Key::ImportStopped
    } else {
        Key::ImportFinished
    });
    let existing = report
        .items
        .iter()
        .filter(|i| i.outcome == TelegramImportOutcome::Reused)
        .count();
    let restore = report
        .items
        .iter()
        .filter(|i| i.outcome == TelegramImportOutcome::RestoreRequired)
        .count();
    let mut summary = match i18n::language() {
        i18n::Language::Zh => format!(
            "{title}\n新增 {added} 张 · 已有 {existing} 张 · 失败 {failed} 张 · 待恢复 {restore} 张"
        ),
        i18n::Language::En => format!(
            "{title}\nAdded: {added} · Existing: {existing} · Failed: {failed} · In trash: {restore}"
        ),
    };
    if let Some(code) = report.items.iter().find_map(|i| {
        if let TelegramImportOutcome::Failed(code) = i.outcome {
            Some(code)
        } else {
            None
        }
    }) {
        summary.push_str(&format!("\n{}", failure(code)));
    }
    summary
}
fn failure(code: memedock_core::ErrorCode) -> &'static str {
    match code {
        memedock_core::ErrorCode::Network | memedock_core::ErrorCode::Timeout => {
            i18n::text(Key::NetworkFailed)
        }
        memedock_core::ErrorCode::Unauthorized => i18n::text(Key::TokenInvalid),
        memedock_core::ErrorCode::RateLimited => i18n::text(Key::RateLimited),
        memedock_core::ErrorCode::NotFound => i18n::text(Key::PackMissing),
        _ => i18n::core(code),
    }
}
fn parts(cell: &gtk4::Box) -> Option<(gtk4::Picture, gtk4::CheckButton, gtk4::Label)> {
    let p = cell.first_child()?.downcast::<gtk4::Picture>().ok()?;
    let c = p.next_sibling()?.downcast::<gtk4::CheckButton>().ok()?;
    let l = c.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    Some((p, c, l))
}
fn update_cell(cell: &gtk4::Box, id: &SourceItemId, state: &Rc<RefCell<Model>>) {
    let Some((_, check, label)) = parts(cell) else {
        return;
    };
    let (sensitive, active, text) = {
        let model = state.borrow();
        let sensitive = !model.selection.busy
            && model.selection.states.get(id) != Some(&SourceImportState::RestoreRequired);
        let active = model.selection.selected.contains(id);
        let status = match model.selection.states.get(id) {
            Some(SourceImportState::Imported) => Key::AlreadyImported,
            Some(SourceImportState::RestoreRequired) => Key::InTrash,
            Some(SourceImportState::OriginalMissing) => Key::DownloadAgain,
            _ => Key::ChooseStickers,
        };
        let failed = model
            .report
            .as_ref()
            .and_then(|r| r.items.iter().find(|i| &i.id == id))
            .is_some_and(|i| matches!(i.outcome, TelegramImportOutcome::Failed(_)));
        let text = if failed {
            i18n::text(Key::ImportFailed)
        } else if status == Key::ChooseStickers {
            ""
        } else {
            i18n::text(status)
        };
        (sensitive, active, text)
    };
    check.set_sensitive(sensitive);
    check.set_active(active);
    label.set_text(text);
}
fn wire_cells(view: &Rc<View>, model: &Rc<RefCell<Model>>) {
    let factory = gtk4::SignalListItemFactory::new();
    let state = Rc::clone(model);
    let weak_view = Rc::downgrade(view);
    factory.connect_setup(move |_, object| {
        let Some(item) = object.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let cell = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        cell.set_margin_start(4);
        cell.set_margin_end(4);
        let picture = gtk4::Picture::new();
        picture.set_size_request(96, 96);
        picture.set_content_fit(gtk4::ContentFit::Contain);
        let check = gtk4::CheckButton::new();
        let label = gtk4::Label::new(None);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        cell.append(&picture);
        cell.append(&check);
        cell.append(&label);
        item.set_child(Some(&cell));
        let state = Rc::clone(&state);
        let weak = weak_view.clone();
        check.connect_toggled(move |check| {
            let Ok(id) = SourceItemId::new(check.widget_name().to_string()) else {
                return;
            };
            {
                let mut s = state.borrow_mut();
                // Programmatic updates already match state; don't refresh the
                // grid recursively while its factory is binding a cell.
                if s.selection.selected.contains(&id) == check.is_active() {
                    return;
                }
                s.selection.toggle(id, check.is_active());
            }
            if let Some(view) = weak.upgrade() {
                refresh(&view, &state);
            }
        });
    });
    let state = Rc::clone(model);
    factory.connect_bind(move |_, object| {
        let Some(item) = object.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let Some(id) = item
            .item()
            .and_downcast::<gtk4::StringObject>()
            .and_then(|o| SourceItemId::new(o.string().to_string()).ok())
        else {
            return;
        };
        let Some(cell) = item.child().and_downcast::<gtk4::Box>() else {
            return;
        };
        let Some((picture, check, _)) = parts(&cell) else {
            return;
        };
        let sticker = state.borrow().stickers.get(&id).cloned();
        let Some(sticker) = sticker else {
            return;
        };
        check.set_widget_name(id.as_str());
        check.set_label(Some(
            sticker
                .emoji
                .as_deref()
                .unwrap_or(i18n::text(Key::TabStickers)),
        ));
        update_cell(&cell, &id, &state);
        let queue = {
            let mut s = state.borrow_mut();
            s.cells.insert(id.clone(), cell.downgrade());
            s.previews.clone()
        };
        if sticker.has_preview
            && let Some(queue) = queue
        {
            preview::Queue::request(&queue, id, &picture);
        }
    });
    let state = Rc::clone(model);
    factory.connect_unbind(move |_, object| {
        let Some(item) = object.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let Some(cell) = item.child().and_downcast::<gtk4::Box>() else {
            return;
        };
        let Some((picture, check, _)) = parts(&cell) else {
            return;
        };
        if let Ok(id) = SourceItemId::new(check.widget_name().to_string()) {
            let queue = {
                let mut s = state.borrow_mut();
                s.cells.remove(&id);
                s.previews.clone()
            };
            if let Some(queue) = queue {
                preview::Queue::cancel(&queue, &id);
            }
        }
        picture.set_paintable(None::<&gtk4::gdk::Texture>);
        picture.set_tooltip_text(None);
        check.set_widget_name("");
    });
    view.grid.set_factory(Some(&factory));
}

#[cfg(test)]
pub(crate) fn verify_grid_reentrancy(
    context: &glib::MainContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let view = new_view();
    let model = Rc::new(RefCell::new(Model {
        alive: true,
        credentials_busy: false,
        credentials_failed: false,
        selection: Selection::default(),
        pack: None,
        stickers: BTreeMap::new(),
        previews: None,
        credentials: None,
        loading: None,
        importing: None,
        cells: BTreeMap::new(),
        report: None,
        error: None,
        stopping: false,
    }));
    wire_cells(&view, &model);
    view.grid.set_visible(false);
    let unbound = Rc::new(std::cell::Cell::new(0));
    let counted = Rc::clone(&unbound);
    let factory = view
        .grid
        .factory()
        .and_downcast::<gtk4::SignalListItemFactory>()
        .ok_or("missing grid factory")?;
    factory.connect_unbind(move |_, _| counted.set(counted.get() + 1));
    for index in 0..60 {
        let id = SourceItemId::new(format!("sticker_{index}"))?;
        model.borrow_mut().stickers.insert(
            id.clone(),
            TelegramSticker {
                id,
                emoji: None,
                width: 512,
                height: 512,
                format: memedock_core::sources::telegram::TelegramFormat::Static,
                state: SourceImportState::Available,
                has_preview: false,
            },
        );
    }
    view.store.extend_from_slice(
        &(0..60)
            .map(|id| gtk4::StringObject::new(&format!("sticker_{id}")))
            .collect::<Vec<_>>(),
    );
    let window = adw::Window::new();
    window.set_default_size(720, 700);
    window.present();
    view.dialog.present(Some(&window));
    context.block_on(glib::timeout_future(Duration::from_millis(100)));
    // Showing the populated grid calls bind synchronously from set_visible.
    // Previously refresh held model.borrow() across that call and aborted.
    refresh(&view, &model);
    context.block_on(glib::timeout_future(Duration::from_millis(100)));
    assert!(view.grid.first_child().is_some(), "grid must realize cells");
    let (id, cell) = model
        .borrow()
        .cells
        .iter()
        .find_map(|(id, weak)| weak.upgrade().map(|cell| (id.clone(), cell)))
        .ok_or("no bound cells")?;
    let (_, check, _) = parts(&cell).ok_or("missing cell controls")?;
    check.set_active(true);
    assert!(model.borrow().selection.selected.contains(&id));
    model.borrow_mut().selection.selected.clear();
    refresh(&view, &model);
    assert!(!check.is_active());
    // Hiding a mapped grid synchronously calls the real factory's unbind
    // handler. This used to panic while refresh held model.borrow().
    model.borrow_mut().stickers.clear();
    refresh(&view, &model);
    view.store.remove_all();
    assert!(unbound.get() > 0, "exercise actual factory unbinding");
    view.dialog.force_close();
    window.destroy();
    Ok(())
}
