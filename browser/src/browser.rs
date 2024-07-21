use std::fs;

use anyhow::{anyhow, Context};
use unicode_segmentation::UnicodeSegmentation;

use octo_http::cache::Cache;
use octo_http::request::{Request, RequestMethod, Response};
use octo_http::url::{Url, WebUrl};

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
    // TODO: Think of a way of getting all the graphemes without allocating another Vec
    let graphemes = UnicodeSegmentation::graphemes(body, true).collect::<Vec<_>>();

    let mut current_index = 0;
    while current_index < graphemes.len() {
        let grapheme = graphemes[current_index];

        if grapheme == "&" {
            if skip_entity {
                // Reset.
                skip_entity = false;
            } else {
                // This is an entity, so we'll consume the chars until we reach its end.

                // TODO: Use https://html.spec.whatwg.org/entities.json to get all entities
                // in the spec?

                current_entity.push_str(grapheme);
                current_index += 1;

                while let Some(next_grapheme) = graphemes.get(current_index) {
                    current_entity.push_str(next_grapheme);
                    current_index += 1;
                    if *next_grapheme == ";" || current_entity.len() == MAX_ENTITY_LEN {
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

        if grapheme == "<" && render {
            in_tag = true;
        } else if grapheme == ">" && render {
            in_tag = false;
        } else if !in_tag {
            result.push_str(grapheme);
        }
        current_index += 1;
    }

    Ok(result)
}

/// Returns the body of a WebUrl, handling potential redirects.
fn load_web_url(url: &WebUrl) -> anyhow::Result<Response> {
    let mut request = Request::new(RequestMethod::Get, &url.host, true, true);
    let mut response = request.make(url, None)?;
    let mut status_code = response.status_code();
    let mut num_redirects = 0;

    while (300..400).contains(&status_code) && num_redirects < MAX_REDIRECTS {
        let new_url = response
            .headers
            .get("location")
            .ok_or_else(|| anyhow!("Missing Location header in {response:?}"))?
            .first()
            .ok_or_else(|| {
                anyhow!(
                    "Missing Location value in response headers: {:?}",
                    response.headers
                )
            })?;

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
        num_redirects += 1;
    }

    Ok(response)
}

#[derive(Debug, Default)]
pub struct Browser {
    cache: Cache,
}

impl Browser {
    fn maybe_cache_response(&mut self, url: WebUrl, response: Response) -> bool {
        self.cache
            .insert(url, response)
            .inspect_err(|e| eprintln!("Couldn't cache the response: {e}"))
            .is_ok()
    }

    fn load_or_get_cached(&mut self, url: &WebUrl) -> anyhow::Result<Option<String>> {
        if let Some(response) = self.cache.get(url).get() {
            render_optional_body!(response.as_ref().body)
        } else {
            let response = load_web_url(url)?;
            let parsed_body = render_optional_body!(&response.body)?;
            self.maybe_cache_response(url.clone(), response);
            Ok(parsed_body)
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
                let response = Request::get(&url)?;
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
    fn load_url() -> Result<()> {
        Browser::default().load("http://example.org")?;
        Ok(())
    }

    #[test]
    fn load_url_https() -> Result<()> {
        Browser::default().load("https://example.org")?;
        Ok(())
    }

    #[test]
    fn load_file() -> Result<()> {
        let current_dir = env::current_dir().expect("Can't get current directory.");
        let project_root = current_dir.parent().expect("Can't get parent directory");
        Browser::default()
            .load(format!("file://{}/LICENSE", project_root.to_string_lossy()).as_str())?;
        Ok(())
    }

    #[test]
    fn parse_entities() -> Result<()> {
        let example = "&lt;div&gt;";
        let parsed = parse_body(example, true)?;
        assert_eq!(parsed, "<div>");
        Ok(())
    }

    #[test]
    fn skip_unknown_entities() -> Result<()> {
        let example = "&potato;div&chips;";
        let parsed = parse_body(example, true)?;
        assert_eq!(parsed, example);
        Ok(())
    }

    #[test]
    fn load_view_source() -> Result<()> {
        Browser::default().load("view-source:http://example.org/")?;
        Ok(())
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

    #[test]
    fn cache() -> Result<()> {
        let mut browser = Browser::default();
        browser.load("https://example.org")?;
        browser.load("https://browser.engineering/http.html")?;
        assert!(!browser.cache.into_iter().collect::<Vec<_>>().is_empty());
        Ok(())
    }
}
