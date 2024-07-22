use gtk::prelude::*;

use gtk::{Application, ApplicationWindow};

const TITLE: &str = "Octo";

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .width_request(800)
        .height_request(600)
        .title(TITLE)
        .build();
    window.present();
}
