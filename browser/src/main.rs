use std::env;

use anyhow::Result;

use octo_browser::Browser;

const TITLE: &str = "Octo";

fn main() -> iced::Result {
    let _url = env::args().nth(1).unwrap_or_else(|| {
        let current_dir = env::current_dir().expect("Failed to get current working directory");
        format!("file://{}/LICENSE", current_dir.to_string_lossy())
    });
    //
    // browser.load(&url)

    iced::application(TITLE, Browser::update, Browser::view)
        .theme(Browser::theme)
        .run()
}
