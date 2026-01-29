//! JAM Visualization PoC - Real-time telemetry dashboard
//!
//! Connects to jamtart via WebSocket and displays:
//! - Time series: num_peers over time per validator
//! - Scatter plot: best block and finalized block per validator

#![cfg(target_arch = "wasm32")]

use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod core;
mod theme;
mod websocket_wasm;

use core::{parse_event, BestBlockData, TimeSeriesData};
use theme::{colors, minimal_visuals};
use websocket_wasm::{WsClient, WsState};

/// Default WebSocket URL for jamtart
const DEFAULT_WS_URL: &str = "ws://127.0.0.1:8080/api/ws";

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();

    // Initialize tracing for browser console
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
                Box::new(|cc| Ok(Box::new(JamVisApp::new(cc)))),
            )
            .await
            .expect("Failed to start eframe");
    });
}

/// Shared state that can be updated from WebSocket callbacks
struct SharedData {
    time_series: TimeSeriesData,
    blocks: BestBlockData,
}

struct JamVisApp {
    /// Shared data updated by WebSocket callbacks
    data: Rc<RefCell<SharedData>>,
    /// WebSocket connection state
    ws_state: Rc<RefCell<WsState>>,
    /// WebSocket client (kept alive)
    #[allow(dead_code)]
    ws_client: Option<WsClient>,
    /// FPS counter
    fps_counter: FpsCounter,
    /// UI state
    paused: bool,
}

impl JamVisApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());

        // Create shared data structures
        let data = Rc::new(RefCell::new(SharedData {
            time_series: TimeSeriesData::new(1024, 200),
            blocks: BestBlockData::new(1024),
        }));

        // Create WebSocket state
        let ws_state = Rc::new(RefCell::new(WsState::Connecting));

        // Connect WebSocket with callback that updates shared data
        let data_clone = data.clone();
        let ws_client = WsClient::connect(
            DEFAULT_WS_URL,
            move |msg| {
                let mut data = data_clone.borrow_mut();
                let SharedData {
                    ref mut time_series,
                    ref mut blocks,
                } = *data;
                parse_event(&msg, time_series, blocks);
            },
            ws_state.clone(),
        )
        .ok();

        Self {
            data,
            ws_state,
            ws_client,
            fps_counter: FpsCounter::new(),
            paused: false,
        }
    }
}

impl eframe::App for JamVisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaint for real-time updates
        ctx.request_repaint();

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY))
            .show(ctx, |ui| {
                self.render_header(ui);

                ui.add_space(8.0);

                let available = ui.available_size();
                let plot_height = (available.y - 20.0) / 2.0;

                // Top row: Time series (full width)
                ui.allocate_ui(egui::vec2(available.x, plot_height), |ui| {
                    self.render_time_series(ui);
                });

                ui.add_space(10.0);

                // Bottom row: Block scatter plots side by side
                ui.horizontal(|ui| {
                    let half_width = (available.x - 10.0) / 2.0;

                    ui.allocate_ui(egui::vec2(half_width, plot_height - 20.0), |ui| {
                        self.render_best_blocks(ui);
                    });

                    ui.add_space(10.0);

                    ui.allocate_ui(egui::vec2(half_width, plot_height - 20.0), |ui| {
                        self.render_finalized_blocks(ui);
                    });
                });
            });
    }
}

impl JamVisApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        self.fps_counter.tick();

        let data = self.data.borrow();
        let ws_state = self.ws_state.borrow();

        ui.horizontal(|ui| {
            // Connection status indicator
            let (status_color, status_text) = match &*ws_state {
                WsState::Connected => (egui::Color32::from_rgb(100, 200, 100), "Connected"),
                WsState::Connecting => (egui::Color32::from_rgb(200, 200, 100), "Connecting..."),
                WsState::Disconnected => (egui::Color32::from_rgb(200, 100, 100), "Disconnected"),
                WsState::Error(_) => (egui::Color32::from_rgb(200, 100, 100), "Error"),
            };

            ui.colored_label(status_color, egui::RichText::new(status_text).size(11.0));

            ui.add_space(10.0);

            ui.label(
                egui::RichText::new(format!("{:.0} fps", self.fps_counter.fps()))
                    .color(colors::TEXT_SECONDARY)
                    .monospace()
                    .size(11.0),
            );

            ui.label(
                egui::RichText::new("/")
                    .color(colors::TEXT_MUTED)
                    .size(11.0),
            );

            // Show validator count
            let validator_count = data.time_series.validator_count();
            ui.label(
                egui::RichText::new(format!("{} validators", validator_count))
                    .color(colors::TEXT_MUTED)
                    .monospace()
                    .size(11.0),
            );

            // Show highest slot
            if let Some(slot) = data.blocks.highest_slot() {
                ui.label(
                    egui::RichText::new("/")
                        .color(colors::TEXT_MUTED)
                        .size(11.0),
                );
                ui.label(
                    egui::RichText::new(format!("slot {}", slot))
                        .color(colors::TEXT_MUTED)
                        .monospace()
                        .size(11.0),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("JAM")
                        .color(colors::TEXT_PRIMARY)
                        .size(12.0),
                );
            });
        });
    }

    fn render_time_series(&self, ui: &mut egui::Ui) {
        use egui_plot::{Line, Plot, PlotPoints};

        let data = self.data.borrow();
        let point_count = data.time_series.point_count();

        // Calculate Y bounds from actual data
        let (y_min, y_max) = data
            .time_series
            .series
            .iter()
            .flat_map(|s| s.iter())
            .fold((f32::MAX, f32::MIN), |(min, max), &v| {
                (min.min(v), max.max(v))
            });

        // Default bounds if no data
        let (y_min, y_max) = if y_min > y_max {
            (0.0, 100.0)
        } else {
            let pad = (y_max - y_min).max(10.0) * 0.1;
            (y_min - pad, y_max + pad)
        };

        Plot::new("time_series")
            .show_axes([true, true])
            .show_grid(true)
            .allow_zoom(true)
            .allow_drag(true)
            .show_background(false)
            .include_x(0.0)
            .include_x(point_count.max(1) as f64)
            .include_y(y_min as f64)
            .include_y(y_max as f64)
            .y_axis_label("num_peers")
            .show(ui, |plot_ui| {
                for series in &data.time_series.series {
                    if series.len() < 2 {
                        continue;
                    }

                    let points: PlotPoints = series
                        .iter()
                        .enumerate()
                        .map(|(x, &y)| [x as f64, y as f64])
                        .collect();

                    let color =
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, colors::LINE_ALPHA);
                    plot_ui.line(Line::new(points).color(color).width(1.0));
                }
            });
    }

    fn render_best_blocks(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        let data = self.data.borrow();

        // Find max block for Y axis
        let max_block = data.blocks.highest_slot().unwrap_or(1) as f64;

        Plot::new("best_blocks")
            .show_axes([true, true])
            .show_grid(true)
            .allow_zoom(true)
            .allow_drag(true)
            .show_background(false)
            .include_x(0.0)
            .include_x(1024.0)
            .include_y(max_block - 10.0)
            .include_y(max_block + 5.0)
            .x_axis_label("Validator ID")
            .y_axis_label("Best Block Slot")
            .show(ui, |plot_ui| {
                let points: PlotPoints = data
                    .blocks
                    .best_blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, &slot)| slot > 0)
                    .map(|(id, &slot)| [id as f64, slot as f64])
                    .collect();

                plot_ui.points(
                    Points::new(points)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180))
                        .radius(2.0)
                        .filled(true),
                );
            });
    }

    fn render_finalized_blocks(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        let data = self.data.borrow();

        // Find max finalized for Y axis
        let max_finalized = data.blocks.highest_finalized().unwrap_or(1) as f64;

        Plot::new("finalized_blocks")
            .show_axes([true, true])
            .show_grid(true)
            .allow_zoom(true)
            .allow_drag(true)
            .show_background(false)
            .include_x(0.0)
            .include_x(1024.0)
            .include_y(max_finalized - 10.0)
            .include_y(max_finalized + 5.0)
            .x_axis_label("Validator ID")
            .y_axis_label("Finalized Slot")
            .show(ui, |plot_ui| {
                let points: PlotPoints = data
                    .blocks
                    .finalized_blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, &slot)| slot > 0)
                    .map(|(id, &slot)| [id as f64, slot as f64])
                    .collect();

                // Grey color for finalized blocks
                plot_ui.points(
                    Points::new(points)
                        .color(egui::Color32::from_rgba_unmultiplied(150, 150, 150, 180))
                        .radius(2.0)
                        .filled(true),
                );
            });
    }
}

struct FpsCounter {
    frames: Vec<f64>,
}

impl FpsCounter {
    fn new() -> Self {
        Self {
            frames: Vec::with_capacity(60),
        }
    }

    fn tick(&mut self) {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);

        self.frames.push(now);
        if self.frames.len() > 60 {
            self.frames.remove(0);
        }
    }

    fn fps(&self) -> f64 {
        if self.frames.len() < 2 {
            return 0.0;
        }
        let elapsed = self.frames.last().unwrap() - self.frames.first().unwrap();
        if elapsed == 0.0 {
            return 0.0;
        }
        (self.frames.len() as f64 - 1.0) / (elapsed / 1000.0)
    }
}
