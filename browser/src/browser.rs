use thiserror::Error;

use eframe::egui::{Context, Visuals};
use eframe::{egui, Frame};

use crate::engine::{Engine, EngineError};
use crate::lex::{lex, Token};

const EMPTY_BODY_TEXT: &str = "The response body was empty.";
const DEFAULT_TEXT_SIZE_PIXELS: f32 = 16.;
const VSTEP: f32 = 18.;
const PADDING: f32 = 10.;

macro_rules! starting_x {
    ($ui:expr) => {
        $ui.min_rect().left()
    };
}

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

            let mut current_x = starting_x!(ui);

            // Show below the address bar
            let mut current_y = ui.min_rect().top() + response.rect.height() + PADDING;

            for item in &self.display_list {
                match item {
                    DisplayListItem::Text { text, format } => {
                        let galley = ui.painter().layout_no_wrap(
                            text.to_string(),
                            format.font_id.clone(),
                            format.color,
                        );

                        let galley_space = ui.painter().layout_no_wrap(
                            " ".to_string(),
                            format.font_id.clone(),
                            Default::default(),
                        );

                        if current_x + galley.rect.width() > ui.min_rect().width() - PADDING {
                            current_y += galley.rect.height();
                            current_x = starting_x!(ui);
                        }

                        let pos = egui::Pos2::new(current_x, current_y);

                        ui.painter().text(
                            pos,
                            egui::Align2::LEFT_TOP,
                            text.clone(),
                            format.font_id.clone(),
                            format.color,
                        );

                        current_x += galley.rect.width() + galley_space.rect.width();
                    }
                    DisplayListItem::LineBreak => {
                        current_x = starting_x!(ui);
                        current_y += VSTEP;
                    }
                }
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
enum DisplayListItem {
    Text {
        text: String,
        format: egui::TextFormat,
    },
    LineBreak,
}

impl DisplayListItem {
    fn new_text(text: String, format: egui::TextFormat) -> Self {
        Self::Text { text, format }
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
                .push(DisplayListItem::new_text(word.to_string(), format.clone()))
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
                    self.process_text(" ");
                }
                "/p" => {
                    self.process_text(" ");
                    self.display_list.push(DisplayListItem::LineBreak);
                }
                _ => {}
            },
        }
    }

    fn process_all_tokens(&mut self, tokens: Vec<Token>) {
        for token in tokens {
            self.process_token(token);
        }
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
