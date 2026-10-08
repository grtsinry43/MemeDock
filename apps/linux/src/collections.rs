use crate::{
    detail,
    i18n::{self, Key},
};
use gtk4::{gdk, gio, glib, prelude::*};
use libadwaita::{self as adw, prelude::*};
use memedock_core::{CollectionSummary, Library};
use memedock_domain::{collection::Collection, identity::CollectionId, tag::Name};
use std::{cell::RefCell, rc::Rc};

type Notify = Rc<dyn Fn(&str)>;

pub fn open(
    navigation: &adw::NavigationView,
    library: Library,
    choose: Rc<dyn Fn(CollectionId)>,
    notify: Notify,
) {
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    let create = i18n::button(Key::NewCollection);
    header.pack_end(&create);
    content.append(&header);
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, value| {
        if let Some(item) = value.downcast_ref::<gtk4::ListItem>() {
            let card = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
            card.add_css_class("card");
            card.set_margin_start(6);
            card.set_margin_end(6);
            card.set_margin_top(6);
            card.set_margin_bottom(6);
            item.set_child(Some(&card));
        }
    });
    let source = library.clone();
    let message = Rc::clone(&notify);
    let weak_store = store.downgrade();
    factory.connect_bind(move |_, value| {
        let Some(item) = value.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let Some(row) = item
            .item()
            .and_then(|v| v.downcast::<glib::BoxedAnyObject>().ok())
        else {
            return;
        };
        let Some(card) = item.child().and_then(|v| v.downcast::<gtk4::Box>().ok()) else {
            return;
        };
        let summary = row.borrow::<CollectionSummary>();
        let collection = summary.collection.clone();
        let picture = gtk4::Picture::new();
        picture.set_content_fit(gtk4::ContentFit::Contain);
        picture.set_size_request(144, 144);
        picture.set_can_shrink(true);
        let cover = gtk4::Overlay::new();
        let placeholder = gtk4::Image::from_icon_name("folder-pictures-symbolic");
        placeholder.set_pixel_size(40);
        placeholder.set_visible(true);
        cover.set_child(Some(&picture));
        cover.add_overlay(&placeholder);
        card.append(&cover);
        let line = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        line.set_margin_start(12);
        line.set_margin_end(8);
        let title = gtk4::Label::new(Some(collection.name().as_str()));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title.add_css_class("heading");
        line.append(&title);
        let menu = gtk4::MenuButton::new();
        menu.set_icon_name("view-more-symbolic");
        let popup = gtk4::Popover::new();
        let buttons = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        let rename = i18n::button(Key::Rename);
        let parent = card.downgrade();
        let owner = source.clone();
        let feedback = Rc::clone(&message);
        let target = collection.clone();
        let weak_popup = popup.downgrade();
        rename.connect_clicked(move |_| {
            if let Some(p) = weak_popup.upgrade() {
                p.popdown();
            }
            if let Some(parent) = parent.upgrade() {
                ask_name(
                    &parent,
                    owner.clone(),
                    Some(target.clone()),
                    Rc::clone(&feedback),
                );
            }
        });
        buttons.append(&rename);
        let delete = i18n::button(Key::Delete);
        delete.add_css_class("destructive-action");
        let parent = card.downgrade();
        let owner = source.clone();
        let feedback = Rc::clone(&message);
        let target = collection.clone();
        let weak_popup = popup.downgrade();
        delete.connect_clicked(move |_| {
            if let Some(p) = weak_popup.upgrade() {
                p.popdown();
            }
            let Some(parent) = parent.upgrade() else {
                return;
            };
            let owner = owner.clone();
            let feedback = Rc::clone(&feedback);
            let target = target.clone();
            detail::confirm(
                &parent,
                i18n::text(Key::Delete),
                i18n::text(Key::DeleteHint),
                i18n::text(Key::Delete),
                true,
                move || {
                    let owner = owner.clone();
                    let feedback = Rc::clone(&feedback);
                    let target = target.clone();
                    glib::spawn_future_local(async move {
                        let result = match owner
                            .delete_collection(target.id(), target.lifecycle().generation())
                        {
                            Ok(t) => t.wait().await,
                            Err(e) => Err(e),
                        };
                        if let Err(e) = result {
                            feedback(i18n::core(e.code()));
                        }
                    });
                },
            );
        });
        buttons.append(&delete);
        for (key, direction) in [(Key::MoveUp, -1i64), (Key::MoveDown, 1)] {
            let button = i18n::button(key);
            let weak = weak_store.clone();
            let owner = source.clone();
            let feedback = Rc::clone(&message);
            let target = collection.clone();
            let weak_popup = popup.downgrade();
            button.connect_clicked(move |_| {
                if let Some(p) = weak_popup.upgrade() {
                    p.popdown();
                }
                let Some(store) = weak.upgrade() else {
                    return;
                };
                let ids: Vec<_> = (0..store.n_items())
                    .filter_map(|i| {
                        store
                            .item(i)
                            .and_then(|v| v.downcast::<glib::BoxedAnyObject>().ok())
                            .map(|v| v.borrow::<CollectionSummary>().collection.id())
                    })
                    .collect();
                let Some(position) = ids.iter().position(|v| *v == target.id()) else {
                    return;
                };
                let destination = position as i64 + direction;
                if destination < 0 || destination >= ids.len() as i64 {
                    return;
                }
                let before = if direction < 0 {
                    ids.get(destination as usize).copied()
                } else {
                    ids.get(destination as usize + 1).copied()
                };
                let owner = owner.clone();
                let feedback = Rc::clone(&feedback);
                let target = target.clone();
                glib::spawn_future_local(async move {
                    let result = match owner.move_collection(
                        target.id(),
                        target.lifecycle().generation(),
                        before,
                    ) {
                        Ok(t) => t.wait().await,
                        Err(e) => Err(e),
                    };
                    if let Err(e) = result {
                        feedback(i18n::core(e.code()));
                    }
                });
            });
            buttons.append(&button);
        }
        popup.set_child(Some(&buttons));
        menu.set_popover(Some(&popup));
        line.append(&menu);
        card.append(&line);
        let count = gtk4::Label::new(Some(&i18n::collection_count(summary.count)));
        count.set_xalign(0.0);
        count.set_margin_start(12);
        count.set_margin_bottom(12);
        count.add_css_class("dim-label");
        card.append(&count);
        let id = collection.id();
        let cover = summary.cover;
        let path = summary.thumbnail_path.clone();
        let owner = source.clone();
        let weak_picture = picture.downgrade();
        let weak_placeholder = placeholder.downgrade();
        let weak_item = item.downgrade();
        glib::spawn_future_local(async move {
            let path = match path {
                Some(p) => Some(p),
                None => match cover {
                    Some(id) => {
                        match owner.request_thumbnail(id, memedock_core::tasks::Priority::Visible) {
                            Ok(t) => t.wait().await.ok().map(|v| v.path),
                            Err(_) => None,
                        }
                    }
                    None => None,
                },
            };
            let Some(path) = path else {
                return;
            };
            let bytes = match gio::spawn_blocking(move || std::fs::read(path)).await {
                Ok(Ok(v)) => v,
                _ => return,
            };
            let Some(item) = weak_item.upgrade() else {
                return;
            };
            if item
                .item()
                .and_then(|v| v.downcast::<glib::BoxedAnyObject>().ok())
                .is_none_or(|v| v.borrow::<CollectionSummary>().collection.id() != id)
            {
                return;
            }
            if let Some(picture) = weak_picture.upgrade()
                && let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes))
            {
                picture.set_paintable(Some(&texture));
                if let Some(placeholder) = weak_placeholder.upgrade() {
                    placeholder.set_visible(false);
                }
            }
        });
    });
    factory.connect_unbind(|_, value| {
        if let Some(item) = value.downcast_ref::<gtk4::ListItem>()
            && let Some(card) = item.child().and_then(|v| v.downcast::<gtk4::Box>().ok())
        {
            while let Some(child) = card.first_child() {
                card.remove(&child);
            }
        }
    });
    let grid = gtk4::GridView::new(
        Some(gtk4::NoSelection::new(Some(store.clone()))),
        Some(factory),
    );
    grid.set_min_columns(2);
    grid.set_max_columns(8);
    grid.set_single_click_activate(true);
    let model = store.clone();
    grid.connect_activate(move |_, position| {
        if let Some(row) = model
            .item(position)
            .and_then(|v| v.downcast::<glib::BoxedAnyObject>().ok())
        {
            choose(row.borrow::<CollectionSummary>().collection.id());
        }
    });
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&grid));
    content.append(&scroll);
    let page = adw::NavigationPage::new(&content, i18n::text(Key::TabCollections));
    let parent = page.downgrade();
    let owner = library.clone();
    let feedback = Rc::clone(&notify);
    create.connect_clicked(move |_| {
        if let Some(parent) = parent.upgrade() {
            ask_name(&parent, owner.clone(), None, Rc::clone(&feedback));
        }
    });
    let weak = store.downgrade();
    let mut subscription = match library.subscribe() {
        Ok(v) => v,
        Err(error) => {
            notify(i18n::core(error.code()));
            return;
        }
    };
    let task = glib::spawn_future_local(async move {
        loop {
            let result = match library.collection_summaries() {
                Ok(t) => t.wait().await,
                Err(e) => Err(e),
            };
            let Some(store) = weak.upgrade() else {
                break;
            };
            match result {
                Ok(values) => {
                    let objects: Vec<_> =
                        values.into_iter().map(glib::BoxedAnyObject::new).collect();
                    store.splice(0, store.n_items(), &objects);
                }
                Err(error) => notify(i18n::core(error.code())),
            }
            match subscription.next().await {
                Ok(memedock_core::events::Notification::Closed) | Err(_) => break,
                _ => {}
            }
            glib::timeout_future(std::time::Duration::from_millis(100)).await;
        }
    });
    let task = Rc::new(RefCell::new(Some(task)));
    page.connect_hidden(move |_| {
        if let Some(task) = task.borrow_mut().take() {
            task.abort();
        }
    });
    navigation.push(&page);
}

fn ask_name(
    parent: &impl IsA<gtk4::Widget>,
    library: Library,
    target: Option<Collection>,
    notify: Notify,
) {
    let dialog = adw::AlertDialog::new(
        Some(i18n::text(if target.is_some() {
            Key::Rename
        } else {
            Key::NewCollection
        })),
        None,
    );
    let entry = gtk4::Entry::new();
    i18n::bind(&entry, "placeholder-text", Key::Name);
    if let Some(value) = &target {
        entry.set_text(value.name().as_str());
    }
    dialog.set_extra_child(Some(&entry));
    dialog.add_response("cancel", i18n::text(Key::Cancel));
    dialog.add_response("save", i18n::text(Key::Done));
    dialog.set_default_response(Some("save"));
    dialog.connect_response(None, move |_, response| {
        if response != "save" {
            return;
        }
        let name = match Name::new(entry.text().to_string()) {
            Ok(v) => v,
            Err(_) => {
                notify(i18n::text(Key::FailureUnknown));
                return;
            }
        };
        let library = library.clone();
        let target = target.clone();
        let notify = Rc::clone(&notify);
        glib::spawn_future_local(async move {
            let task = match target {
                Some(v) => library.rename_collection(v.id(), v.lifecycle().generation(), name),
                None => library.create_collection(name, None),
            };
            let result = match task {
                Ok(t) => t.wait().await,
                Err(e) => Err(e),
            };
            if let Err(error) = result {
                notify(i18n::core(error.code()));
            }
        });
    });
    dialog.present(Some(parent));
}
