use iced::{Application, Font, Settings};
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
        ..Default::default()
    })
    .unwrap();
}
