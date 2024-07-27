use std::env;

use anyhow::Result;

use gtk::gio::ApplicationFlags;
use gtk::prelude::*;
use gtk::{glib, Application};

use octo_browser::Browser;

const APP_ID: &str = "me.annahope.Octo";

fn main() -> Result<glib::ExitCode> {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    app.connect_command_line(move |app, cmd| {
        let url = cmd
            .arguments()
            .get(1)
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                let current_dir =
                    env::current_dir().expect("Failed to get current working directory");
                format!("file://{}/LICENSE", current_dir.to_string_lossy())
            });

        let mut browser = Browser::new(app).expect("Failed to initialize the browser");
        if browser.load(&url).is_ok() {
            0
        } else {
            1
        }
    });

    let args = env::args().collect::<Vec<_>>();
    Ok(app.run_with_args(&args))
}
