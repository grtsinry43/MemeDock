mod actions;
mod backup;
mod batch;
mod browse;
mod clipboard;
mod collections;
mod credentials;
mod detail;
mod drag;
mod i18n;
mod import;
mod library_view;
mod output;
mod paths;
mod playback;
mod preferences;
mod settings;
mod telegram;
mod window;

use gtk4::prelude::*;
use libadwaita as adw;

fn main() -> gtk4::glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("com.grtsinry43.memedock")
        .flags(gtk4::gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    window::connect(&app);
    app.run()
}
