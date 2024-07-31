use iced::window::Settings;
use iced::Size;

use octo_browser::Browser;

const TITLE: &str = "Octo";

fn main() -> iced::Result {
    let size = Size::new(800., 600.);
    let settings = Settings {
        size,
        ..Default::default()
    };

    iced::application(TITLE, Browser::update, Browser::view)
        .theme(Browser::theme)
        .window(settings)
        .subscription(Browser::subscription)
        .run_with(Browser::new)
}
