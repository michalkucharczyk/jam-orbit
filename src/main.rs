//! JAM Orbit - Native Desktop App
//!
//! Run with: cargo run --bin jam-orbit

#[cfg(not(target_arch = "wasm32"))]
mod app;
#[cfg(not(target_arch = "wasm32"))]
mod core;
#[cfg(not(target_arch = "wasm32"))]
mod theme;
#[cfg(not(target_arch = "wasm32"))]
mod time;
#[cfg(not(target_arch = "wasm32"))]
mod scatter;
#[cfg(not(target_arch = "wasm32"))]
mod vring;
#[cfg(not(target_arch = "wasm32"))]
mod websocket_native;
#[cfg(not(target_arch = "wasm32"))]
mod ws_state;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,jam_orbit=debug"));
    fmt().with_env_filter(filter).with_target(true).init();

    let use_cpu = std::env::args().any(|a| a == "--use-cpu");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([2400.0, 1600.0])
            .with_title("JAM Orbit"),
        ..Default::default()
    };

    eframe::run_native(
        "JAM Orbit",
        options,
        Box::new(move |cc| Ok(Box::new(app::JamApp::new(cc, use_cpu)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
