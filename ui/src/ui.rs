use gtk::prelude::*;

use gtk::{Application, ApplicationWindow};

const TITLE: &str = "Octo";

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title(TITLE)
        .build();
    window.present();
}
