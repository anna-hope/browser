use std::sync::OnceLock;
use thiserror::Error;

use iced::advanced::text::Shaping;
use iced::advanced::Widget;
use iced::font::{Family, Style, Weight};
use iced::futures::{channel::mpsc, SinkExt, Stream, StreamExt};
use iced::mouse::Cursor;
use iced::widget::canvas::{Frame, Geometry, Program};
use iced::widget::text::{LineHeight, Rich, Span};
use iced::widget::{canvas, column, row, scrollable, text_input, Column, TextInput};
use iced::{
    stream, window, Color, Element, Fill, Font, Pixels, Point, Rectangle, Renderer, Size,
    Subscription, Task, Theme,
};

use crate::engine::{Engine, EngineError};
use crate::lex::{lex, Token};

const DEFAULT_LOADING_TEXT: &str = "Loading...";
const EMPTY_BODY_TEXT: &str = "The response body was empty.";
const DEFAULT_TEXT_SIZE_PIXELS: f32 = 16.;

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

        let display_list = Layout::make_display_list(&self.tokens);
        let text = CanvasText::from_spans(&display_list);

        let content = canvas::Canvas::new(text).width(Fill);

        let scrollable_content: Element<Message> = Element::from(scrollable(content).width(Fill));
        let column: Column<_> = column![url_input, scrollable_content];
        column.width(Fill).padding(5).into()
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

type DisplayList<'a> = Vec<Span<'a, Message>>;

struct Layout<'a> {
    display_list: DisplayList<'a>,
    text_size: f32,
    style: Style,
    weight: Weight,
}

impl<'a> Layout<'a> {
    fn make_display_list(tokens: &'a [Token]) -> DisplayList {
        let mut layout = Self::default();
        layout.process_all_tokens(tokens);
        layout.display_list
    }

    fn process_text(&mut self, text: &'a str) {
        let font = Font {
            family: Family::Serif,
            style: self.style,
            weight: self.weight,
            ..Default::default()
        };

        let span: Span<Message> = Span::new(text).size(self.text_size).font(font);
        self.display_list.push(span);
    }

    fn process_token(&mut self, token: &'a Token) {
        match token {
            Token::Text(text) => {
                self.process_text(text.as_str());
            }
            Token::Tag(tag) => match tag.as_str() {
                "i" => {
                    self.style = Style::Italic;
                }
                "/i" => {
                    self.style = Style::default();
                }
                "b" => {
                    self.weight = Weight::Bold;
                }
                "/b" => {
                    self.weight = Weight::Normal;
                }
                "small" => {
                    self.text_size -= 2.;
                }
                "/small" => {
                    self.text_size += 2.;
                }
                "big" => {
                    self.text_size += 4.;
                }
                "/big" => {
                    self.text_size -= 4.;
                }
                "sup" => {}
                "/sup" => {}
                "br" => {
                    self.flush();
                }
                "/p" => {
                    self.flush();
                    // We ultimately want to add line separation here in the layout,
                    // not just a newline.
                    self.flush();
                }
                _ => {}
            },
        }
    }

    fn flush(&mut self) {
        self.display_list.push(Span::new('\n'));
    }

    fn process_all_tokens(&mut self, tokens: &'a [Token]) {
        for token in tokens {
            self.process_token(token);
        }
        self.flush();
    }
}

impl<'a> Default for Layout<'a> {
    fn default() -> Self {
        Self {
            display_list: vec![],
            text_size: DEFAULT_TEXT_SIZE_PIXELS,
            style: Style::default(),
            weight: Weight::default(),
        }
    }
}

#[derive(Debug)]
struct CanvasText {
    texts: Vec<canvas::Text>,
}

impl CanvasText {
    fn from_spans(spans: &[Span<Message>]) -> Self {
        let mut texts = Vec::with_capacity(spans.len());
        for span in spans {
            let text = canvas::Text {
                content: span.text.to_string(),
                font: span.font.unwrap_or_default(),
                size: span.size.unwrap_or(DEFAULT_TEXT_SIZE_PIXELS.into()),
                shaping: Shaping::Advanced,
                ..Default::default()
            };

            texts.push(text);
        }

        Self { texts }
    }
}

impl Program<Message> for CanvasText {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        for text in &self.texts {
            text.draw_with(|path, color| frame.fill(&path, color))
        }
        vec![frame.into_geometry()]
    }
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
        // TODO: actually test what we get?
        Ok(())
    }
}
