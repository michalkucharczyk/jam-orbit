//! JAM Visualization PoC - Real-time telemetry dashboard
//!
//! WASM entry point. Connects to jamtart via WebSocket and displays real-time graphs.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod app;
mod core;
mod theme;
mod time;
mod scatter;
mod vring;
mod websocket_wasm;
mod ws_state;

use app::JamApp;

/// Called once from JS to set up panic hook and tracing.
/// The WASM module is loaded eagerly but the app is NOT started until `start()`.
#[wasm_bindgen(start)]
pub fn init_runtime() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
}

/// Start the egui app. Called from JS after the user clicks Connect.
#[wasm_bindgen]
pub fn start() {
    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document")
            .get_element_by_id("canvas")
            .expect("no canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("not a canvas element");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(JamApp::new(cc)))),
            )
            .await
            .expect("Failed to start eframe");
    });
}
