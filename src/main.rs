//! JAM Visualization PoC - Native Desktop App
//!
//! Run with: cargo run --bin jam-cli

#[cfg(not(target_arch = "wasm32"))]
mod app;
#[cfg(not(target_arch = "wasm32"))]
mod core;
#[cfg(not(target_arch = "wasm32"))]
mod theme;
#[cfg(not(target_arch = "wasm32"))]
mod time;
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
        .unwrap_or_else(|_| EnvFilter::new("info,jam_vis_poc=debug"));
    fmt().with_env_filter(filter).with_target(true).init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("JAM Visualizer"),
        ..Default::default()
    };

    eframe::run_native(
        "JAM Visualizer",
        options,
        Box::new(|cc| Ok(Box::new(app::JamApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
