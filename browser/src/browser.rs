use anyhow::Result;
use gtk::prelude::TextBufferExt;
use gtk::{Application, TextBuffer};
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
    pub fn load_and_show(&mut self, url: &str) -> Result<()> {
        if let Some(body) = self.engine.load(url)? {
            self.text_buffer.set_text(&body);
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
