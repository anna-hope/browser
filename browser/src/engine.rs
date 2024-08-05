use crate::lex;
use crate::lex::Token;
use anyhow::{anyhow, Context};
use octo_http::cache::Cache;
use octo_http::request::{Request, RequestMethod, Response};
use octo_url::url::AboutValue;
use octo_url::{Url, UrlError, WebUrl};
use std::fs;
use thiserror::Error;

// TODO: Check what real browsers set this to.
const MAX_REDIRECTS: u8 = 5;

macro_rules! lex_optional_body {
    ($maybe_body:expr, $render:expr) => {
        $maybe_body.as_deref().map(|s| lex::lex(s, $render))
    };
}

macro_rules! render_optional_body {
    ($maybe_body:expr) => {
        lex_optional_body!($maybe_body, true)
    };
}

#[derive(Error, Debug)]
pub(crate) enum EngineError {
    #[error("Error loading page: {0}")]
    Load(#[from] octo_http::HttpError),

    #[error("Redirect error: {0}")]
    Redirect(String),

    #[error("Error parsing URL: {0}")]
    ParseUrl(#[from] UrlError),

    #[error("Not a web URL: {0:?}")]
    NotWebUrl(Url),
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
            .ok_or_else(|| {
                EngineError::Redirect(format!(
                    "Missing Location header in response: {:?}",
                    response.headers
                ))
            })?
            .first()
            .ok_or_else(|| {
                EngineError::Redirect(format!(
                    "Missing Location value in response headers: {:?}",
                    response.headers
                ))
            })?;

        let new_url = if new_url.starts_with('/') {
            Url::Web(url.with_path(new_url))
        } else {
            new_url.parse::<Url>()?
        };

        let new_url = new_url
            .as_web_url()
            .ok_or_else(|| EngineError::NotWebUrl(new_url.clone()))
            .context(anyhow!("{response:?}"))?;

        response = request.make(new_url, None)?;
        status_code = response.status_code();
        num_redirects += 1;
    }

    if (300..400).contains(&status_code) {
        // If we still have a redirect status code and exhausted MAX_REDIRECTS.
        return Err(EngineError::Redirect("Too many redirects.".to_string()).into());
    }

    Ok(response)
}

#[derive(Debug)]
enum LoadedResponse {
    Fresh(Response),
    Cached(Response),
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct Engine {
    cache: Cache,
}

impl Engine {
    fn maybe_cache_response(&mut self, url: WebUrl, response: Response) -> bool {
        self.cache
            .insert(url, response)
            .inspect_err(|e| eprintln!("Couldn't cache the response: {e}"))
            .is_ok()
    }

    fn load_or_get_cached(&self, url: &WebUrl) -> anyhow::Result<LoadedResponse> {
        if let Some(response) = self.cache.get(url).maybe_clone() {
            Ok(LoadedResponse::Cached(response))
        } else {
            load_web_url(url).map(LoadedResponse::Fresh)
        }
    }

    fn load_or_maybe_cache(&mut self, url: WebUrl) -> anyhow::Result<Response> {
        let response = self.load_or_get_cached(&url)?;
        Ok(match response {
            LoadedResponse::Fresh(response) => {
                self.maybe_cache_response(url, response.clone());
                response
            }
            LoadedResponse::Cached(response) => response,
        })
    }

    fn load_and_parse_body(&mut self, url: WebUrl) -> anyhow::Result<Option<Vec<Token>>> {
        let response = self.load_or_maybe_cache(url)?;
        Ok(render_optional_body!(response.body))
    }

    pub(crate) fn load(&mut self, url: &str) -> anyhow::Result<Option<Vec<Token>>> {
        let url = url
            .parse::<Url>()
            .inspect_err(|e| eprintln!("{e}"))
            .unwrap_or(Url::About(AboutValue::Blank));

        match url {
            Url::Web(url) => self.load_and_parse_body(url),
            Url::File(url) => {
                let contents = fs::read(&url.path).context(url.path)?;
                let contents = String::from_utf8_lossy(&contents);
                let tokens = vec![Token::Text(contents.to_string())];
                Ok(Some(tokens))
            }
            Url::Data(url) => {
                let tokens = render_optional_body!(Some(url.data));
                Ok(tokens)
            }
            Url::ViewSource(url) => {
                let response = Request::get(&url)?;
                Ok(lex_optional_body!(response.body, false))
            }
            Url::About(about_value) => {
                let body = match about_value {
                    AboutValue::Blank => "".to_string(),
                };
                let tokens = vec![Token::Text(body)];
                Ok(Some(tokens))
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
        Engine::default().load("http://example.org")?;
        Ok(())
    }

    #[test]
    fn load_url_https() -> Result<()> {
        Engine::default().load("https://example.org")?;
        Ok(())
    }

    #[test]
    fn load_file() -> Result<()> {
        let current_dir = env::current_dir().expect("Can't get current directory.");
        let project_root = current_dir.parent().expect("Can't get parent directory");
        Engine::default()
            .load(format!("file://{}/LICENSE", project_root.to_string_lossy()).as_str())?;
        Ok(())
    }

    #[test]
    fn load_view_source() -> Result<()> {
        Engine::default().load("view-source:http://example.org/")?;
        Ok(())
    }

    fn test_redirect_equality(url_redirect: &str, url_no_redirect: &str) -> Result<()> {
        let mut browser = Engine::default();
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
        let mut browser = Engine::default();
        browser.load("https://example.org")?;
        browser.load("https://browser.engineering/http.html")?;
        assert!(!browser.cache.into_iter().collect::<Vec<_>>().is_empty());
        Ok(())
    }
}
