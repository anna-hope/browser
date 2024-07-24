use gtk::prelude::{GtkWindowExt, IsA};
use gtk::{Application, ApplicationWindow, ScrolledWindow, TextBuffer, TextView, Widget};

const TITLE: &str = "Octo";

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
