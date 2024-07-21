use std::env;

use anyhow::Result;
use octo_engine::Engine;

use octo_ui::build_ui;
use octo_ui::gtk::prelude::*;
use octo_ui::gtk::{glib, Application};

const APP_ID: &str = "me.annahope.Octo";

fn show(url: &str, engine: &mut Engine) -> Result<()> {
    let body = engine.load(url)?;
    if let Some(body) = body {
        println!("{body}");
    } else {
        eprintln!("The request returned an empty response");
    }
    Ok(())
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);
    app.run()
}
