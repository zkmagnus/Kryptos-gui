#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod kryptos;

fn main() -> iced::Result {
    iced::application(kryptos::App::new, kryptos::App::update, kryptos::view::view)
        .title("Kryptos")
        .theme(kryptos::view::theme)
        .subscription(kryptos::App::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(1200.0, 850.0),
            min_size: Some(iced::Size::new(1000.0, 700.0)),
            exit_on_close_request: false,
            ..Default::default()
        })
        .font(include_bytes!("../assets/font/BerkeleyMono-Regular.ttf").as_slice())
        .run()
}
