//! Availability & Consensus panel
//!
//! Visualizes block production, finalization gap, assurances, shard storage, and forks.

use std::collections::HashSet;
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Points};
use crate::theme::colors;
use crate::time::now_seconds;
use super::{JamApp, with_data};

impl JamApp {
    pub(crate) fn render_consensus_tab(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let row_height = (available.y - 20.0) / 2.0;

        // Row 1: Block Height Gap + Assurances Rate
        ui.horizontal(|ui| {
            let col_width = (available.x - 10.0) / 2.0;

            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_block_height_gap(ui);
            });
            ui.add_space(10.0);
            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_assurances_rate(ui);
            });
        });

        ui.add_space(4.0);

        // Row 2: Shard Storage + Fork Detection
        ui.horizontal(|ui| {
            let col_width = (available.x - 10.0) / 2.0;

            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_shard_storage(ui);
            });
            ui.add_space(10.0);
            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_fork_detection(ui);
            });
        });

        if self.show_legend {
            let panel_rect = ui.min_rect();
            self.draw_legend(ui.painter(), panel_rect);
        }
    }

    /// Block Height Gap scatter: best_block - finalized_block per validator
    fn render_block_height_gap(&self, ui: &mut egui::Ui) {
        let gap_points: Vec<[f64; 2]> = with_data!(self, |data| {
            data.blocks
                .best_blocks
                .iter()
                .zip(data.blocks.finalized_blocks.iter())
                .enumerate()
                .filter(|(_, (&best, &fin))| best > 0 && fin > 0)
                .map(|(i, (&best, &fin))| {
                    let gap = if best >= fin { best - fin } else { 0 };
                    [i as f64, gap as f64]
                })
                .collect()
        });

        if gap_points.is_empty() {
            Self::paint_plot_title(ui, ui.max_rect(), "Block Height Gap — no data", colors::TEXT_MUTED);
            return;
        }

        let max_gap = gap_points.iter().map(|p| p[1]).fold(0.0_f64, f64::max);

        let resp = Plot::new("block_height_gap")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(0.0)
            .include_y(max_gap.max(3.0) + 1.0)
            .y_axis_formatter(|mark, _| {
                if (mark.value - mark.value.round()).abs() < 0.01 {
                    format!("{:.0}", mark.value)
                } else {
                    String::new()
                }
            })
            .label_formatter(|_name, value| {
                format!("validator={} gap={:.0} slots", value.x as u32, value.y)
            })
            .show(ui, |plot_ui| {
                plot_ui.points(
                    Points::new(PlotPoints::from(gap_points))
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 150))
                        .radius(2.0)
                        .filled(true),
                );
            });

        Self::paint_plot_title(ui, resp.response.rect, "Block Height Gap (best - finalized)", colors::TEXT_MUTED);
    }

    /// Assurances rate (sent + received)
    fn render_assurances_rate(&self, ui: &mut egui::Ui) {
        let now = now_seconds();

        let sent_rates: Vec<f64> = with_data!(self, |data| {
            data.events.compute_aggregate_rate(&[128], now, 1.0, 60)
        });
        let recv_rates: Vec<f64> = with_data!(self, |data| {
            data.events.compute_aggregate_rate(&[131], now, 1.0, 60)
        });

        let max_y = sent_rates.iter().chain(recv_rates.iter())
            .copied().fold(0.0_f64, f64::max);

        let resp = Plot::new("assurances_rate")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(60.0)
            .include_y(0.0)
            .include_y((max_y * 1.1).max(2.0))
            .label_formatter(|_name, value| {
                format!("t=-{:.0}s rate={:.1}/s", 60.0 - value.x, value.y)
            })
            .show(ui, |plot_ui| {
                if !sent_rates.is_empty() {
                    let points: Vec<[f64; 2]> = sent_rates
                        .iter()
                        .enumerate()
                        .map(|(x, &y)| [x as f64, y])
                        .collect();
                    plot_ui.line(
                        Line::new(PlotPoints::from(points))
                            .color(egui::Color32::from_rgb(255, 255, 100))
                            .width(2.0)
                            .name("Sent"),
                    );
                }
                if !recv_rates.is_empty() {
                    let points: Vec<[f64; 2]> = recv_rates
                        .iter()
                        .enumerate()
                        .map(|(x, &y)| [x as f64, y])
                        .collect();
                    plot_ui.line(
                        Line::new(PlotPoints::from(points))
                            .color(egui::Color32::from_rgb(200, 200, 50))
                            .width(2.0)
                            .name("Received"),
                    );
                }
            });

        Self::paint_plot_title(ui, resp.response.rect, "Assurances (sent/received)", colors::TEXT_MUTED);
    }

    /// Shard storage metrics from Status events
    fn render_shard_storage(&self, ui: &mut egui::Ui) {
        let (count_series, size_series, point_count) = with_data!(self, |data| {
            let counts: Vec<Vec<f32>> = data.shard_metrics.shard_counts.series.iter().map(|s| s.clone()).collect();
            let sizes: Vec<Vec<f32>> = data.shard_metrics.shard_sizes.series.iter().map(|s| s.clone()).collect();
            let pc = data.shard_metrics.shard_counts.point_count();
            (counts, sizes, pc)
        });

        let half_height = (ui.available_height() - 4.0) / 2.0;
        let width = ui.available_width();

        // Shard count plot
        ui.allocate_ui(egui::vec2(width, half_height), |ui| {
            let resp = Plot::new("shard_counts")
                .show_axes([false, true])
                .show_grid(false)
                .allow_zoom(false)
                .allow_drag(false)
                .allow_scroll(false)
                .show_background(false)
                .include_x(0.0)
                .include_x(point_count.max(1) as f64)
                .include_y(0.0)
                .clamp_grid(true)
                .show(ui, |plot_ui| {
                    let num_nodes = count_series.len().max(1);
                    let alpha = (255.0_f32 / num_nodes as f32).max(10.0).min(200.0) as u8;

                    for series in &count_series {
                        if series.len() < 2 { continue; }
                        let points: PlotPoints = series
                            .iter()
                            .enumerate()
                            .map(|(x, &y)| [x as f64, y as f64])
                            .collect();
                        let color = egui::Color32::from_rgba_unmultiplied(255, 255, 100, alpha);
                        plot_ui.line(Line::new(points).color(color).width(1.0));
                    }
                });

            Self::paint_plot_title(ui, resp.response.rect, "Shard Storage: num_shards", colors::TEXT_MUTED);
        });

        ui.add_space(4.0);

        // Shard size plot
        ui.allocate_ui(egui::vec2(width, half_height), |ui| {
            let resp = Plot::new("shard_sizes")
                .show_axes([false, true])
                .show_grid(false)
                .allow_zoom(false)
                .allow_drag(false)
                .allow_scroll(false)
                .show_background(false)
                .include_x(0.0)
                .include_x(point_count.max(1) as f64)
                .include_y(0.0)
                .clamp_grid(true)
                .show(ui, |plot_ui| {
                    let num_nodes = size_series.len().max(1);
                    let alpha = (255.0_f32 / num_nodes as f32).max(10.0).min(200.0) as u8;

                    for series in &size_series {
                        if series.len() < 2 { continue; }
                        let points: PlotPoints = series
                            .iter()
                            .enumerate()
                            .map(|(x, &y)| [x as f64, y as f64])
                            .collect();
                        let color = egui::Color32::from_rgba_unmultiplied(255, 200, 100, alpha);
                        plot_ui.line(Line::new(points).color(color).width(1.0));
                    }
                });

            Self::paint_plot_title(ui, resp.response.rect, "Shard Storage: shards_size", colors::TEXT_MUTED);
        });
    }

    /// Fork detection: check for divergent best block slots
    fn render_fork_detection(&self, ui: &mut egui::Ui) {
        let (distinct_slots, min_slot, max_slot, best_points) = with_data!(self, |data| {
            let non_zero: Vec<u64> = data.blocks.best_blocks.iter().copied().filter(|&s| s > 0).collect();
            let unique: HashSet<u64> = non_zero.iter().copied().collect();
            let min = non_zero.iter().copied().min().unwrap_or(0);
            let max = non_zero.iter().copied().max().unwrap_or(0);

            let points: Vec<[f64; 2]> = data.blocks.best_blocks
                .iter()
                .enumerate()
                .filter(|(_, &slot)| slot > 0)
                .map(|(id, &slot)| [id as f64, slot as f64])
                .collect();

            (unique.len(), min, max, points)
        });

        let spread = if max_slot > min_slot { max_slot - min_slot } else { 0 };
        let is_fork = spread > 2 && distinct_slots > 1;

        if best_points.is_empty() {
            let title = if is_fork {
                format!("FORK DETECTED: {} distinct slots ({}..{})", distinct_slots, min_slot, max_slot)
            } else {
                format!("No forks ({} distinct slots, spread={})", distinct_slots, spread)
            };
            Self::paint_plot_title(ui, ui.max_rect(), &title, colors::TEXT_MUTED);
            return;
        }

        let max_block = max_slot as f64;

        let resp = Plot::new("best_blocks_fork")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(max_block - 10.0)
            .include_y(max_block + 5.0)
            .y_axis_formatter(|mark, _| {
                if (mark.value - mark.value.round()).abs() < 0.01 {
                    format!("{:.0}", mark.value)
                } else {
                    String::new()
                }
            })
            .label_formatter(|_name, value| {
                format!("validator={} slot={:.0}", value.x as u32, value.y)
            })
            .show(ui, |plot_ui| {
                plot_ui.points(
                    Points::new(PlotPoints::from(best_points))
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180))
                        .radius(2.0)
                        .filled(true),
                );
            });

        let (title, color) = if is_fork {
            (format!("FORK: {} slots ({}..{})", distinct_slots, min_slot, max_slot),
             egui::Color32::from_rgb(255, 100, 100))
        } else {
            (format!("No forks ({} slots, spread={})", distinct_slots, spread),
             colors::TEXT_MUTED)
        };
        Self::paint_plot_title(ui, resp.response.rect, &title, color);
    }
}
