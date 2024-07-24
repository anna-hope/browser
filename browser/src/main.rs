use std::env;

use anyhow::Result;

use gtk::gio::ApplicationFlags;
use gtk::prelude::*;
use gtk::{glib, Application, TextBuffer};

use octo_browser::Browser;

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
        if browser.load_and_show(&url).is_ok() {
            0
        } else {
            1
        }
    });

    let args = env::args().collect::<Vec<_>>();
    Ok(app.run_with_args(&args))
}
