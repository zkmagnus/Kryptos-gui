mod rhombus;

fn main() -> iced::Result {
    iced::application(rhombus::App::new, rhombus::App::update, rhombus::view::view)
        .title("Rhombus")
        .theme(rhombus::view::theme)
        .subscription(rhombus::App::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(1200.0, 850.0),
            min_size: Some(iced::Size::new(1000.0, 700.0)),
            ..Default::default()
        })
        .font(include_bytes!("../assets/font/BerkeleyMono-Regular.ttf").as_slice())
        .run()
}
