mod request;
mod url;

use std::fs;

use anyhow::{anyhow, Context, Result};

fn parse_body(body: &str, render: bool) -> Result<String> {
    let mut in_tag = false;
    let mut current_entity = String::new();
    let mut chars = body.chars();
    let mut result = String::new();

    while let Some(c) = chars.next() {
        if c == '&' {
            // This is an entity, so we'll consume the chars until we reach the end.
            current_entity.extend(chars.by_ref().take_while(|c| *c != ';'));
            let entity_char = match current_entity.as_str() {
                "lt" => '<',
                "gt" => '>',
                _ => return Err(anyhow!("Unknown entity: {}", current_entity)),
            };
            result.push(entity_char);
            current_entity.clear();
        } else if c == '<' && render {
            in_tag = true;
        } else if c == '>' && render {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }

    Ok(result)
}

pub fn load(url: &str) -> Result<()> {
    let url = url.parse::<url::Url>()?;

    match url {
        url::Url::Web(url) => {
            let response = request::Request::get(&url)?;
            let parsed_body = parse_body(
                response
                    .body
                    .ok_or_else(|| anyhow!("Empty response body"))?
                    .as_str(),
                true,
            )?;
            println!("{parsed_body}");
        }
        url::Url::File(url) => {
            let contents = fs::read(&url.path).context(url.path)?;
            let contents = String::from_utf8_lossy(&contents);
            println!("{contents}");
        }
        url::Url::Data(url) => {
            println!("{}", url.data);
        }
        url::Url::ViewSource(url) => {
            let response = request::Request::get(&url)?;
            let parsed_body = parse_body(
                response
                    .body
                    .ok_or_else(|| anyhow!("Empty response body"))?
                    .as_str(),
                false,
            )?;
            println!("{parsed_body}");
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

    #[test]
    fn parse_entities() {
        let example = "&lt;div&gt;";
        let parsed = parse_body(example, true).unwrap();
        assert_eq!(parsed, "<div>");
    }

    #[test]
    fn load_view_source() {
        load("view-source:http://example.org/").unwrap();
    }
}
