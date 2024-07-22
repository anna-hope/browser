use std::env;
use std::rc::Rc;

use anyhow::Result;
use octo_engine::Engine;

use octo_ui::gtk::prelude::*;
use octo_ui::gtk::{glib, Application};
use octo_ui::{build_ui, ui};

const APP_ID: &str = "me.annahope.Octo";

fn show(url: &str, engine: &mut Engine, app: &Application) -> Result<()> {
    let body = engine.load(url)?;
    if let Some(body) = body {
        let text_view = ui::build_text_view(body.as_str()); // This will segfault.
        if let Some(window) = app.active_window() {
            window.set_child(Some(&text_view));
        }
    } else {
        eprintln!("The request returned an empty response");
    }
    Ok(())
}

fn main() -> Result<glib::ExitCode> {
    let mut engine = Engine::default();
    let url = env::args().nth(1).unwrap_or_else(|| {
        let current_dir = env::current_dir().expect("Failed to get current working directory");
        format!("file://{}/LICENSE", current_dir.to_string_lossy())
    });

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    show(&url, &mut engine, &app)?;
    Ok(app.run())
}
