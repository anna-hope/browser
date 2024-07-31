use anyhow::Result;
use iced::widget::{column, row, scrollable, text, vertical_space};
use iced::{Element, Task, Theme};
use thiserror::Error;

use crate::engine::{Engine, EngineError};
use crate::lex::Token;

const DEFAULT_LOADING_TEXT: &str = "Loading...";
const EMPTY_BODY_TEXT: &str = "The response body was empty.";
const TAG_PREFIX: &str = "tag_";

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),
}

#[derive(Debug, Clone)]
pub enum Message {
    Scrolled(scrollable::Viewport),
}

#[derive(Debug)]
pub struct Browser {
    engine: Engine,
    scrollbar_width: u16,
    scrollbar_margin: u16,
    scroller_width: u16,
    current_scroll_offset: scrollable::RelativeOffset,
    anchor: scrollable::Anchor,
}

impl Browser {
    fn draw(&self, tokens: &[Token]) -> Result<()> {
        let mut _text_buf = String::new();

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

    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
            scrollbar_width: 10,
            scrollbar_margin: 0,
            scroller_width: 10,
            current_scroll_offset: scrollable::RelativeOffset::START,
            anchor: scrollable::Anchor::Start,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Scrolled(viewport) => {
                self.current_scroll_offset = viewport.relative_offset();

                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let scrollable_content: Element<Message> = Element::from(scrollable(row![column![
            text("Some content"),
            vertical_space().height(2400)
        ]]));
        scrollable_content
    }

    pub fn theme(&self) -> Theme {
        Theme::Light
    }
}

impl Default for Browser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize() {
        let _browser = Browser::new();
    }

    #[test]
    fn draw() {
        let _url = "data:text/html,<b><i><small>Hello</small></i></b>";
        let _browser = Browser::new();
        todo!()
    }
}
