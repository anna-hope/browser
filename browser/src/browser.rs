use std::sync::Arc;
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
    processed_tokens: Vec<ProcessedToken>,
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
                        self.processed_tokens =
                            TokenProcessor::from_tokens(tokens).processed_tokens;
                    }
                    Ok(None) => {
                        self.processed_tokens =
                            TokenProcessor::from_tokens(lex(EMPTY_BODY_TEXT, true))
                                .processed_tokens;
                    }
                    Err(error) => {
                        ui.label(error.to_string());
                    }
                }
            }

            let display_list =
                Layout::display_list(&self.processed_tokens, ui, PADDING + response.rect.height());

            for item in display_list {
                ui.painter().text(
                    item.pos,
                    egui::Align2::LEFT_TOP,
                    item.text_with_format.text.clone(),
                    item.text_with_format.format.font_id.clone(),
                    item.text_with_format.format.color,
                );
            }
        });
    }
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            url: "about:blank".to_string(),
            engine: Default::default(),
            processed_tokens: vec![],
        }
    }
}

#[derive(Debug, Clone)]
struct TextWithFormat {
    text: String,
    format: egui::TextFormat,
}

impl TextWithFormat {
    fn new(text: String, format: egui::TextFormat) -> Self {
        Self { text, format }
    }
}

#[derive(Debug, Clone)]
enum ProcessedToken {
    Text(TextWithFormat),
    LineBreak,
}

struct TokenProcessor {
    processed_tokens: Vec<ProcessedToken>,
    text_size: f32,
    italics: bool,
    color: egui::Color32,
}

impl Default for TokenProcessor {
    fn default() -> Self {
        Self {
            processed_tokens: vec![],
            text_size: DEFAULT_TEXT_SIZE_PIXELS,
            italics: false,
            color: egui::Color32::BLACK,
        }
    }
}

impl TokenProcessor {
    fn from_tokens(tokens: Vec<Token>) -> Self {
        let mut layout = Self::default();
        layout.process_all_tokens(tokens);
        layout
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
            self.processed_tokens
                .push(ProcessedToken::Text(TextWithFormat::new(
                    word.to_string(),
                    format.clone(),
                )))
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
                    self.process_text("\n");
                }
                "/p" => {
                    self.process_text("\n");
                    self.processed_tokens.push(ProcessedToken::LineBreak);
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

struct LineItem<'a> {
    text_with_format: &'a TextWithFormat,
    galley: Arc<egui::Galley>,
    x: f32,
}

impl<'a> LineItem<'a> {
    fn new(text_with_format: &'a TextWithFormat, galley: Arc<egui::Galley>, x: f32) -> Self {
        Self {
            galley,
            text_with_format,
            x,
        }
    }
}

struct DisplayListItem<'a> {
    text_with_format: &'a TextWithFormat,
    pos: egui::Pos2,
}

impl<'a> DisplayListItem<'a> {
    fn new(text: &'a TextWithFormat, pos: egui::Pos2) -> Self {
        Self {
            text_with_format: text,
            pos,
        }
    }
}

type DisplayList<'a> = Vec<DisplayListItem<'a>>;

struct Layout<'a, 'b> {
    display_list: DisplayList<'a>,
    line: Vec<LineItem<'a>>,
    ui: &'b egui::Ui,
    current_x: f32,
    current_y: f32,
}

impl<'a, 'b> Layout<'a, 'b> {
    fn display_list(
        processed_tokens: &'a [ProcessedToken],
        ui: &'b egui::Ui,
        padding_top: f32,
    ) -> DisplayList<'a> {
        let mut layout = Layout {
            display_list: vec![],
            line: vec![],
            ui,
            current_x: starting_x!(ui),
            current_y: ui.min_rect().top() + padding_top,
        };

        for token in processed_tokens {
            layout.push_to_line(token);
        }

        layout.flush();
        layout.display_list
    }

    fn push_to_line(&mut self, token: &'a ProcessedToken) {
        match token {
            ProcessedToken::Text(text) => {
                let galley = self.ui.painter().layout_no_wrap(
                    text.text.to_string(),
                    text.format.font_id.clone(),
                    text.format.color,
                );

                let galley_space = self.ui.painter().layout_no_wrap(
                    " ".to_string(),
                    text.format.font_id.clone(),
                    Default::default(),
                );

                if self.current_x + galley.rect.width() > self.ui.min_rect().width() - PADDING {
                    self.flush();
                }

                let line_item = LineItem::new(text, Arc::clone(&galley), self.current_x);
                self.line.push(line_item);
                self.current_x += galley.rect.width() + galley_space.rect.width();
            }
            ProcessedToken::LineBreak => {
                self.flush();
                self.current_y += VSTEP;
            }
        }
    }

    fn flush(&mut self) {
        // Get the maximum height of all the galleys in the current line.
        let max_ascent = self
            .line
            .iter()
            .flat_map(|item| get_max_ascent(&item.galley))
            .reduce(f32::max);

        let max_descent = self
            .line
            .iter()
            .map(|item| item.galley.mesh_bounds.bottom() - item.galley.mesh_bounds.center().y)
            .reduce(f32::max);

        if let (Some(max_ascent), Some(max_descent)) = (max_ascent, max_descent) {
            let baseline = self.current_y + 1.25 * max_ascent;

            for line_item in &self.line {
                let ascent = get_max_ascent(&line_item.galley).unwrap_or_default();
                let y = baseline - ascent;
                let pos = egui::Pos2::new(line_item.x, y);
                self.display_list
                    .push(DisplayListItem::new(line_item.text_with_format, pos));
            }

            self.current_y = baseline + 1.25 * max_descent;
            self.current_x = starting_x!(self.ui);
            self.line.clear();
        }
    }
}

#[inline]
fn get_max_ascent(galley: &egui::Galley) -> Option<f32> {
    galley
        .rows
        .iter()
        .flat_map(|row| &row.glyphs)
        .map(|glyph| glyph.ascent)
        .reduce(f32::max)
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
