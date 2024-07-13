mod request;

use std::fs;

use anyhow::{Context, Result};

pub fn show(body: &str) {
    let mut in_tag = false;
    for c in body.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            print!("{c}");
        }
    }
}

pub fn load(url: &str) -> Result<()> {
    let url = url.parse::<request::Url>()?;
    match url.scheme {
        request::Scheme::Http | request::Scheme::Https => {
            let request = request::Request::init(request::RequestMethod::Get, url.clone())
                .with_extra_headers(&[("User-Agent", "Octo")]);
            let response = request.make()?;
            show(
                response
                    .body
                    .ok_or_else(|| anyhow::anyhow!("Empty response body"))?
                    .as_str(),
            );
        }
        request::Scheme::File => {
            let contents = fs::read(&url.path).context(url.path)?;
            let contents = String::from_utf8_lossy(&contents);
            println!("{contents}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn load_url() {
        load("http://example.org").unwrap();
    }

    #[test]
    fn load_url_https() {
        load("https://example.org").unwrap();
    }

    #[test]
    fn load_file() {
        let project_root = env::current_dir().unwrap();
        load(format!("file://{}/LICENSE", project_root.to_string_lossy()).as_str()).unwrap();
    }
}
