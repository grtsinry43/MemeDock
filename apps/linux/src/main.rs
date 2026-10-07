mod actions;
mod browse;
mod detail;
mod i18n;
mod import;
mod library_view;
mod output;
mod paths;
mod playback;
mod window;

use gtk4::prelude::*;
use libadwaita as adw;

fn main() -> gtk4::glib::ExitCode {
    let exe = std::env::current_exe().map_or_else(
        |_| String::from("unknown"),
        |path| path.display().to_string(),
    );
    output::log_process(&format!("进程启动 exe={exe}"));
    let app = adw::Application::builder()
        .application_id("com.grtsinry43.memedock")
        .build();
    app.connect_activate(window::activate);
    app.run()
}
