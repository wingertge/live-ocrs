use iced::{multi_window::Application, Font, Settings, Size};
use iced_winit::core::window;
use live_ocrs::app::LiveOcr;
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

fn main() {
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::CLOSE)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    LiveOcr::run(Settings {
        default_font: Font::with_name("Microsoft YaHei"),
        antialiasing: true,
        window: window::Settings {
            size: Size::new(512., 400.),
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
}
