//! JAM Visualization PoC - Multiple graphs

use eframe::egui;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod data;
mod theme;

use data::{BestBlockData, EventHistogramData, TimeSeriesData};
use theme::{colors, minimal_visuals};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();

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

struct JamVisApp {
    // Data sources
    time_series: TimeSeriesData,
    best_blocks: BestBlockData,
    event_histogram: EventHistogramData,

    // Timing
    last_time_series_tick: f64,
    last_histogram_tick: f64,
    fps_counter: FpsCounter,

    // UI state
    paused: bool,
}

impl JamVisApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());

        let mut time_series = TimeSeriesData::new(1024, 200);  // 1024 validators
        for _ in 0..50 {
            time_series.tick();
        }

        Self {
            time_series,
            best_blocks: BestBlockData::new(1024),
            event_histogram: EventHistogramData::new(1024),
            last_time_series_tick: 0.0,
            last_histogram_tick: 0.0,
            fps_counter: FpsCounter::new(),
            paused: false,
        }
    }
}

impl eframe::App for JamVisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);

        if !self.paused {
            // Time series: 60 FPS
            if now - self.last_time_series_tick > 1.0 / 60.0 {
                self.time_series.tick();
                self.last_time_series_tick = now;
            }

            // Best blocks: continuous check (internal timing)
            self.best_blocks.tick(now);

            // Histogram: ~5 times per second
            if now - self.last_histogram_tick > 0.2 {
                self.event_histogram.tick();
                self.last_histogram_tick = now;
            }
        }

        ctx.request_repaint();

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY))
            .show(ctx, |ui| {
                self.render_header(ui);

                // Use a grid layout for multiple plots
                ui.add_space(8.0);

                let available = ui.available_size();
                let plot_height = (available.y - 20.0) / 2.0;  // 2 rows

                // Top row: Time series (full width)
                ui.allocate_ui(egui::vec2(available.x, plot_height), |ui| {
                    self.render_time_series(ui);
                });

                ui.add_space(10.0);

                // Bottom row: Best blocks and histogram side by side
                ui.horizontal(|ui| {
                    let half_width = (available.x - 10.0) / 2.0;

                    ui.allocate_ui(egui::vec2(half_width, plot_height - 20.0), |ui| {
                        self.render_best_blocks(ui);
                    });

                    ui.add_space(10.0);

                    ui.allocate_ui(egui::vec2(half_width, plot_height - 20.0), |ui| {
                        self.render_histogram(ui);
                    });
                });
            });
    }
}

impl JamVisApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        self.fps_counter.tick();

        ui.horizontal(|ui| {
            // Pause button
            let pause_text = if self.paused { "▶ Play" } else { "⏸ Pause" };
            if ui.button(egui::RichText::new(pause_text).size(11.0)).clicked() {
                self.paused = !self.paused;
            }

            ui.add_space(10.0);

            ui.label(
                egui::RichText::new(format!("{:.0} fps", self.fps_counter.fps()))
                    .color(colors::TEXT_SECONDARY)
                    .monospace()
                    .size(11.0),
            );

            ui.label(egui::RichText::new("/").color(colors::TEXT_MUTED).size(11.0));

            ui.label(
                egui::RichText::new("1024 validators")
                    .color(colors::TEXT_MUTED)
                    .monospace()
                    .size(11.0),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("JAM").color(colors::TEXT_PRIMARY).size(12.0));
            });
        });
    }

    fn render_time_series(&self, ui: &mut egui::Ui) {
        use egui_plot::{Line, Plot, PlotPoints};

        let point_count = self.time_series.point_count();
        let (y_min, y_max) = self.time_series.series.iter()
            .flat_map(|s| s.iter())
            .fold((f32::MAX, f32::MIN), |(min, max), &v| (min.min(v), max.max(v)));
        let y_pad = (y_max - y_min).max(10.0) * 0.1;

        Plot::new("time_series")
            .show_axes([true, true])
            .show_grid(true)
            .allow_zoom(true)
            .allow_drag(true)
            .show_background(false)
            .include_x(0.0)
            .include_x(point_count.max(1) as f64)
            .include_y((y_min - y_pad) as f64)
            .include_y((y_max + y_pad) as f64)
            .show(ui, |plot_ui| {
                for series in &self.time_series.series {
                    if series.len() < 2 { continue; }

                    let points: PlotPoints = series.iter()
                        .enumerate()
                        .map(|(x, &y)| [x as f64, y as f64])
                        .collect();

                    let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, colors::LINE_ALPHA);
                    plot_ui.line(Line::new(points).color(color).width(1.0));
                }
            });
    }

    fn render_best_blocks(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, Points, PlotPoints};

        let max_block = self.best_blocks.blocks.iter().max().copied().unwrap_or(1) as f64;

        Plot::new("best_blocks")
            .show_axes([true, true])
            .show_grid(true)
            .allow_zoom(true)
            .allow_drag(true)
            .show_background(false)
            .include_x(0.0)
            .include_x(1024.0)
            .include_y(0.0)
            .include_y(max_block + 5.0)
            .x_axis_label("Validator ID")
            .y_axis_label("Block #")
            .show(ui, |plot_ui| {
                let points: PlotPoints = self.best_blocks.blocks.iter()
                    .enumerate()
                    .map(|(id, &block)| [id as f64, block as f64])
                    .collect();

                plot_ui.points(
                    Points::new(points)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180))
                        .radius(2.0)
                        .filled(true)
                );
            });
    }

    fn render_histogram(&self, ui: &mut egui::Ui) {
        use egui_plot::{Bar, BarChart, Plot};

        let max_events = self.event_histogram.events_a.iter()
            .chain(self.event_histogram.events_b.iter())
            .max()
            .copied()
            .unwrap_or(1) as f64;

        Plot::new("histogram")
            .show_axes([true, true])
            .show_grid(true)
            .allow_zoom(true)
            .allow_drag(true)
            .show_background(false)
            .include_x(0.0)
            .include_x(1024.0)
            .include_y(0.0)
            .include_y(max_events + 5.0)
            .x_axis_label("Validator ID")
            .y_axis_label("Events")
            .show(ui, |plot_ui| {
                // Events A - white bars
                let bars_a: Vec<Bar> = self.event_histogram.events_a.iter()
                    .enumerate()
                    .map(|(id, &count)| Bar::new(id as f64 - 0.2, count as f64).width(0.35))
                    .collect();

                plot_ui.bar_chart(
                    BarChart::new(bars_a)
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200))
                        .name("Events A")
                );

                // Events B - grey bars
                let bars_b: Vec<Bar> = self.event_histogram.events_b.iter()
                    .enumerate()
                    .map(|(id, &count)| Bar::new(id as f64 + 0.2, count as f64).width(0.35))
                    .collect();

                plot_ui.bar_chart(
                    BarChart::new(bars_b)
                        .color(egui::Color32::from_rgba_unmultiplied(128, 128, 128, 200))
                        .name("Events B")
                );
            });
    }
}

struct FpsCounter {
    frames: Vec<f64>,
}

impl FpsCounter {
    fn new() -> Self {
        Self { frames: Vec::with_capacity(60) }
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
        if self.frames.len() < 2 { return 0.0; }
        let elapsed = self.frames.last().unwrap() - self.frames.first().unwrap();
        if elapsed == 0.0 { return 0.0; }
        (self.frames.len() as f64 - 1.0) / (elapsed / 1000.0)
    }
}
