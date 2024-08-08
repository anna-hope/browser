use thiserror::Error;

use eframe::egui::{Context, Visuals};
use eframe::{egui, Frame};

use crate::engine::{Engine, EngineError};
use crate::layout::{Layout, ProcessedToken, TokenProcessor, PADDING};
use crate::lex::lex;

const EMPTY_BODY_TEXT: &str = "The response body was empty.";
const SCROLL_STEP: f32 = 100.;

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
    scroll: f32,
}

impl eframe::App for Browser {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.set_visuals(Visuals::light());

            ui.spacing_mut().text_edit_width = ui.max_rect().width();

            let response = ui.add(egui::TextEdit::singleline(&mut self.url));
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.scroll = 0.;
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

            let display_list = Layout::display_list(&self.processed_tokens, ui);

            // Account for address bar + padding.
            let top_margin = PADDING + response.rect.height();

            // Get the max_y (maximum y of all items in the display list)
            // so that we can't scroll past the bottom of the page.
            // We add that the height of that item's galley to allow a margin at the bottom,
            // and then subtract the height of the Ui rect to never scroll past a full page
            // of visible content.
            // Finally, we take the max of that (+ top_margin to account for address bar)
            // and 0 in case the height of the Ui rect
            // is larger than the max_y we get this way.
            let max_scroll = display_list
                .iter()
                .map(|item| item.pos.y + item.galley.rect.height() - ui.min_rect().height())
                .reduce(f32::max)
                .map(|max_y| f32::max(max_y + top_margin, 0.))
                .unwrap_or(ui.min_rect().bottom());

            // Scroll up
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                self.scroll = f32::max(self.scroll - SCROLL_STEP, 0.);
            }

            // Scroll down
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                self.scroll = f32::min(self.scroll + SCROLL_STEP, max_scroll);
            }

            // Mouse wheel (subtract the scroll delta instead of adding for "natural" scrolling)
            ui.input(|i| {
                self.scroll = (self.scroll - i.smooth_scroll_delta.y).clamp(0., max_scroll)
            });

            // Account for the address bar;
            for item in display_list {
                let pos = egui::Pos2::new(item.pos.x, item.pos.y - self.scroll + top_margin);
                if pos.y < top_margin || pos.y > ui.min_rect().bottom() {
                    continue;
                }
                ui.painter().galley(pos, item.galley, Default::default());
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
            scroll: 0.,
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
