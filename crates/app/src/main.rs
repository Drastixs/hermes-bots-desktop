//! Hermes Bots Desktop: a native client for Hermes Agent Bot Mode.

mod app;
mod avatar;
mod icons;
mod model;
mod net;
mod screen_input;
mod signin;
mod store;
mod theme;
mod views;

fn main() -> iced::Result {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn,hermes_bots=info".into())).init();
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("hermes-bots {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let icon = iced::window::icon::from_file_data(theme::APP_ICON_PNG, None).ok();
    iced::application(app::App::boot, app::App::update, app::App::view)
        .title(app::App::title)
        .theme(|_: &app::App| theme::theme())
        .subscription(app::App::subscription)
        .font(theme::FONT_INTER_REGULAR)
        .font(theme::FONT_INTER_MEDIUM)
        .font(theme::FONT_INTER_SEMIBOLD)
        .font(theme::FONT_INTER_BOLD)
        .font(theme::FONT_MONO)
        .font(theme::FONT_ICONS)
        .default_font(theme::BODY)
        .window(iced::window::Settings {
            size: iced::Size::new(1280.0, 800.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            icon,
            platform_specific: iced::window::settings::PlatformSpecific { application_id: "io.github.drastixs.HermesBots".into(), ..Default::default() },
            ..Default::default()
        })
        .antialiasing(true)
        .run()
}
