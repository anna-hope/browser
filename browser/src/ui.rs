use gtk::prelude::IsA;
use gtk::{Application, ApplicationWindow, ScrolledWindow, TextBuffer, TextView, Widget};

const TITLE: &str = "Octo";

pub fn build_text_view() -> TextView {
    TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .build()
}

pub fn build_scrolled_window(children: &[&impl IsA<Widget>]) -> ScrolledWindow {
    let scrolled_window = ScrolledWindow::builder().build();

    for child in children {
        scrolled_window.set_child(Some(*child));
    }
    scrolled_window
}

pub fn build_ui(app: &Application) -> ApplicationWindow {
    let app_window = ApplicationWindow::builder()
        .application(app)
        .width_request(800)
        .height_request(600)
        .title(TITLE)
        .build();
    todo!()
}
