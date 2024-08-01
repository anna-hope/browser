use anyhow::Result;
use std::sync::OnceLock;
use thiserror::Error;

use iced::futures::{channel::mpsc, SinkExt, Stream, StreamExt};
use iced::widget::{column, row, scrollable, text, text_input, TextInput};
use iced::{stream, Element, Fill, Subscription, Task, Theme};

use crate::engine::{Engine, EngineError};
use crate::lex::Token;

const DEFAULT_LOADING_TEXT: &str = "Loading...";
const EMPTY_BODY_TEXT: &str = "The response body was empty.";

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),
}

#[derive(Debug, Clone)]
pub enum Message {
    Ready(mpsc::Sender<String>),
    Scrolled(scrollable::Viewport),
    UrlChanged(String),
    NewUrl,
    UrlLoaded(String),
    UrlLoading,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Browser {
    url: String,
    url_sender: OnceLock<mpsc::Sender<String>>,
    current_body: Option<String>,
    scrollbar_width: u16,
    scrollbar_margin: u16,
    scroller_width: u16,
    current_scroll_offset: scrollable::RelativeOffset,
    anchor: scrollable::Anchor,
}

impl Browser {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                url: "about:blank".to_string(),
                url_sender: OnceLock::new(),
                current_body: None,
                scrollbar_width: 10,
                scrollbar_margin: 0,
                scroller_width: 10,
                current_scroll_offset: scrollable::RelativeOffset::START,
                anchor: scrollable::Anchor::Start,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Scrolled(viewport) => {
                self.current_scroll_offset = viewport.relative_offset();
                Task::none()
            }
            Message::UrlChanged(new_url) => {
                self.url = new_url;
                Task::none()
            }
            Message::NewUrl => {
                let sender = self
                    .url_sender
                    .get_mut()
                    .expect("The sender should be initialized.");
                sender
                    .try_send(self.url.clone())
                    .expect("Couldn't send the url");
                Task::none()
            }
            Message::UrlLoading => {
                self.current_body = Some(DEFAULT_LOADING_TEXT.to_string());
                Task::none()
            }
            Message::UrlLoaded(body) => {
                self.current_body = Some(body);
                Task::none()
            }
            Message::Ready(sender) => {
                self.url_sender
                    .set(sender)
                    .expect("Couldn't set the sender");
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let url_input = {
            let mut input: TextInput<_, Theme> = text_input("Enter a URL", &self.url)
                .on_input(Message::UrlChanged)
                .padding(10);
            input = input.on_submit(Message::NewUrl);
            row![input]
        };

        let content = if let Some(body) = &self.current_body {
            body.as_str()
        } else {
            ""
        };

        let scrollable_content: Element<Message> =
            Element::from(scrollable(row![column![text(content)]]).width(Fill));
        column![url_input, scrollable_content].into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Light
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(url_worker)
    }
}

fn draw(tokens: &[Token]) -> Result<String> {
    let mut text_buf = String::new();

    for token in tokens {
        match token {
            Token::Text(text) => text_buf.push_str(text.as_str()),

            Token::Tag(tag) => match tag.as_str() {
                "i" => {}
                "/i" => {}
                "b" => {}
                "/b" => {}
                "small" => {}
                "/small" => {}
                "big" => {}
                "/big" => {}
                "sup" => {}
                "/sup" => {}
                _ => {
                    eprintln!("Unimplemented tag: {tag}");
                }
            },
        }
    }

    Ok(text_buf)
}

fn url_worker() -> impl Stream<Item = Message> {
    stream::channel(100, |mut output| async move {
        let (sender, mut receiver) = mpsc::channel(100);
        output
            .send(Message::Ready(sender))
            .await
            .expect("Couldn't send the Ready event");
        let mut engine = Engine::default();

        loop {
            let new_url = receiver.select_next_some().await;

            output
                .send(Message::UrlLoading)
                .await
                .expect("Couldn't send the message");

            let tokens = match engine.load(new_url.as_str()) {
                Ok(tokens) => tokens,
                Err(error) => {
                    eprintln!("{error}");
                    continue;
                }
            };

            let body = if let Some(tokens) = tokens {
                match draw(&tokens) {
                    Ok(body) => body,
                    Err(error) => {
                        eprintln!("{error}");
                        continue;
                    }
                }
            } else {
                EMPTY_BODY_TEXT.to_string()
            };

            output
                .send(Message::UrlLoaded(body))
                .await
                .expect("Couldn't send the body");
        }
    })
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
