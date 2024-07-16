mod request;
mod url;

use std::fs;

use crate::request::RequestMethod;
use crate::url::Url;
use anyhow::{anyhow, Context, Result};

// TODO: Check what real browsers set this to.
const MAX_REDIRECTS: u8 = 5;

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

fn load(url: &str) -> Result<String> {
    let url = url.parse::<Url>()?;

    match url {
        Url::Web(url) => {
            let mut request = request::Request::init(RequestMethod::Get, &url.host, true);
            let mut response = request.make(&url, None)?;
            let mut status_code = response.status_code();
            let mut num_redirects = 0;
            let mut body = response.body.clone();

            while (300..400).contains(&status_code) && num_redirects < MAX_REDIRECTS {
                let new_url = response
                    .headers
                    .get("location")
                    .ok_or_else(|| anyhow!("Missing location in {response:?}"))?;
                let new_url = new_url.parse::<Url>()?;
                let new_url = new_url
                    .as_web_url()
                    .ok_or_else(|| anyhow!("Not a WebUrl: {new_url:?}"))
                    .context(anyhow!("{response:?}"))?;

                response = request.make(new_url, None)?;
                status_code = response.status_code();
                body.clone_from(&response.body);
                num_redirects += 1;
            }

            let parsed_body = parse_body(
                body.ok_or_else(|| anyhow!("Empty response body"))?.as_str(),
                true,
            )?;
            Ok(parsed_body)
        }
        Url::File(url) => {
            let contents = fs::read(&url.path).context(url.path)?;
            let contents = String::from_utf8_lossy(&contents);
            Ok(contents.to_string())
        }
        Url::Data(url) => Ok(url.data),
        Url::ViewSource(url) => {
            let response = request::Request::get(&url)?;
            let parsed_body = parse_body(
                response
                    .body
                    .ok_or_else(|| anyhow!("Empty response body"))?
                    .as_str(),
                false,
            )?;
            Ok(parsed_body)
        }
    }
}

pub fn show(url: &str) -> Result<()> {
    let body = load(url)?;
    println!("{body}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::{Request, RequestMethod};
    use crate::url::Url;
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

    fn test_redirect_equality(url_redirect: &str, url_no_redirect: &str) {
        let url_no_redirect = url_no_redirect.parse::<Url>().unwrap();
        let url_no_redirect = url_no_redirect.as_web_url().unwrap();

        let url = url_redirect.parse::<Url>().unwrap();
        let url = url.as_web_url().unwrap();

        let response_no_redirect = Request::get(url_no_redirect).unwrap();

        let mut request = Request::init(RequestMethod::Get, &url.host, true);
        let response_redirect = request.make(url, None).unwrap();

        assert_eq!(response_redirect.body, response_no_redirect.body);
    }

    #[test]
    fn redirect() {
        test_redirect_equality(
            "https://browser.engineering/redirect",
            "https://browser.engineering/http.html",
        );
    }

    #[test]
    fn redirect_2() {
        test_redirect_equality(
            "https://browser.engineering/redirect2",
            "https://browser.engineering/http.html",
        );
    }

    #[test]
    fn redirect_3() {
        test_redirect_equality(
            "https://browser.engineering/redirect3",
            "https://browser.engineering/http.html",
        );
    }
}
