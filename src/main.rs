use std::env::args;

use anyhow::{anyhow, Result};

use browser::{load, URL};

fn main() -> Result<()> {
    let url = args()
        .nth(1)
        .ok_or(anyhow!("Please provide an argument for the URL"))?;
    load(&url)
}
