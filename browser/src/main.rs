use std::env;

use anyhow::Result;
use octo_engine::Engine;

pub fn show(url: &str, engine: &mut Engine) -> Result<()> {
    let body = engine.load(url)?;
    if let Some(body) = body {
        println!("{body}");
    } else {
        eprintln!("The request returned an empty response");
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut engine = Engine::default();
    let url = env::args().nth(1).unwrap_or_else(|| {
        let current_dir = env::current_dir().expect("Failed to get current working directory");
        format!("file://{}/LICENSE", current_dir.to_string_lossy())
    });
    show(&url, &mut engine)
}
