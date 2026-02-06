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

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

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
