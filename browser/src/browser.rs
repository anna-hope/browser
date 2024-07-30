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
        // TODO: Figure out if there is a cleaner way to do this, but .
        let mut previous_config = None;
        let mut text_tag_config = TextTagConfig::default();
        let mut tags = vec![];
        let mut text_buf = String::new();

        for token in tokens {
            match token {
                Token::Text { text, start, end } => {
                    let tag = TextTagWithOffsets::new(&text_tag_config, *start, *end)?;
                    tags.push(tag);
                    text_buf.push_str(text.as_str());
                }

                // I am *not* a fan.
                Token::Tag(tag) => match tag.as_str() {
                    "i" => {
                        previous_config = Some(text_tag_config.clone());
                        text_tag_config.style = pango::Style::Italic;
                    }
                    "/i" => {
                        text_tag_config = previous_config.clone().unwrap_or_default();
                    }
                    "b" => {
                        previous_config = Some(text_tag_config.clone());
                        text_tag_config.weight = FontWeight::bold();
                    }
                    "/b" => {
                        text_tag_config = previous_config.clone().unwrap_or_default();
                    }
                    "small" => {
                        previous_config = Some(text_tag_config.clone());
                        text_tag_config.size = text_tag_config.size.decrease(2);
                    }
                    "/small" => {
                        text_tag_config = previous_config.clone().unwrap_or_default();
                    }
                    "big" => {
                        previous_config = Some(text_tag_config.clone());
                        text_tag_config.size = text_tag_config.size.increase(4);
                    }
                    "/big" => {
                        text_tag_config = previous_config.clone().unwrap_or_default();
                    }
                    "sup" => {
                        previous_config = Some(text_tag_config.clone());
                        // Idk if this is actually the correct way to calculate this.
                        text_tag_config.size = FontSize::default();
                        text_tag_config.rise_scaled =
                            FontSize::new_from_points(FontSize::default().points() / 4).scaled();
                        text_tag_config.scale = 0.5;
                    }
                    "/sup" => {
                        text_tag_config = previous_config.clone().unwrap_or_default();
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
