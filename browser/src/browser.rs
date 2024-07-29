use anyhow::Result;
use gtk::prelude::TextBufferExt;
use gtk::{pango, Application, TextBuffer, TextTag};
use thiserror::Error;

use crate::engine::{Engine, EngineError};
use crate::ui::build_ui;

const DEFAULT_LOADING_TEXT: &str = "Loading...";
const EMPTY_BODY_TEXT: &str = "The response body was empty.";

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("GTK hasn't been initialized (make sure you're calling from `connect`)")]
    GtkNotInitialized,

    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),
}

#[derive(Debug)]
pub struct Browser {
    text_buffer: TextBuffer,
    engine: Engine,
}

impl Browser {
    pub fn load(&mut self, url: &str) -> Result<()> {
        if let Some(body) = self.engine.load(url)? {
            self.text_buffer.set_text(&body);
            let style_tag = TextTag::builder()
                .name("test")
                .size(16 * pango::SCALE)
                .weight(800)
                .family("Times")
                .style(pango::Style::Italic)
                .build();

            let tag_table = self.text_buffer.tag_table();
            tag_table.add(&style_tag);

            self.text_buffer.apply_tag(
                &style_tag,
                &self.text_buffer.start_iter(),
                &self.text_buffer.end_iter(),
            );
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
}
