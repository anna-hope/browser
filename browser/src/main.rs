use std::env;

use anyhow::Result;

use octo_browser::Browser;

const APP_ID: &str = "me.annahope.Octo";

fn main() -> Result<()> {
    let mut browser = Browser::new().expect("Failed to initialize the browser");
    let url = env::args().nth(1).unwrap_or_else(|| {
        let current_dir = env::current_dir().expect("Failed to get current working directory");
        format!("file://{}/LICENSE", current_dir.to_string_lossy())
    });

    browser.load(&url)
}
