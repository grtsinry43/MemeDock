use super::{Coordinator, Notify};
use crate::i18n::{self, Key};
use gtk4::{glib, prelude::*};
use libadwaita::{self as adw, prelude::*};
use memedock_core::TrashTarget;
use memedock_domain::lifecycle::EntityId;
use std::rc::Rc;

pub(super) fn present(
    parent: &impl IsA<gtk4::Widget>,
    coordinator: Rc<Coordinator>,
    notify: Notify,
) -> adw::Dialog {
    let dialog = adw::Dialog::new();
    i18n::bind(&dialog, "title", Key::TrashGroups);
    dialog.set_content_width(580);
    dialog.set_content_height(460);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let scroll = gtk4::ScrolledWindow::new();
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    scroll.set_child(Some(&content));
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    let weak = dialog.downgrade();
    let weak_content = content.downgrade();
    let source = Rc::clone(&coordinator);
    let task = glib::spawn_future_local(async move {
        let mut events = source.library.subscribe().ok();
        loop {
            let result = async {
                let collections = source.library.collections(true)?.wait().await?;
                let tags = source.library.tags(true)?.wait().await?;
                Ok::<_, memedock_core::CoreError>((collections, tags))
            }
            .await;
            let (Some(dialog), Some(content)) = (weak.upgrade(), weak_content.upgrade()) else {
                break;
            };
            while let Some(child) = content.first_child() {
                content.remove(&child);
            }
            match result {
                Ok((collections, tags)) => {
                    if collections.is_empty() && tags.is_empty() {
                        content.append(&i18n::label(Key::TrashEmpty));
                    }
                    for (title, entries) in [
                        (
                            Key::TabCollections,
                            collections
                                .iter()
                                .map(|value| {
                                    (
                                        value.name().as_str().to_owned(),
                                        TrashTarget {
                                            entity: EntityId::Collection(value.id()),
                                            generation: value.lifecycle().generation(),
                                            deleted_revision: value.lifecycle().revision(),
                                        },
                                    )
                                })
                                .collect::<Vec<_>>(),
                        ),
                        (
                            Key::TagsTitle,
                            tags.iter()
                                .map(|value| {
                                    (
                                        value.name().as_str().to_owned(),
                                        TrashTarget {
                                            entity: EntityId::Tag(value.id()),
                                            generation: value.lifecycle().generation(),
                                            deleted_revision: value.lifecycle().revision(),
                                        },
                                    )
                                })
                                .collect::<Vec<_>>(),
                        ),
                    ] {
                        if entries.is_empty() {
                            continue;
                        }
                        let group = adw::PreferencesGroup::new();
                        i18n::bind(&group, "title", title);
                        for (name, target) in entries {
                            group.add(&row(&dialog, &source, &notify, &name, target));
                        }
                        content.append(&group);
                    }
                }
                Err(error) => content.append(&gtk4::Label::new(Some(i18n::core(error.code())))),
            }
            drop(dialog);
            drop(content);
            let Some(events) = events.as_mut() else {
                break;
            };
            match events.next().await {
                Ok(memedock_core::events::Notification::Closed) | Err(_) => break,
                _ => (),
            }
        }
    });
    dialog.connect_closed(move |_| task.abort());
    dialog.present(Some(parent));
    dialog
}

fn row(
    dialog: &adw::Dialog,
    coordinator: &Rc<Coordinator>,
    notify: &Notify,
    name: &str,
    target: TrashTarget,
) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_use_markup(false);
    row.set_title(name);
    let restore = i18n::button(Key::Restore);
    restore.set_valign(gtk4::Align::Center);
    restore.set_widget_name("trash-group-restore");
    let source = Rc::clone(coordinator);
    let feedback = Rc::clone(notify);
    let entry = target.clone();
    restore.connect_clicked(move |_| source.restore(entry.clone(), Rc::clone(&feedback)));
    row.add_suffix(&restore);
    let purge = i18n::button(Key::Purge);
    purge.add_css_class("destructive-action");
    purge.set_valign(gtk4::Align::Center);
    purge.set_widget_name("trash-group-purge");
    let source = Rc::clone(coordinator);
    let notify = Rc::clone(notify);
    let parent = dialog.downgrade();
    purge.connect_clicked(move |_| {
        if let Some(parent) = parent.upgrade() {
            source.request(
                &parent,
                vec![target.clone()],
                Rc::clone(&notify),
                Rc::new(|_| {}),
            );
        }
    });
    row.add_suffix(&purge);
    row
}
