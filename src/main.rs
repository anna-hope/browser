use std::env;

use anyhow::Result;

fn main() -> Result<()> {
    let url = env::args().nth(1).unwrap_or_else(|| {
        let current_dir = env::current_dir().unwrap();
        format!("file://{}/LICENSE", current_dir.to_string_lossy())
    });
    browser::show(&url)
}
