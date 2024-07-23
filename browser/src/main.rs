use std::env;
// use std::sync::{Arc, Mutex};

use anyhow::Result;
use octo_engine::Engine;

use octo_ui::gtk::gio::ApplicationFlags;
use octo_ui::gtk::prelude::*;
use octo_ui::gtk::{glib, Application, TextBuffer};
use octo_ui::ui::{build_scrolled_window, build_text_view, build_window};

const APP_ID: &str = "me.annahope.Octo";

// type SharedEngine = Arc<Mutex<Engine>>;

// fn show(
//     engine: SharedEngine,
//     request_recv: Receiver<&str>,
//     response_sender: Sender<Result<Option<String>>>,
// ) {
//     while let Ok(url) = request_recv.recv() {
//         let body = engine.lock().expect("poisoned").load(url);
//         response_sender.send(body).expect("Failed to send the body");
//     }
// }

fn main() -> Result<glib::ExitCode> {
    // let engine = Arc::new(Mutex::new(Engine::default()));
    // let (request_sender, request_recv) = channel();
    // let (response_sender, response_recv) = channel();

    let app = Application::builder()
        .application_id(APP_ID)
        .flags(ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_command_line(move |app, cmd| {
        let window = build_window(app);
        let text_view = build_text_view("Loading...");
        let scrolled_window = build_scrolled_window(&[&text_view]);

        window.set_child(Some(&scrolled_window));

        let url = cmd
            .arguments()
            .get(1)
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                let current_dir =
                    env::current_dir().expect("Failed to get current working directory");
                format!("file://{}/LICENSE", current_dir.to_string_lossy())
            });

        let mut engine = Engine::default();
        let response = engine.load(url.as_str()).expect("Couldn't load the url");
        if let Some(body) = response {
            let buffer = TextBuffer::builder().text(body).build();
            text_view.set_buffer(Some(&buffer));
        } else {
            eprintln!("Response had no body");
        }

        window.present();
        0
    });
    let args = env::args().collect::<Vec<_>>();

    Ok(app.run_with_args(&args))
}
