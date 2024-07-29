use anyhow::Result;
use gtk::prelude::TextBufferExt;
use gtk::{pango, Application, TextBuffer, TextTag};
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;

use crate::engine::{Engine, EngineError};
use crate::lex::Token;
use crate::ui::{build_text_tag, build_ui, FontSize, FontWeight, TextTagConfig};

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

struct TextTagWithOffsets {
    text_tag: TextTag,
    start: i32,
    end: i32,
}

impl TextTagWithOffsets {
    fn new(text_tag_config: &TextTagConfig, start: usize, end: usize) -> Result<Self> {
        // TODO: This might not be the best way to keep track of tags
        // TODO: Since this way we can't reuse them.
        static INDEX: AtomicUsize = AtomicUsize::new(0);

        let start = i32::try_from(start)?;
        let end = i32::try_from(end)?;

        let tag_index = INDEX.fetch_add(1, Ordering::Relaxed);
        let name = format!("{TAG_PREFIX}_{tag_index}");
        let text_tag = build_text_tag(name.as_str(), text_tag_config);

        Ok(Self {
            text_tag,
            start,
            end,
        })
    }
}

#[derive(Debug)]
pub struct Browser {
    text_buffer: TextBuffer,
    engine: Engine,
}

impl Browser {
    fn draw(&self, tokens: &[Token]) -> Result<()> {
        let mut tags = vec![];
        let mut style = pango::Style::Normal;
        let mut weight = FontWeight::default();
        let mut text_buf = String::new();
        let mut size = FontSize::default();
        let mut superscript = false;

        for token in tokens {
            match token {
                Token::Text { text, start, end } => {
                    let mut text_tag_config = TextTagConfig::new(size, weight, style, None);
                    if superscript {
                        text_tag_config = text_tag_config.with_superscript();
                    }
                    tags.push(TextTagWithOffsets::new(&text_tag_config, *start, *end)?);
                    text_buf.push_str(text.as_str());
                }

                Token::Tag(tag) => match tag.as_str() {
                    "i" => {
                        style = pango::Style::Italic;
                    }
                    "/i" => {
                        style = pango::Style::Normal;
                    }
                    "b" => {
                        weight = FontWeight::bold();
                    }
                    "/b" => {
                        weight = FontWeight::default();
                    }
                    "small" => {
                        size = size.small();
                    }
                    "/small" => {
                        size = FontSize::default();
                    }
                    "big" => {
                        size = size.big();
                    }
                    "/big" => {
                        size = FontSize::default();
                    }
                    "sup" => {
                        superscript = true;
                    }
                    "/sup" => {
                        superscript = false;
                    }
                    _ => {
                        eprintln!("Unimplemented tag: {tag}");
                    }
                },
            }
        }

        self.text_buffer.set_text(text_buf.as_str());
        let tag_table = self.text_buffer.tag_table();
        for tag in tags {
            tag_table.add(&tag.text_tag);
            self.text_buffer.apply_tag(
                &tag.text_tag,
                &self.text_buffer.iter_at_offset(tag.start),
                &self.text_buffer.iter_at_offset(tag.end),
            );
        }

        Ok(())
    }

    pub fn load(&mut self, url: &str) -> Result<()> {
        if let Some(body) = self.engine.load(url)? {
            self.draw(&body)?;
        } else {
            self.text_buffer.set_text(EMPTY_BODY_TEXT);
        }
        Ok(())
    }

    pub fn new(app: &Application) -> Result<Self> {
        if !gtk::is_initialized() {
            return Err(BrowserError::GtkNotInitialized.into());
        }

        let text_buffer = TextBuffer::builder().text(DEFAULT_LOADING_TEXT).build();
        build_ui(app, &text_buffer);

        Ok(Self {
            engine: Engine::default(),
            text_buffer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::prelude::ApplicationExt;
    use gtk::Application;

    fn build_application() -> Application {
        Application::builder()
            .application_id("me.annahope.browser-test")
            .build()
    }

    #[test]
    fn initialize_without_gtk_returns_error() {
        let app = build_application();
        #[allow(clippy::unwrap_used)]
        let error = Browser::new(&app)
            .err()
            .unwrap()
            .downcast()
            .expect("Couldn't downcast the error");
        assert!(matches!(error, BrowserError::GtkNotInitialized));
    }

    #[test]
    fn initialize() {
        let app = build_application();
        app.connect_activate(|app| {
            let browser = Browser::new(app).expect("Couldn't initialize the browser");
            assert_eq!(browser.engine, Engine::default());
        });
    }

    #[test]
    fn draw() {
        let app = build_application();
        let url = "data:text/html,<b><i><small>Hello</small></i></b>";
        app.connect_activate(|app| {
            let mut browser = Browser::new(app).expect("Couldn't initialize the browser");
            browser.load(url).expect("Couldn't load the URL");
        });
    }
}
