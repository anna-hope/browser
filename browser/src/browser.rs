use anyhow::Result;
use thiserror::Error;

use crate::engine::{Engine, EngineError};
use crate::lex::Token;

const DEFAULT_LOADING_TEXT: &str = "Loading...";
const EMPTY_BODY_TEXT: &str = "The response body was empty.";
const TAG_PREFIX: &str = "tag_";

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("GTK hasn't been initialized (make sure you're calling from `connect`)")]
    GtkNotInitialized,

    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),
}

#[derive(Debug)]
pub struct Browser {
    engine: Engine,
}

impl Browser {
    fn draw(&self, tokens: &[Token]) -> Result<()> {
        // TODO: Figure out if there is a cleaner way to do this, but .
        let mut text_buf = String::new();

        for token in tokens {
            match token {
                Token::Text(text) => {
                    todo!()
                }

                // I am *not* a fan.
                Token::Tag(tag) => match tag.as_str() {
                    "i" => {
                        todo!()
                    }
                    "/i" => {
                        todo!()
                    }
                    "b" => {
                        todo!()
                    }
                    "/b" => {
                        todo!()
                    }
                    "small" => {
                        todo!()
                    }
                    "/small" => {
                        todo!()
                    }
                    "big" => {
                        todo!()
                    }
                    "/big" => {
                        todo!()
                    }
                    "sup" => {
                        todo!()
                    }
                    "/sup" => {
                        todo!()
                    }
                    _ => {
                        eprintln!("Unimplemented tag: {tag}");
                    }
                },
            }
        }

        todo!();
    }

    pub fn load(&mut self, url: &str) -> Result<()> {
        if let Some(body) = self.engine.load(url)? {
            self.draw(&body)?;
        } else {
            todo!()
        }
        Ok(())
    }

    pub fn new() -> Result<Self> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_without_gtk_returns_error() {
        #[allow(clippy::unwrap_used)]
        let error = Browser::new()
            .err()
            .unwrap()
            .downcast()
            .expect("Couldn't downcast the error");
        assert!(matches!(error, BrowserError::GtkNotInitialized));
    }

    #[test]
    fn initialize() {
        let _browser = Browser::new().expect("Couldn't initialize the browser");
    }

    #[test]
    fn draw() {
        let url = "data:text/html,<b><i><small>Hello</small></i></b>";
        let _browser = Browser::new().expect("Couldn't initialize the browser");
    }
}
