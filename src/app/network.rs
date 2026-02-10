//! Network Health panel
//!
//! Visualizes peer connections, sync status, and network events.

use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Points};
use crate::theme::colors;
use crate::time::now_seconds;
use super::{JamApp, with_data};

impl JamApp {
    pub(crate) fn render_network_tab(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let row_height = (available.y - 20.0) / 2.0;

        // Row 1: Peer Count + Connection Events
        ui.horizontal(|ui| {
            let col_width = (available.x - 10.0) / 2.0;

            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_peer_count(ui);
            });
            ui.add_space(10.0);
            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_connection_events(ui);
            });
        });

        ui.add_space(4.0);

        // Row 2: Sync Status + Misbehavior
        ui.horizontal(|ui| {
            let col_width = (available.x - 10.0) / 2.0;

            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_sync_status(ui);
            });
            ui.add_space(10.0);
            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_misbehavior(ui);
            });
        });

        if self.show_legend {
            let panel_rect = ui.min_rect();
            self.draw_legend(ui.painter(), panel_rect);
        }
    }

    /// Peer Count time series — moved from original render_time_series()
    fn render_peer_count(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Peer Count")
                .color(colors::TEXT_MUTED)
                .size(14.0),
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

        Plot::new("peer_count")
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

    /// Connection events rate (connect vs disconnect)
    fn render_connection_events(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Connection Events")
                .color(colors::TEXT_MUTED)
                .size(14.0),
        );

        let now = now_seconds();

        // Connected events: ConnectedIn(23) + ConnectedOut(26)
        let connect_rates: Vec<f64> = with_data!(self, |data| {
            data.events.compute_aggregate_rate(&[23, 26], now, 1.0, 60)
        });

        // Disconnect events: Disconnected(27)
        let disconnect_rates: Vec<f64> = with_data!(self, |data| {
            data.events.compute_aggregate_rate(&[27], now, 1.0, 60)
        });

        Plot::new("connection_events")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(60.0)
            .include_y(0.0)
            .label_formatter(|_name, value| {
                format!("t=-{:.0}s rate={:.1}/s", 60.0 - value.x, value.y)
            })
            .show(ui, |plot_ui| {
                // Connect rate (green)
                if !connect_rates.is_empty() {
                    let points: Vec<[f64; 2]> = connect_rates
                        .iter()
                        .enumerate()
                        .map(|(x, &y)| [x as f64, y])
                        .collect();
                    plot_ui.line(
                        Line::new(PlotPoints::from(points))
                            .color(egui::Color32::from_rgb(100, 200, 100))
                            .width(2.0)
                            .name("Connected"),
                    );
                }

                // Disconnect rate (red)
                if !disconnect_rates.is_empty() {
                    let points: Vec<[f64; 2]> = disconnect_rates
                        .iter()
                        .enumerate()
                        .map(|(x, &y)| [x as f64, y])
                        .collect();
                    plot_ui.line(
                        Line::new(PlotPoints::from(points))
                            .color(egui::Color32::from_rgb(255, 100, 100))
                            .width(2.0)
                            .name("Disconnected"),
                    );
                }
            });
    }

    /// Sync status dots
    fn render_sync_status(&self, ui: &mut egui::Ui) {
        let (synced, total) = with_data!(self, |data| {
            (data.sync_status.synced_count(), data.sync_status.total_count())
        });

        ui.label(
            egui::RichText::new(format!("Sync Status ({}/{} synced)", synced, total))
                .color(colors::TEXT_MUTED)
                .size(14.0),
        );

        let status_data: Vec<(bool, f64)> = with_data!(self, |data| {
            data.sync_status.status.clone()
        });

        let mut synced_points: Vec<[f64; 2]> = Vec::new();
        let mut unsynced_points: Vec<[f64; 2]> = Vec::new();

        for (i, &(is_synced, _last_update)) in status_data.iter().enumerate() {
            if is_synced {
                synced_points.push([i as f64, 1.0]);
            } else {
                unsynced_points.push([i as f64, 0.0]);
            }
        }

        Plot::new("sync_status")
            .show_axes([false, false])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(-0.5)
            .include_y(1.5)
            .label_formatter(|_name, value| {
                let status = if value.y > 0.5 { "synced" } else { "not synced" };
                format!("validator={} {}", value.x as u32, status)
            })
            .show(ui, |plot_ui| {
                if !synced_points.is_empty() {
                    plot_ui.points(
                        Points::new(PlotPoints::from(synced_points))
                            .color(egui::Color32::from_rgb(100, 200, 100))
                            .radius(2.0)
                            .filled(true),
                    );
                }
                if !unsynced_points.is_empty() {
                    plot_ui.points(
                        Points::new(PlotPoints::from(unsynced_points))
                            .color(egui::Color32::from_rgb(255, 100, 100))
                            .radius(2.0)
                            .filled(true),
                    );
                }
            });
    }

    /// Misbehavior counter + sparkline
    fn render_misbehavior(&self, ui: &mut egui::Ui) {
        let now = now_seconds();

        let count: u64 = with_data!(self, |data| {
            data.events.count_events(&[28], now, 60.0)
        });

        ui.label(
            egui::RichText::new(format!("PeerMisbehaved: {} (60s)", count))
                .color(if count > 0 {
                    egui::Color32::from_rgb(255, 100, 100)
                } else {
                    colors::TEXT_MUTED
                })
                .size(14.0),
        );

        // Sparkline
        let rates: Vec<f64> = with_data!(self, |data| {
            data.events.compute_aggregate_rate(&[28], now, 1.0, 60)
        });

        Plot::new("misbehavior_rate")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(60.0)
            .include_y(0.0)
            .label_formatter(|_name, value| {
                format!("t=-{:.0}s count={:.0}", 60.0 - value.x, value.y)
            })
            .show(ui, |plot_ui| {
                let points: Vec<[f64; 2]> = rates
                    .iter()
                    .enumerate()
                    .map(|(x, &y)| [x as f64, y])
                    .collect();
                plot_ui.line(
                    Line::new(PlotPoints::from(points))
                        .color(egui::Color32::from_rgb(255, 100, 100))
                        .width(2.0),
                );
            });
    }
}
