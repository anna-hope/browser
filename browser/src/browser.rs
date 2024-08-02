use std::collections::VecDeque;
use std::sync::OnceLock;
use thiserror::Error;

use iced::advanced::Widget;
use iced::font::{Family, Style, Weight};
use iced::futures::{channel::mpsc, SinkExt, Stream, StreamExt};
use iced::widget::text::{LineHeight, Rich, Span};
use iced::widget::{column, row, scrollable, text, text_input, Column, Row, TextInput};
use iced::{
    stream, window, Element, Fill, Font, Pixels, Point, Renderer, Size, Subscription, Task, Theme,
};

use crate::engine::{Engine, EngineError};
use crate::lex::{lex, Token};

const DEFAULT_LOADING_TEXT: &str = "Loading...";
const EMPTY_BODY_TEXT: &str = "The response body was empty.";
const HSTEP: f32 = 13.;
const VSTEP: f32 = 18.;
const DEFAULT_TEXT_SIZE_PIXELS: f32 = 16.;
const LINESPACE_MULTIPLIER: f32 = 1.25;

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
    UrlLoaded(Vec<Token>),
    UrlLoading,
    WindowResized(Size),
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Browser {
    url: String,
    url_sender: OnceLock<mpsc::Sender<String>>,
    tokens: Vec<Token>,
    scrollbar_width: u16,
    scrollbar_margin: u16,
    scroller_width: u16,
    current_scroll_offset: scrollable::RelativeOffset,
    anchor: scrollable::Anchor,
    current_size: Option<Size>,
}

impl Browser {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                url: "about:blank".to_string(),
                url_sender: OnceLock::new(),
                tokens: vec![],
                scrollbar_width: 10,
                scrollbar_margin: 0,
                scroller_width: 10,
                current_scroll_offset: scrollable::RelativeOffset::START,
                anchor: scrollable::Anchor::Start,
                current_size: None,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ready(sender) => {
                self.url_sender
                    .set(sender)
                    .expect("Couldn't set the sender");
                Task::none()
            }
            Message::WindowResized(size) => {
                self.current_size = Some(size);
                Task::none()
            }
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
                self.tokens = lex(DEFAULT_LOADING_TEXT, true);
                Task::none()
            }
            Message::UrlLoaded(tokens) => {
                self.tokens = tokens;
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

        let display_list = layout(&self.tokens);
        let content = Rich::with_spans(display_list);

        let scrollable_content: Element<Message> = Element::from(scrollable(content).width(Fill));
        column![url_input, scrollable_content].into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Light
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([self.url_subscription(), self.resize_subscription()])
    }

    fn url_subscription(&self) -> Subscription<Message> {
        Subscription::run(url_worker)
    }

    pub fn resize_subscription(&self) -> Subscription<Message> {
        window::resize_events().map(|(_id, size)| Message::WindowResized(size))
    }
}

struct Layout<'a> {
    display_list: Vec<Span<'a, Message>>,
    line: VecDeque<Span<'a, Message>>,
    text_size: f32,
    style: Style,
    weight: Weight,
}

impl<'a> Default for Layout<'a> {
    fn default() -> Self {
        Self {
            display_list: vec![],
            line: VecDeque::new(),
            text_size: DEFAULT_TEXT_SIZE_PIXELS,
            style: Style::default(),
            weight: Weight::default(),
        }
    }
}

impl<'a> Layout<'a> {
    fn flush(&mut self) {
        if self.line.is_empty() {
            return;
        }

        while let Some(span) = self.line.pop_front() {
            self.display_list.push(span);
        }

        self.display_list.push(Span::new('\n'));
    }

    fn push(&mut self, text: &'a str) {
        let font = Font {
            family: Family::default(),
            style: self.style,
            weight: self.weight,
            ..Default::default()
        };

        let span: Span<Message> = Span::new(text).size(self.text_size).font(font);
        self.line.push_back(span);
    }
}

fn layout(tokens: &[Token]) -> Vec<Span<Message>> {
    let mut display_list = vec![];
    let mut text_size = DEFAULT_TEXT_SIZE_PIXELS;
    let mut style = Style::default();
    let mut weight = Weight::default();

    for token in tokens {
        match token {
            Token::Text(text) => {
                // This includes the original whitespace.
                let text_tokens =
                    unicode_segmentation::UnicodeSegmentation::split_word_bounds(text.as_str())
                        .collect::<Vec<_>>();
                for text_token in text_tokens {
                    let font = Font {
                        family: Family::Serif,
                        style,
                        weight,
                        ..Default::default()
                    };
                    let span: Span<Message, _> = Span::new(text_token).size(text_size).font(font);
                    display_list.push(span);
                }
            }

            Token::Tag(tag) => match tag.as_str() {
                "i" => {
                    style = Style::Italic;
                }
                "/i" => {
                    style = Style::default();
                }
                "b" => {
                    weight = Weight::Bold;
                }
                "/b" => {
                    weight = Weight::Normal;
                }
                "small" => {
                    text_size -= 2.;
                }
                "/small" => {
                    text_size += 2.;
                }
                "big" => {
                    text_size += 4.;
                }
                "/big" => {
                    text_size -= 4.;
                }
                "sup" => {}
                "/sup" => {}
                _ => {
                    eprintln!("Unimplemented tag: {tag}");
                }
            },
        }
    }

    display_list
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

            let tokens = tokens.unwrap_or_else(|| lex(EMPTY_BODY_TEXT, true));

            output
                .send(Message::UrlLoaded(tokens))
                .await
                .expect("Couldn't send the body");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw() -> anyhow::Result<()> {
        let url = "data:text/html,<b><i><small>Hello</small></i></b>";
        let mut engine = Engine::default();
        let tokens = engine.load(url)?;
        assert!(tokens.as_ref().is_some_and(|tokens| !tokens.is_empty()));
        #[allow(clippy::unwrap_used)]
        layout(&tokens.unwrap());
        // TODO: actually test what we get?
        Ok(())
    }
}
