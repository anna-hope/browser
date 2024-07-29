use gtk::prelude::{GtkWindowExt, IsA};
use gtk::{
    pango, Application, ApplicationWindow, ScrolledWindow, TextBuffer, TextTag, TextView, Widget,
};

const TITLE: &str = "Octo";
const DEFAULT_FONT_FAMILY: &str = "Sans-Serif";
const DEFAULT_FONT_SIZE: i32 = 16;
const DEFAULT_FONT_WEIGHT: i32 = 400;

#[derive(Debug, Copy, Clone)]
pub(crate) struct FontSize(i32);

impl FontSize {
    pub(crate) fn small() -> Self {
        Self((DEFAULT_FONT_SIZE - 2) * pango::SCALE)
    }

    pub(crate) fn big() -> Self {
        Self((DEFAULT_FONT_SIZE + 4) * pango::SCALE)
    }
}

impl Default for FontSize {
    fn default() -> Self {
        Self(DEFAULT_FONT_SIZE * pango::SCALE)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct FontWeight {
    pub(crate) weight: i32,
}

impl Default for FontWeight {
    fn default() -> Self {
        Self {
            weight: DEFAULT_FONT_WEIGHT,
        }
    }
}

pub(crate) struct TextTagConfig<'a> {
    size: i32,
    weight: i32,
    style: pango::Style,
    family: &'a str,
}

impl<'a> TextTagConfig<'a> {
    pub(crate) fn new(
        size: FontSize,
        weight: FontWeight,
        style: pango::Style,
        family: Option<&'a str>,
    ) -> Self {
        Self {
            size: size.0,
            weight: weight.weight,
            style,
            family: family.unwrap_or(DEFAULT_FONT_FAMILY),
        }
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

pub(crate) fn build_text_tag(name: &str, text_tag_config: &TextTagConfig) -> TextTag {
    TextTag::builder()
        .name(name)
        .size(text_tag_config.size)
        .weight(text_tag_config.weight)
        .style(text_tag_config.style)
        .family(text_tag_config.family)
        .build()
}
