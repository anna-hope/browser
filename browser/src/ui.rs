use gtk::prelude::{GtkWindowExt, IsA};
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
    pub(crate) fn new_from_points(size_points: i32) -> Self {
        Self(size_points)
    }

    pub(crate) fn decrease(&self, by_points: i32) -> Self {
        Self(self.0 - by_points)
    }

    pub(crate) fn increase(&self, by_points: i32) -> Self {
        Self(self.0 + by_points)
    }

    #[inline]
    pub(crate) fn points(&self) -> i32 {
        self.0
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

#[derive(Copy, Clone, Debug)]
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

#[derive(Debug, Clone)]
pub(crate) struct TextTagConfig {
    pub(crate) size: FontSize,
    pub(crate) weight: FontWeight,
    pub(crate) style: pango::Style,
    pub(crate) family: String,
    pub(crate) scale: f64,
    pub(crate) rise_scaled: i32,
}

impl Default for TextTagConfig {
    fn default() -> Self {
        Self {
            size: FontSize::default(),
            weight: FontWeight::default(),
            style: pango::Style::Normal,
            family: DEFAULT_FONT_FAMILY.to_string(),
            scale: pango::SCALE_MEDIUM,
            rise_scaled: 0,
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

pub(crate) fn build_text_tag(name: &str, text_tag_config: &TextTagConfig) -> TextTag {
    TextTag::builder()
        .name(name)
        .size(text_tag_config.size.scaled())
        .weight(text_tag_config.weight.into())
        .style(text_tag_config.style)
        .family(text_tag_config.family.as_str())
        .scale(text_tag_config.scale)
        .rise(text_tag_config.rise_scaled)
        .build()
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
