use std::borrow::Cow;
use std::sync::{mpsc, Arc};
use thiserror::Error;

use eframe::egui::scroll_area::ScrollBarVisibility;
use eframe::egui::{Context, Visuals};
use eframe::{egui, Frame};

use crate::engine::{Engine, EngineError};
use crate::lex::{lex, Token};

const DEFAULT_LOADING_TEXT: &str = "Loading...";
const EMPTY_BODY_TEXT: &str = "The response body was empty.";
const DEFAULT_TEXT_SIZE_PIXELS: f32 = 16.;
const HSTEP: f32 = 13.;
const VSTEP: f32 = 18.;
const PADDING: f32 = 10.;

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),
}

#[derive(Debug)]
pub struct Browser {
    url: String,
    engine: Engine,
    display_list: DisplayList,
}

impl eframe::App for Browser {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.set_visuals(Visuals::light());

            ui.spacing_mut().text_edit_width = ui.max_rect().width();

            let response = ui.add(egui::TextEdit::singleline(&mut self.url));
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                match self.engine.load(&self.url) {
                    Ok(Some(tokens)) => {
                        self.display_list = Layout::display_list(tokens);
                    }
                    Ok(None) => {
                        self.display_list = Layout::display_list(lex(EMPTY_BODY_TEXT, true));
                    }
                    Err(error) => {
                        ui.label(error.to_string());
                    }
                }
            }

            let mut current_x = ui.min_rect().left();

            // Show below the address bar
            let mut current_y = ui.min_rect().top() + response.rect.height() + PADDING;

            for item in &self.display_list {
                let galley = ui.painter().layout_no_wrap(
                    item.text.to_string(),
                    item.format.font_id.clone(),
                    item.format.color,
                );

                let galley_space = ui.painter().layout_no_wrap(
                    " ".to_string(),
                    item.format.font_id.clone(),
                    Default::default(),
                );

                if current_x + galley.rect.width() > ui.min_rect().width() - PADDING {
                    current_y += galley.rect.height();
                    current_x = ui.min_rect().left();
                }

                let pos = egui::Pos2::new(current_x, current_y);

                ui.painter().text(
                    pos,
                    egui::Align2::LEFT_TOP,
                    item.text.clone(),
                    item.format.font_id.clone(),
                    item.format.color,
                );
                current_x += galley.rect.width() + galley_space.rect.width();
            }
        });
    }
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            url: "about:blank".to_string(),
            engine: Default::default(),
            display_list: vec![],
        }
    }
}

#[derive(Debug, Clone)]
struct DisplayListItem {
    text: String,
    format: egui::TextFormat,
}

impl DisplayListItem {
    fn new(text: String, format: egui::TextFormat) -> Self {
        Self { text, format }
    }
}

type DisplayList = Vec<DisplayListItem>;

struct Layout {
    display_list: DisplayList,
    text_size: f32,
    italics: bool,
    color: egui::Color32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            display_list: vec![],
            text_size: DEFAULT_TEXT_SIZE_PIXELS,
            italics: false,
            color: egui::Color32::BLACK,
        }
    }
}

impl Layout {
    fn display_list(tokens: Vec<Token>) -> DisplayList {
        let mut layout = Self::default();
        layout.process_all_tokens(tokens);
        layout.display_list
    }

    fn process_text(&mut self, text: &str) {
        let font_id = egui::FontId::new(self.text_size, egui::FontFamily::Proportional);
        let format = egui::text::TextFormat {
            font_id,
            italics: self.italics,
            color: self.color,
            valign: egui::Align::Min,
            ..Default::default()
        };
        for word in text.split_whitespace() {
            self.display_list
                .push(DisplayListItem::new(word.to_string(), format.clone()))
        }
    }

    fn process_token(&mut self, token: Token) {
        match token {
            Token::Text(text) => {
                self.process_text(text.as_str());
            }
            Token::Tag(tag) => match tag.as_str() {
                "i" => {
                    self.italics = true;
                }
                "/i" => {
                    self.italics = false;
                }
                "b" => {
                    self.color = egui::Color32::BLACK;
                }
                "/b" => {
                    self.color = Default::default();
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
        self.display_list
            .push(DisplayListItem::new("\n".to_string(), Default::default()))
    }

    fn process_all_tokens(&mut self, tokens: Vec<Token>) {
        for token in tokens {
            self.process_token(token);
        }
        self.flush();
    }
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
