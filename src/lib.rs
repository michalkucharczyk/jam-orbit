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

use core::{parse_event, BestBlockData, EventStore, TimeSeriesData, EVENT_CATEGORIES};
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
    events: EventStore,
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
    /// Event filter: [event_type] = enabled (200 slots for all event types)
    selected_events: Vec<bool>,
    /// Toggle event selector panel visibility
    show_event_selector: bool,
}

impl JamVisApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());

        // Create shared data structures
        let data = Rc::new(RefCell::new(SharedData {
            time_series: TimeSeriesData::new(1024, 200),
            blocks: BestBlockData::new(1024),
            events: EventStore::new(50000, 60.0), // 50K events, 60s retention
        }));

        // Create WebSocket state
        let ws_state = Rc::new(RefCell::new(WsState::Connecting));

        // Connect WebSocket with callback that updates shared data
        let data_clone = data.clone();
        let ws_client = WsClient::connect(
            DEFAULT_WS_URL,
            move |msg| {
                // Get current time relative to app start (in seconds)
                let now = web_sys::window()
                    .and_then(|w| w.performance())
                    .map(|p| p.now() / 1000.0) // Convert ms to seconds
                    .unwrap_or(0.0);

                let mut data = data_clone.borrow_mut();
                let SharedData {
                    ref mut time_series,
                    ref mut blocks,
                    ref mut events,
                } = *data;
                parse_event(&msg, time_series, blocks, events, now);
            },
            ws_state.clone(),
        )
        .ok();

        // Initialize event filter: all events enabled by default
        let mut selected_events = vec![false; 200];
        // Enable Status events by default for visibility
        for &et in &[10, 11, 12, 13] {
            selected_events[et] = true;
        }

        Self {
            data,
            ws_state,
            ws_client,
            fps_counter: FpsCounter::new(),
            selected_events,
            show_event_selector: false,
        }
    }

    /// Get current time in seconds (app-relative)
    fn current_time(&self) -> f64 {
        web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now() / 1000.0)
            .unwrap_or(0.0)
    }
}

impl eframe::App for JamVisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaint for real-time updates
        ctx.request_repaint();

        // Prune old events periodically
        let now = self.current_time();
        self.data.borrow_mut().events.prune(now);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY))
            .show(ctx, |ui| {
                self.render_header(ui);

                // Show event selector if toggled
                if self.show_event_selector {
                    ui.add_space(4.0);
                    self.render_event_selector(ui);
                }

                ui.add_space(8.0);

                let available = ui.available_size();
                // With 5 graphs: time series, particles, rates, 2x blocks
                let graph_height = (available.y - 40.0) / 5.0;

                // Time series (num_peers)
                ui.allocate_ui(egui::vec2(available.x, graph_height), |ui| {
                    self.render_time_series(ui);
                });

                ui.add_space(4.0);

                // Particle trails (events as drifting particles)
                ui.allocate_ui(egui::vec2(available.x, graph_height), |ui| {
                    self.render_particle_trails(ui);
                });

                ui.add_space(4.0);

                // Event rate lines (events/sec per node)
                ui.allocate_ui(egui::vec2(available.x, graph_height), |ui| {
                    self.render_event_rates(ui);
                });

                ui.add_space(4.0);

                // Bottom row: Block scatter plots side by side
                ui.horizontal(|ui| {
                    let half_width = (available.x - 10.0) / 2.0;

                    ui.allocate_ui(egui::vec2(half_width, graph_height * 2.0 - 10.0), |ui| {
                        self.render_best_blocks(ui);
                    });

                    ui.add_space(10.0);

                    ui.allocate_ui(egui::vec2(half_width, graph_height * 2.0 - 10.0), |ui| {
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

            // Show event count
            let event_count = data.events.node_count();
            ui.label(
                egui::RichText::new("/")
                    .color(colors::TEXT_MUTED)
                    .size(11.0),
            );
            ui.label(
                egui::RichText::new(format!("{} nodes", event_count))
                    .color(colors::TEXT_MUTED)
                    .monospace()
                    .size(11.0),
            );

            // Need to drop borrow before we can modify self
            drop(ws_state);
            drop(data);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("JAM")
                        .color(colors::TEXT_PRIMARY)
                        .size(12.0),
                );

                ui.add_space(10.0);

                // Filter toggle button
                let filter_text = if self.show_event_selector {
                    "Filter ▲"
                } else {
                    "Filter ▼"
                };
                if ui
                    .button(egui::RichText::new(filter_text).size(11.0))
                    .clicked()
                {
                    self.show_event_selector = !self.show_event_selector;
                }
            });
        });
    }

    fn render_time_series(&self, ui: &mut egui::Ui) {
        use egui_plot::{Line, Plot, PlotPoints};

        // Title
        ui.label(
            egui::RichText::new("Peer Count")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

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
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(point_count.max(1) as f64)
            .include_y(y_min as f64)
            .include_y(y_max as f64)
            .label_formatter(|_name, value| {
                format!("t={} peers={:.0}", value.x as u32, value.y)
            })
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

        // Title
        ui.label(
            egui::RichText::new("Best Block")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let data = self.data.borrow();

        // Find max block for Y axis
        let max_block = data.blocks.highest_slot().unwrap_or(1) as f64;

        Plot::new("best_blocks")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(1024.0)
            .include_y(max_block - 10.0)
            .include_y(max_block + 5.0)
            .label_formatter(|_name, value| {
                format!("validator={} slot={:.0}", value.x as u32, value.y)
            })
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

        // Title
        ui.label(
            egui::RichText::new("Finalized Block")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let data = self.data.borrow();

        // Find max finalized for Y axis
        let max_finalized = data.blocks.highest_finalized().unwrap_or(1) as f64;

        Plot::new("finalized_blocks")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(1024.0)
            .include_y(max_finalized - 10.0)
            .include_y(max_finalized + 5.0)
            .label_formatter(|_name, value| {
                format!("validator={} finalized={:.0}", value.x as u32, value.y)
            })
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

    fn render_particle_trails(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        // Title
        ui.label(
            egui::RichText::new("Event Particles")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let data = self.data.borrow();
        let now = self.current_time();
        let max_age = 10.0; // particles visible for 10 seconds
        let cutoff = now - max_age;

        Plot::new("particle_trails")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(0.0)
            .include_y(max_age)
            .label_formatter(|_name, value| {
                format!("node={} age={:.1}s", value.x as u32, value.y)
            })
            .show(ui, |plot_ui| {
                // Collect all points with their age for alpha calculation
                // Group by age buckets for efficiency (10 alpha levels)
                for alpha_bucket in 0..10 {
                    let bucket_min_age = alpha_bucket as f64;
                    let bucket_max_age = (alpha_bucket + 1) as f64;
                    // Fade from 255 (newest) to ~25 (oldest)
                    let alpha = (255.0 * (1.0 - alpha_bucket as f64 / 10.0)).max(25.0) as u8;

                    let mut bucket_points: Vec<[f64; 2]> = Vec::new();

                    for (_, node) in data.events.nodes() {
                        for stored in &node.events {
                            if stored.timestamp < cutoff {
                                continue;
                            }

                            let et = stored.event_type() as usize;
                            if et >= self.selected_events.len() || !self.selected_events[et] {
                                continue;
                            }

                            let age = now - stored.timestamp;
                            if age >= bucket_min_age && age < bucket_max_age {
                                bucket_points.push([node.index as f64, age]);
                            }
                        }
                    }

                    if bucket_points.is_empty() {
                        continue;
                    }

                    // White color with fading alpha
                    let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                    plot_ui.points(
                        Points::new(PlotPoints::from(bucket_points))
                            .color(color)
                            .radius(2.0)
                            .filled(true),
                    );
                }
            });
    }

    fn render_event_rates(&self, ui: &mut egui::Ui) {
        use egui_plot::{Line, Plot, PlotPoints};

        // Title
        ui.label(
            egui::RichText::new("Event Rate (per node)")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let data = self.data.borrow();
        let now = self.current_time();

        // Compute rates per node for selected event types
        let rates = data.events.compute_rates_per_node(
            now,
            1.0,                   // 1 second buckets
            60,                    // 60 buckets (~1 min history)
            &self.selected_events, // Filter by selected event types
        );

        Plot::new("event_rates")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(60.0)
            .include_y(0.0)
            .include_y(50.0) // Fixed Y range: 0-50 events/sec
            .label_formatter(|_name, value| {
                format!("t=-{:.0}s rate={:.0}/s", 60.0 - value.x, value.y)
            })
            .show(ui, |plot_ui| {
                // Dynamic alpha: ~1/num_nodes so lines sum up where there's density
                let num_nodes = rates.len().max(1);
                let alpha = (255.0 / num_nodes as f32).max(10.0).min(200.0) as u8;

                // One line per node
                for (_node_idx, node_rates) in rates.iter() {
                    if node_rates.len() < 2 {
                        continue;
                    }

                    let line_points: Vec<[f64; 2]> = node_rates
                        .iter()
                        .enumerate()
                        .map(|(x, &count)| [x as f64, count as f64])
                        .collect();

                    let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                    plot_ui.line(Line::new(PlotPoints::from(line_points)).color(color).width(1.0));
                }
            });
    }

    fn render_event_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new("All").color(colors::TEXT_SECONDARY).size(11.0))
                .clicked()
            {
                self.selected_events.fill(true);
            }
            if ui
                .button(egui::RichText::new("None").color(colors::TEXT_SECONDARY).size(11.0))
                .clicked()
            {
                self.selected_events.fill(false);
            }
            ui.add_space(10.0);

            // Category quick-toggles as compact chips
            for category in EVENT_CATEGORIES {
                let all_selected = category
                    .event_types
                    .iter()
                    .all(|&et| self.selected_events[et as usize]);
                let mut cat_checked = all_selected;

                // Use custom checkbox with explicit text color
                let text_color = if all_selected {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_SECONDARY
                };

                if ui
                    .checkbox(
                        &mut cat_checked,
                        egui::RichText::new(category.name).color(text_color).size(11.0),
                    )
                    .on_hover_text(format!("{} events", category.event_types.len()))
                    .changed()
                {
                    for &et in category.event_types {
                        self.selected_events[et as usize] = cat_checked;
                    }
                }
            }
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
