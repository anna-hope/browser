use eframe::egui;

use octo_browser::Browser;

const TITLE: &str = "Octo";

fn main() -> eframe::Result {
    let size = [800., 600.];
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(size),
        ..Default::default()
    };

    eframe::run_native(TITLE, options, Box::new(|_| Ok(Box::<Browser>::default())))
}
