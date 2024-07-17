use crate::request::RequestMethod;
use crate::url::{Url, WebUrl};
use crate::{cache::Cache, request};
use anyhow::{anyhow, Context};
use std::fs;

// TODO: Check what real browsers set this to.
const MAX_REDIRECTS: u8 = 5;

// AFAIK no entity in the spec is longer than 26 chars.
const MAX_ENTITY_LEN: usize = 26;

macro_rules! parse_optional_body {
    ($maybe_body:expr, $render:expr) => {
        $maybe_body
            .as_deref()
            .map(|s| parse_body(s, $render))
            .transpose()
    };
}

macro_rules! render_optional_body {
    ($maybe_body:expr) => {
        parse_optional_body!($maybe_body, true)
    };
}

fn parse_body(body: &str, render: bool) -> anyhow::Result<String> {
    let mut in_tag = false;
    let mut current_entity = String::new();
    let mut skip_entity = false;

    let mut result = String::new();
    let bytes = body.as_bytes();

    let mut current_index = 0;
    while current_index < bytes.len() {
        let c = char::from(bytes[current_index]);

        if c == '&' {
            if skip_entity {
                // Reset.
                skip_entity = false;
            } else {
                // This is an entity, so we'll consume the chars until we reach its end.

                // TODO: Use https://html.spec.whatwg.org/entities.json to get all entities
                // in the spec?

                current_entity.push(c);
                current_index += 1;

                while let Some(next_char) = bytes.get(current_index).map(|b| char::from(*b)) {
                    current_entity.push(next_char);
                    current_index += 1;
                    if next_char == ';' || current_entity.len() == MAX_ENTITY_LEN {
                        break;
                    }
                }

                let parsed_entity = match current_entity.as_str() {
                    "&lt;" => Some('<'),
                    "&gt;" => Some('>'),
                    _ => None,
                };

                if let Some(entity) = parsed_entity {
                    result.push(entity);
                } else {
                    // Skip entities we don't know by "rewinding" the index
                    // to start at the current entity (or whatever else starts with &).
                    // (I don't love this.)
                    skip_entity = true;
                    current_index -= current_entity.len();
                }
                current_entity.clear();
                continue;
            }
        }

        if c == '<' && render {
            in_tag = true;
        } else if c == '>' && render {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
        current_index += 1;
    }

    Ok(result)
}

/// Returns the body of a WebUrl, handling potential redirects.
fn load_web_url(url: &WebUrl) -> anyhow::Result<Option<String>> {
    let mut request = request::Request::init(RequestMethod::Get, &url.host, true);
    let mut response = request.make(url, None)?;
    let mut status_code = response.status_code();
    let mut num_redirects = 0;
    let mut body = response.body.clone();

    while (300..400).contains(&status_code) && num_redirects < MAX_REDIRECTS {
        let new_url = response
            .headers
            .get("location")
            .ok_or_else(|| anyhow!("Missing location in {response:?}"))?;

        let new_url = if new_url.starts_with('/') {
            Url::Web(url.with_path(new_url))
        } else {
            new_url.parse::<Url>()?
        };

        let new_url = new_url
            .as_web_url()
            .ok_or_else(|| anyhow!("Not a WebUrl: {new_url:?}"))
            .context(anyhow!("{response:?}"))?;

        response = request.make(new_url, None)?;
        status_code = response.status_code();
        body.clone_from(&response.body);
        num_redirects += 1;
    }

    Ok(body)
}

#[derive(Debug, Default)]
pub struct Browser {
    cache: Cache,
}

impl Browser {
    fn load_or_get_cached(&self, url: &WebUrl) -> anyhow::Result<Option<String>> {
        if let Some(response) = self.cache.get(url) {
            render_optional_body!(response.body)
        } else {
            let body = load_web_url(url)?;
            render_optional_body!(body)
        }
    }

    pub fn load(&mut self, url: &str) -> anyhow::Result<Option<String>> {
        let url = url.parse::<Url>()?;

        match url {
            Url::Web(url) => self.load_or_get_cached(&url),
            Url::File(url) => {
                let contents = fs::read(&url.path).context(url.path)?;
                let contents = String::from_utf8_lossy(&contents);
                Ok(Some(contents.to_string()))
            }
            Url::Data(url) => Ok(Some(url.data)),
            Url::ViewSource(url) => {
                let response = request::Request::get(&url)?;
                parse_optional_body!(response.body, false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::env;

    #[test]
    fn load_url() {
        Browser::default().load("http://example.org").unwrap();
    }

    #[test]
    fn load_url_https() {
        Browser::default().load("https://example.org").unwrap();
    }

    #[test]
    fn load_file() {
        let project_root = env::current_dir().unwrap();
        Browser::default()
            .load(format!("file://{}/LICENSE", project_root.to_string_lossy()).as_str())
            .unwrap();
    }

    #[test]
    fn parse_entities() {
        let example = "&lt;div&gt;";
        let parsed = parse_body(example, true).unwrap();
        assert_eq!(parsed, "<div>");
    }

    #[test]
    fn skip_unknown_entities() {
        let example = "&potato;div&chips;";
        let parsed = parse_body(example, true).unwrap();
        assert_eq!(parsed, example);
    }

    #[test]
    fn load_view_source() {
        Browser::default()
            .load("view-source:http://example.org/")
            .unwrap();
    }

    fn test_redirect_equality(url_redirect: &str, url_no_redirect: &str) -> Result<()> {
        let mut browser = Browser::default();
        let body_no_redirect = browser.load(url_no_redirect)?;
        let body_redirect = browser.load(url_redirect)?;
        assert_eq!(body_redirect, body_no_redirect);
        Ok(())
    }

    #[test]
    fn redirect() -> Result<()> {
        test_redirect_equality(
            "https://browser.engineering/redirect",
            "https://browser.engineering/http.html",
        )
    }

    #[test]
    fn redirect_2() -> Result<()> {
        test_redirect_equality(
            "https://browser.engineering/redirect2",
            "https://browser.engineering/http.html",
        )
    }

    #[test]
    fn redirect_3() -> Result<()> {
        test_redirect_equality(
            "https://browser.engineering/redirect3",
            "https://browser.engineering/http.html",
        )
    }
}