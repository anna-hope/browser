use gtk::prelude::{GtkWindowExt, IsA, TextTagExt};
use gtk::{
    pango, Application, ApplicationWindow, ScrolledWindow, TextBuffer, TextTag, TextView, Widget,
};

const TITLE: &str = "Octo";
const DEFAULT_FONT_FAMILY: &str = "Sans-Serif";
const DEFAULT_FONT_SIZE: i32 = 16;
const DEFAULT_FONT_WEIGHT: i32 = 400;
const BOLD_FONT_WEIGHT: i32 = 800;

#[derive(Debug, Copy, Clone)]
pub(crate) struct FontSize(i32);

impl FontSize {
    pub(crate) fn small(&self) -> Self {
        Self(self.0 - 2)
    }

    pub(crate) fn big(&self) -> Self {
        Self(self.0 + 4)
    }

    #[inline]
    pub(crate) fn scaled(&self) -> i32 {
        self.0 * pango::SCALE
    }
}

impl Default for FontSize {
    fn default() -> Self {
        Self(DEFAULT_FONT_SIZE)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct FontWeight {
    pub(crate) weight: i32,
}

impl FontWeight {
    pub(crate) fn bold() -> Self {
        Self {
            weight: BOLD_FONT_WEIGHT,
        }
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self {
            weight: DEFAULT_FONT_WEIGHT,
        }
    }
}

impl From<FontWeight> for i32 {
    fn from(value: FontWeight) -> Self {
        value.weight
    }
}

pub(crate) struct TextTagConfig<'a> {
    size: FontSize,
    weight: FontWeight,
    style: pango::Style,
    family: &'a str,
    scale: f64,
    superscript: bool,
}

impl<'a> TextTagConfig<'a> {
    pub(crate) fn new(
        size: FontSize,
        weight: FontWeight,
        style: pango::Style,
        family: Option<&'a str>,
    ) -> Self {
        Self {
            size,
            weight,
            style,
            family: family.unwrap_or(DEFAULT_FONT_FAMILY),
            scale: pango::SCALE_MEDIUM,
            superscript: false,
        }
    }

    pub(crate) fn with_superscript(mut self) -> Self {
        self.superscript = true;
        self
    }
}

fn build_text_view(text_buffer: &TextBuffer) -> TextView {
    TextView::builder()
        .buffer(text_buffer)
        .editable(false)
        .cursor_visible(false)
        .build()
}

fn build_scrolled_window(children: &[&impl IsA<Widget>]) -> ScrolledWindow {
    let scrolled_window = ScrolledWindow::builder().build();

    for child in children {
        scrolled_window.set_child(Some(*child));
    }
    scrolled_window
}

pub(crate) fn build_text_tag(name: &str, text_tag_config: &TextTagConfig) -> TextTag {
    let text_tag = TextTag::builder()
        .name(name)
        .size(text_tag_config.size.scaled())
        .weight(text_tag_config.weight.into())
        .style(text_tag_config.style)
        .family(text_tag_config.family)
        .scale(text_tag_config.scale)
        .build();

    if text_tag_config.superscript {
        text_tag.set_scale(0.5);
        // Idk if this is actually the correct way to calculate this.
        let rise = text_tag.size() / 4;
        text_tag.set_rise(rise);
    }

    text_tag
}

pub fn build_ui(app: &Application, text_buffer: &TextBuffer) {
    let app_window = ApplicationWindow::builder()
        .application(app)
        .width_request(800)
        .height_request(600)
        .title(TITLE)
        .build();

    let text_view = build_text_view(text_buffer);
    let scrolled_window = build_scrolled_window(&[&text_view]);
    app_window.set_child(Some(&scrolled_window));
    app_window.present()
}
