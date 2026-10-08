use crate::{
    i18n::{self, Key},
    preferences::{Language, Preferences, Theme},
};
use gtk4::{glib, prelude::*};
use libadwaita::{self as adw, prelude::*};
use memedock_core::Library;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone)]
pub struct Host {
    pub library: Library,
    pub parent: gtk4::Widget,
    pub preferences: Rc<RefCell<Preferences>>,
    pub maintenance: Rc<Cell<bool>>,
    pub busy: Rc<dyn Fn() -> bool>,
    pub reopen: Rc<dyn Fn()>,
    pub trash: Rc<dyn Fn()>,
}
pub fn present(host: Host) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::new();
    i18n::bind(&dialog, "title", Key::Settings);
    let page = adw::PreferencesPage::new();
    dialog.add(&page);
    let library = adw::PreferencesGroup::new();
    i18n::bind(&library, "title", Key::Library);
    let count = value_row(Key::OriginalCount);
    let originals = value_row(Key::OriginalBytes);
    let cache = value_row(Key::CacheBytes);
    library.add(&count);
    library.add(&originals);
    library.add(&cache);
    let weak_dialog = dialog.downgrade();
    let source = host.library.clone();
    let statistics_task = glib::spawn_future_local(async move {
        let mut subscription = source.subscribe().ok();
        loop {
            let stats = match source.space_statistics() {
                Ok(t) => t.wait().await,
                Err(e) => Err(e),
            };
            if let Some(dialog) = weak_dialog.upgrade() {
                match stats {
                    Ok(stats) => {
                        count.set_subtitle(&stats.known_assets.to_string());
                        originals.set_subtitle(&bytes(stats.ready_original_bytes));
                        cache.set_subtitle(&bytes(
                            stats
                                .thumbnail_bytes
                                .saturating_add(stats.temporary_share_bytes),
                        ));
                    }
                    Err(error) => dialog.add_toast(adw::Toast::new(i18n::core(error.code()))),
                }
            }
            let Some(events) = &mut subscription else {
                break;
            };
            match events.next().await {
                Ok(memedock_core::events::Notification::Closed) | Err(_) => break,
                _ => (),
            }
        }
    });
    dialog.connect_closed(move |_| statistics_task.abort());
    let trash = action_row(Key::Trash);
    let open = Rc::clone(&host.trash);
    let weak = dialog.downgrade();
    trash.connect_activated(move |_| {
        if let Some(d) = weak.upgrade() {
            d.close();
        }
        open();
    });
    library.add(&trash);
    let backup = action_row(Key::Backup);
    let backup_host = host.clone();
    backup.connect_activated(move |_| crate::backup::present(backup_host.clone()));
    library.add(&backup);
    let telegram = action_row(Key::Telegram);
    let telegram_host = host.clone();
    telegram.connect_activated(move |_| crate::telegram::present(telegram_host.clone()));
    library.add(&telegram);
    page.add(&library);
    let appearance = adw::PreferencesGroup::new();
    i18n::bind(&appearance, "title", Key::Appearance);
    let theme = combo(Key::Appearance, &[Key::System, Key::Light, Key::Dark]);
    theme.set_selected(match host.preferences.borrow().theme {
        Theme::System => 0,
        Theme::Light => 1,
        Theme::Dark => 2,
    });
    appearance.add(&theme);
    page.add(&appearance);
    let language = adw::PreferencesGroup::new();
    i18n::bind(&language, "title", Key::Language);
    let choice = combo(Key::Language, &[Key::System, Key::Chinese, Key::English]);
    choice.set_selected(match host.preferences.borrow().language {
        Language::System => 0,
        Language::Chinese => 1,
        Language::English => 2,
    });
    language.add(&choice);
    page.add(&language);
    for (row, is_theme) in [(theme.clone(), true), (choice.clone(), false)] {
        let host = host.clone();
        let dialog = dialog.downgrade();
        let theme = theme.downgrade();
        let language = choice.downgrade();
        row.connect_selected_notify(move |row| {
            if !row.is_sensitive() {
                return;
            }
            let mut value = *host.preferences.borrow();
            if is_theme {
                value.theme = match row.selected() {
                    1 => Theme::Light,
                    2 => Theme::Dark,
                    _ => Theme::System,
                };
            } else {
                value.language = match row.selected() {
                    1 => Language::Chinese,
                    2 => Language::English,
                    _ => Language::System,
                };
            }
            if value == *host.preferences.borrow() {
                return;
            }
            let previous = *host.preferences.borrow();
            if let Some(t) = theme.upgrade() {
                t.set_sensitive(false);
            }
            if let Some(l) = language.upgrade() {
                l.set_sensitive(false);
            }
            let host = host.clone();
            let dialog = dialog.clone();
            let theme = theme.clone();
            let language = language.clone();
            glib::spawn_future_local(async move {
                let saved = gtk4::gio::spawn_blocking(move || {
                    let path = crate::paths::config_dir()
                        .map_err(|_| std::io::Error::other("config directory unavailable"))?
                        .join("settings.json");
                    crate::preferences::save(&path, value)
                })
                .await;
                if matches!(saved, Ok(Ok(()))) {
                    *host.preferences.borrow_mut() = value;
                    value.apply();
                } else {
                    if let Some(d) = dialog.upgrade() {
                        d.add_toast(adw::Toast::new(i18n::text(Key::SettingsFailed)));
                    }
                    if let Some(t) = theme.upgrade() {
                        t.set_selected(match previous.theme {
                            Theme::System => 0,
                            Theme::Light => 1,
                            Theme::Dark => 2,
                        });
                    }
                    if let Some(l) = language.upgrade() {
                        l.set_selected(match previous.language {
                            Language::System => 0,
                            Language::Chinese => 1,
                            Language::English => 2,
                        });
                    }
                }
                if let Some(t) = theme.upgrade() {
                    t.set_sensitive(true);
                }
                if let Some(l) = language.upgrade() {
                    l.set_sensitive(true);
                }
            });
        });
    }
    dialog.present(Some(&host.parent));
    dialog
}
pub fn value_row(key: Key) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    i18n::bind(&row, "title", key);
    row
}
pub fn action_row(key: Key) -> adw::ActionRow {
    let row = value_row(key);
    row.set_activatable(true);
    row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
    row
}
fn combo(key: Key, keys: &'static [Key]) -> adw::ComboRow {
    let row = adw::ComboRow::new();
    i18n::bind(&row, "title", key);
    let model = gtk4::StringList::new(&keys.iter().map(|k| i18n::text(*k)).collect::<Vec<_>>());
    row.set_model(Some(&model));
    let weak = row.downgrade();
    i18n::on_language(&row, move || {
        if let Some(row) = weak.upgrade() {
            let selected = row.selected();
            let sensitive = row.is_sensitive();
            row.set_sensitive(false);
            if let Some(model) = row.model().and_downcast::<gtk4::StringList>() {
                model.splice(
                    0,
                    model.n_items(),
                    &keys.iter().map(|k| i18n::text(*k)).collect::<Vec<_>>(),
                );
            }
            row.set_selected(selected);
            row.set_sensitive(sensitive);
        }
    });
    row
}
pub fn bytes(value: i64) -> String {
    glib::format_size(u64::try_from(value).unwrap_or(0)).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a display; run under xvfb-run"]
    fn settings_and_language_bindings_work_in_a_real_gtk_context()
    -> Result<(), Box<dyn std::error::Error>> {
        adw::init()?;
        let context = glib::MainContext::default();
        let _guard = context.acquire()?;
        let root = tempfile::tempdir()?;
        let library = context.block_on(Library::open(memedock_core::LibraryConfig::new(
            root.path().join("data"),
            root.path().join("cache"),
            root.path().join("export"),
        )))?;
        i18n::set_language_mode(i18n::LanguageMode::Chinese);
        let button = i18n::button(Key::Settings);
        let custom = i18n::label(Key::TabStickers);
        custom.set_text("My own album");
        let window = adw::Window::new();
        window.set_content(Some(&button));
        window.present();
        let host = Host {
            library: library.clone(),
            parent: window.clone().upcast(),
            preferences: Rc::new(RefCell::new(Preferences::default())),
            maintenance: Rc::new(Cell::new(false)),
            busy: Rc::new(|| false),
            reopen: Rc::new(|| ()),
            trash: Rc::new(|| ()),
        };
        let dialog = present(host);
        assert_eq!(dialog.title(), "设置");
        i18n::set_language_mode(i18n::LanguageMode::English);
        assert_eq!(dialog.title(), "Settings");
        assert_eq!(button.label().as_deref(), Some("Settings"));
        assert_eq!(custom.text(), "My own album");
        crate::telegram::verify_grid_reentrancy(&context)?;
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        toolbar.add_top_bar(&header);
        let (browse_task, importer) = crate::library_view::attach(
            &toolbar,
            &header,
            library.clone(),
            Rc::new(RefCell::new(crate::import::Batch::new())),
            Rc::new(RefCell::new(crate::output::Hold::new())),
        );
        let old_content = toolbar.content().ok_or("missing library page")?.downgrade();
        // Restore can detach a newly created page before its subscription is
        // first polled. Its signal owners must be released in that case too.
        browse_task.abort();
        drop(importer);
        toolbar.set_content(None::<&gtk4::Widget>);
        toolbar.remove(&header);
        context.block_on(glib::timeout_future(std::time::Duration::from_millis(100)));
        assert!(
            old_content.upgrade().is_none(),
            "old library page retained after detaching"
        );
        context.block_on(glib::timeout_future(std::time::Duration::from_millis(100)));
        dialog.force_close();
        window.destroy();
        context.block_on(library.close())?;
        Ok(())
    }
}
