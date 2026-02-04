//! Shared JAM Visualization App
//!
//! This module contains the egui app that runs on both native and WASM platforms.

use eframe::egui;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use crate::core::{parse_event, BestBlockData, EventStore, TimeSeriesData, EVENT_CATEGORIES};
use crate::theme::{colors, minimal_visuals};
use crate::time::now_seconds;
use crate::ws_state::WsState;

#[cfg(target_arch = "wasm32")]
use crate::websocket_wasm::WsClient;

#[cfg(not(target_arch = "wasm32"))]
use crate::websocket_native::NativeWsClient;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};

/// Default WebSocket URL for jamtart
pub const DEFAULT_WS_URL: &str = "ws://127.0.0.1:8080/api/ws";

/// Shared state that can be updated from WebSocket callbacks
pub struct SharedData {
    pub time_series: TimeSeriesData,
    pub blocks: BestBlockData,
    pub events: EventStore,
}

/// JAM Visualization App - runs on both native and WASM
pub struct JamApp {
    /// Shared data (platform-specific wrapper)
    #[cfg(target_arch = "wasm32")]
    data: Rc<RefCell<SharedData>>,
    #[cfg(not(target_arch = "wasm32"))]
    data: SharedData,

    /// WebSocket state (platform-specific wrapper)
    #[cfg(target_arch = "wasm32")]
    ws_state: Rc<RefCell<WsState>>,
    #[cfg(not(target_arch = "wasm32"))]
    ws_state: Arc<Mutex<WsState>>,

    /// WebSocket client (kept alive)
    #[cfg(target_arch = "wasm32")]
    #[allow(dead_code)]
    ws_client: Option<WsClient>,
    #[cfg(not(target_arch = "wasm32"))]
    ws_client: Option<NativeWsClient>,

    /// FPS counter
    fps_counter: FpsCounter,
    /// Event filter: [event_type] = enabled
    selected_events: Vec<bool>,
    /// Toggle event selector panel visibility
    show_event_selector: bool,
}

impl JamApp {
    /// Create new app for WASM platform
    #[cfg(target_arch = "wasm32")]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());

        let data = Rc::new(RefCell::new(SharedData {
            time_series: TimeSeriesData::new(1024, 200),
            blocks: BestBlockData::new(1024),
            events: EventStore::new(50000, 60.0),
        }));

        let ws_state = Rc::new(RefCell::new(WsState::Connecting));

        // Connect WebSocket with callback that updates shared data
        let data_clone = data.clone();
        let ws_client = WsClient::connect(
            DEFAULT_WS_URL,
            move |msg| {
                let now = now_seconds();
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

        Self {
            data,
            ws_state,
            ws_client,
            fps_counter: FpsCounter::new(),
            selected_events: Self::default_selected_events(),
            show_event_selector: false,
        }
    }

    /// Create new app for native platform
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());

        let data = SharedData {
            time_series: TimeSeriesData::new(1024, 200),
            blocks: BestBlockData::new(1024),
            events: EventStore::new(50000, 60.0),
        };

        let ws_client = NativeWsClient::connect(DEFAULT_WS_URL);
        let ws_state = ws_client.state.clone();

        Self {
            data,
            ws_state,
            ws_client: Some(ws_client),
            fps_counter: FpsCounter::new(),
            selected_events: Self::default_selected_events(),
            show_event_selector: false,
        }
    }

    fn default_selected_events() -> Vec<bool> {
        let mut selected = vec![false; 200];
        // Enable Status events by default for visibility
        for &et in &[10, 11, 12, 13] {
            selected[et] = true;
        }
        selected
    }

    /// Process incoming WebSocket messages (native only)
    #[cfg(not(target_arch = "wasm32"))]
    fn process_messages(&mut self) {
        if let Some(ref client) = self.ws_client {
            while let Ok(msg) = client.rx.try_recv() {
                let now = now_seconds();
                parse_event(
                    &msg,
                    &mut self.data.time_series,
                    &mut self.data.blocks,
                    &mut self.data.events,
                    now,
                );
            }
        }
    }

    /// Get the current WebSocket state
    fn get_ws_state(&self) -> WsState {
        #[cfg(target_arch = "wasm32")]
        {
            self.ws_state.borrow().clone()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.ws_state.lock().unwrap().clone()
        }
    }
}

impl eframe::App for JamApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaint for real-time updates
        ctx.request_repaint();

        // Process WebSocket messages (native only - WASM uses callbacks)
        #[cfg(not(target_arch = "wasm32"))]
        self.process_messages();

        // Prune old events periodically
        let now = now_seconds();
        #[cfg(target_arch = "wasm32")]
        self.data.borrow_mut().events.prune(now);
        #[cfg(not(target_arch = "wasm32"))]
        self.data.events.prune(now);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY))
            .show(ctx, |ui| {
                self.render_header(ui);

                if self.show_event_selector {
                    ui.add_space(4.0);
                    self.render_event_selector(ui);
                }

                ui.add_space(8.0);

                let available = ui.available_size();
                let graph_height = (available.y - 40.0) / 5.0;

                // Time series (num_peers)
                ui.allocate_ui(egui::vec2(available.x, graph_height), |ui| {
                    self.render_time_series(ui);
                });

                ui.add_space(4.0);

                // Particle trails
                ui.allocate_ui(egui::vec2(available.x, graph_height), |ui| {
                    self.render_particle_trails(ui);
                });

                ui.add_space(4.0);

                // Event rate lines
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

// Helper macro to access data on both platforms
macro_rules! with_data {
    ($self:expr, |$data:ident| $body:expr) => {{
        #[cfg(target_arch = "wasm32")]
        {
            let $data = $self.data.borrow();
            $body
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let $data = &$self.data;
            $body
        }
    }};
}

impl JamApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        self.fps_counter.tick();

        let ws_state = self.get_ws_state();

        let (validator_count, highest_slot, event_count) = with_data!(self, |data| {
            (
                data.time_series.validator_count(),
                data.blocks.highest_slot(),
                data.events.node_count(),
            )
        });

        ui.horizontal(|ui| {
            // Connection status indicator
            let (status_color, status_text) = match &ws_state {
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

            ui.label(
                egui::RichText::new(format!("{} validators", validator_count))
                    .color(colors::TEXT_MUTED)
                    .monospace()
                    .size(11.0),
            );

            if let Some(slot) = highest_slot {
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

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("JAM")
                        .color(colors::TEXT_PRIMARY)
                        .size(12.0),
                );

                ui.add_space(10.0);

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

        ui.label(
            egui::RichText::new("Peer Count")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let (point_count, y_min, y_max, series_data) = with_data!(self, |data| {
            let point_count = data.time_series.point_count();
            let (y_min, y_max) = data
                .time_series
                .series
                .iter()
                .flat_map(|s| s.iter())
                .fold((f32::MAX, f32::MIN), |(min, max), &v| {
                    (min.min(v), max.max(v))
                });

            let series_data: Vec<Vec<f32>> =
                data.time_series.series.iter().map(|s| s.clone()).collect();

            (point_count, y_min, y_max, series_data)
        });

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
                for series in &series_data {
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

        ui.label(
            egui::RichText::new("Best Block")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let (max_block, points_data) = with_data!(self, |data| {
            let max_block = data.blocks.highest_slot().unwrap_or(1) as f64;
            let points_data: Vec<[f64; 2]> = data
                .blocks
                .best_blocks
                .iter()
                .enumerate()
                .filter(|(_, &slot)| slot > 0)
                .map(|(id, &slot)| [id as f64, slot as f64])
                .collect();
            (max_block, points_data)
        });

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
                plot_ui.points(
                    Points::new(PlotPoints::from(points_data))
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180))
                        .radius(2.0)
                        .filled(true),
                );
            });
    }

    fn render_finalized_blocks(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        ui.label(
            egui::RichText::new("Finalized Block")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let (max_finalized, points_data) = with_data!(self, |data| {
            let max_finalized = data.blocks.highest_finalized().unwrap_or(1) as f64;
            let points_data: Vec<[f64; 2]> = data
                .blocks
                .finalized_blocks
                .iter()
                .enumerate()
                .filter(|(_, &slot)| slot > 0)
                .map(|(id, &slot)| [id as f64, slot as f64])
                .collect();
            (max_finalized, points_data)
        });

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
                plot_ui.points(
                    Points::new(PlotPoints::from(points_data))
                        .color(egui::Color32::from_rgba_unmultiplied(150, 150, 150, 180))
                        .radius(2.0)
                        .filled(true),
                );
            });
    }

    fn render_particle_trails(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        ui.label(
            egui::RichText::new("Event Particles")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let now = now_seconds();
        let max_age = 10.0;
        let cutoff = now - max_age;

        // Collect points grouped by age bucket
        let bucket_points: Vec<(u8, Vec<[f64; 2]>)> = with_data!(self, |data| {
            let mut buckets: Vec<(u8, Vec<[f64; 2]>)> = Vec::new();

            for alpha_bucket in 0..10 {
                let bucket_min_age = alpha_bucket as f64;
                let bucket_max_age = (alpha_bucket + 1) as f64;
                let alpha = (255.0 * (1.0 - alpha_bucket as f64 / 10.0)).max(25.0) as u8;

                let mut points: Vec<[f64; 2]> = Vec::new();

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
                            points.push([node.index as f64, age]);
                        }
                    }
                }

                if !points.is_empty() {
                    buckets.push((alpha, points));
                }
            }

            buckets
        });

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
                for (alpha, points) in bucket_points {
                    let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                    plot_ui.points(
                        Points::new(PlotPoints::from(points))
                            .color(color)
                            .radius(2.0)
                            .filled(true),
                    );
                }
            });
    }

    fn render_event_rates(&self, ui: &mut egui::Ui) {
        use egui_plot::{Line, Plot, PlotPoints};

        ui.label(
            egui::RichText::new("Event Rate (per node)")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let now = now_seconds();

        let rates: Vec<(u16, Vec<u32>)> = with_data!(self, |data| {
            data.events
                .compute_rates_per_node(now, 1.0, 60, &self.selected_events)
        });

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
            .include_y(50.0)
            .label_formatter(|_name, value| {
                format!("t=-{:.0}s rate={:.0}/s", 60.0 - value.x, value.y)
            })
            .show(ui, |plot_ui| {
                let num_nodes = rates.len().max(1);
                let alpha = (255.0_f32 / num_nodes as f32).max(10.0).min(200.0) as u8;

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
                .button(
                    egui::RichText::new("All")
                        .color(colors::TEXT_SECONDARY)
                        .size(11.0),
                )
                .clicked()
            {
                self.selected_events.fill(true);
            }
            if ui
                .button(
                    egui::RichText::new("None")
                        .color(colors::TEXT_SECONDARY)
                        .size(11.0),
                )
                .clicked()
            {
                self.selected_events.fill(false);
            }
            ui.add_space(10.0);

            for category in EVENT_CATEGORIES {
                let all_selected = category
                    .event_types
                    .iter()
                    .all(|&et| self.selected_events[et as usize]);
                let mut cat_checked = all_selected;

                let text_color = if all_selected {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_SECONDARY
                };

                if ui
                    .checkbox(
                        &mut cat_checked,
                        egui::RichText::new(category.name)
                            .color(text_color)
                            .size(11.0),
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

/// FPS counter using platform-agnostic time
pub struct FpsCounter {
    frames: Vec<f64>,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            frames: Vec::with_capacity(60),
        }
    }

    pub fn tick(&mut self) {
        let now = now_seconds() * 1000.0; // Convert to ms for compatibility
        self.frames.push(now);
        if self.frames.len() > 60 {
            self.frames.remove(0);
        }
    }

    pub fn fps(&self) -> f64 {
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

impl Default for FpsCounter {
    fn default() -> Self {
        Self::new()
    }
}
