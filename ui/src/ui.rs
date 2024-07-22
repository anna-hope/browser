use gtk::prelude::*;

use gtk::{Application, ApplicationWindow, TextBuffer, TextView};

const TITLE: &str = "Octo";

pub fn build_text_view(text: &str) -> TextView {
    let buffer = TextBuffer::builder().text(text).build();
    TextView::builder()
        .editable(false)
        .buffer(&buffer)
        .cursor_visible(false)
        .build()
}

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .width_request(800)
        .height_request(600)
        .title(TITLE)
        .build();

    let text_view = build_text_view("test");
    window.set_child(Some(&text_view));

    window.present();
}
