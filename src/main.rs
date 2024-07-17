use std::env;

use anyhow::Result;
use browser::Browser;

pub fn show(url: &str, browser: &mut Browser) -> Result<()> {
    let body = browser.load(url)?;
    if let Some(body) = body {
        println!("{body}");
    } else {
        eprintln!("The request returned an empty response");
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut browser = Browser::default();
    let url = env::args().nth(1).unwrap_or_else(|| {
        let current_dir = env::current_dir().unwrap();
        format!("file://{}/LICENSE", current_dir.to_string_lossy())
    });
    show(&url, &mut browser)
}
