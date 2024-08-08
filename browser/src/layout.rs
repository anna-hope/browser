use crate::lex::Token;
use std::sync::Arc;

use eframe::egui;

const VSTEP: f32 = 18.;
const DEFAULT_TEXT_SIZE_PIXELS: f32 = 16.;
pub(crate) const PADDING: f32 = 10.;

macro_rules! starting_x {
    ($ui:expr) => {
        $ui.min_rect().left()
    };
}

#[derive(Debug, Clone)]
pub(crate) enum ProcessedToken {
    Text(egui::text::LayoutJob),
    LineBreak,
}

pub(crate) struct DisplayListItem {
    pub(crate) galley: Arc<egui::Galley>,
    pub(crate) pos: egui::Pos2,
}

impl DisplayListItem {
    fn new(galley: Arc<egui::Galley>, pos: egui::Pos2) -> Self {
        Self { galley, pos }
    }
}

type DisplayList = Vec<DisplayListItem>;

pub(crate) struct Layout<'a> {
    display_list: DisplayList,
    line: Vec<LineItem>,
    ui: &'a egui::Ui,
    current_x: f32,
    current_y: f32,
}

impl<'a> Layout<'a> {
    pub(crate) fn display_list(
        processed_tokens: &[ProcessedToken],
        ui: &'a egui::Ui,
    ) -> DisplayList {
        let mut layout = Layout {
            display_list: vec![],
            line: vec![],
            ui,
            current_x: starting_x!(ui),
            current_y: ui.min_rect().top(),
        };

        for token in processed_tokens {
            layout.push_to_line(token);
        }

        layout.flush();
        layout.display_list
    }

    fn push_to_line(&mut self, token: &ProcessedToken) {
        match token {
            ProcessedToken::Text(layout_job) => {
                let galley = self.ui.painter().layout_job(layout_job.clone());
                let font_id = layout_job
                    .sections
                    .first()
                    .map(|s| s.format.font_id.clone())
                    .unwrap_or_default();

                let galley_space =
                    self.ui
                        .painter()
                        .layout_no_wrap(" ".to_string(), font_id, Default::default());

                if self.current_x + galley.rect.width() > self.ui.min_rect().width() - PADDING {
                    self.flush();
                }

                let line_item = LineItem::new(Arc::clone(&galley), self.current_x);
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
                    .push(DisplayListItem::new(Arc::clone(&line_item.galley), pos));
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

pub(crate) struct TokenProcessor {
    pub(crate) processed_tokens: Vec<ProcessedToken>,
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
    pub(crate) fn from_tokens(tokens: Vec<Token>) -> Self {
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
            let mut layout_job = egui::text::LayoutJob::default();
            layout_job.append(word, 0.0, format.clone());
            self.processed_tokens.push(ProcessedToken::Text(layout_job));
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

struct LineItem {
    galley: Arc<egui::Galley>,
    x: f32,
}

impl LineItem {
    fn new(galley: Arc<egui::Galley>, x: f32) -> Self {
        Self { galley, x }
    }
}
