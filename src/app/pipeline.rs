//! Work Report Pipeline panel
//!
//! Visualizes the work package lifecycle: Submission → Refined → Guaranteed
//! Plus queue per core, PVM costs, and discard reasons.

use eframe::egui;
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints};
use crate::theme::colors;
use crate::time::now_seconds;
use super::{JamApp, with_data};

impl JamApp {
    pub(crate) fn render_pipeline_tab(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let row_height = (available.y - 20.0) / 2.0;

        // Row 1: Three rate graphs
        ui.horizontal(|ui| {
            let col_width = (available.x - 20.0) / 3.0;

            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_rate_plot(ui, "submission_rate", "Submission Rate (90)", &[90]);
            });
            ui.add_space(10.0);
            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_rate_plot(ui, "refined_rate", "Refined Rate (101)", &[101]);
            });
            ui.add_space(10.0);
            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_rate_plot(ui, "guaranteed_rate", "Guaranteed Rate (105)", &[105]);
            });
        });

        ui.add_space(4.0);

        // Row 2: Queue per Core, PVM Costs, Discard Reasons
        ui.horizontal(|ui| {
            let col_width = (available.x - 20.0) / 3.0;

            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_queue_per_core(ui);
            });
            ui.add_space(10.0);
            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_pvm_costs(ui);
            });
            ui.add_space(10.0);
            ui.allocate_ui(egui::vec2(col_width, row_height), |ui| {
                self.render_discard_reasons(ui);
            });
        });

        if self.show_legend {
            let panel_rect = ui.min_rect();
            self.draw_legend(ui.painter(), panel_rect);
        }
    }

    fn render_queue_per_core(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Queue per Core")
                .color(colors::TEXT_MUTED)
                .size(14.0),
        );

        let queue_data: Vec<u32> = with_data!(self, |data| {
            data.guarantee_queue.aggregate_per_core()
        });

        if queue_data.is_empty() {
            ui.label(
                egui::RichText::new("No data yet")
                    .color(colors::TEXT_MUTED)
                    .size(12.0),
            );
            return;
        }

        let bars: Vec<Bar> = queue_data
            .iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .map(|(core_idx, &count)| {
                Bar::new(core_idx as f64, count as f64)
                    .width(0.8)
                    .fill(egui::Color32::from_rgba_unmultiplied(100, 255, 200, 150))
            })
            .collect();

        let max_y = queue_data.iter().copied().max().unwrap_or(1) as f64;

        Plot::new("queue_per_core")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(0.0)
            .include_y(max_y + 1.0)
            .label_formatter(|_name, value| {
                format!("core={} queue={:.0}", value.x as u32, value.y)
            })
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new(bars));
            });
    }

    fn render_pvm_costs(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Block Execution Rate (47)")
                .color(colors::TEXT_MUTED)
                .size(14.0),
        );

        let now = now_seconds();
        let rates: Vec<f64> = with_data!(self, |data| {
            data.events.compute_aggregate_rate(&[47], now, 1.0, 60)
        });

        if rates.iter().all(|&r| r == 0.0) {
            ui.label(
                egui::RichText::new("No data yet")
                    .color(colors::TEXT_MUTED)
                    .size(12.0),
            );
            return;
        }

        Plot::new("pvm_costs")
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
                let line_points: Vec<[f64; 2]> = rates
                    .iter()
                    .enumerate()
                    .map(|(x, &y)| [x as f64, y])
                    .collect();
                let color = egui::Color32::from_rgb(255, 200, 100);
                plot_ui.line(Line::new(PlotPoints::from(line_points)).color(color).width(2.0));
            });
    }

    fn render_discard_reasons(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Discard Reasons (113)")
                .color(colors::TEXT_MUTED)
                .size(14.0),
        );

        let now = now_seconds();
        let distribution = with_data!(self, |data| {
            data.events.discard_reason_distribution(now, 60.0)
        });

        if distribution.is_empty() {
            ui.label(
                egui::RichText::new("No discards")
                    .color(colors::TEXT_MUTED)
                    .size(12.0),
            );
            return;
        }

        let reason_names = [
            "OnChain",
            "Replaced",
            "CannotReport",
            "TooMany",
            "Other",
        ];

        let bars: Vec<Bar> = distribution
            .iter()
            .enumerate()
            .filter(|(_, &(_, count))| count > 0)
            .map(|(i, &(ref _reason, count))| {
                Bar::new(i as f64, count as f64)
                    .width(0.8)
                    .fill(egui::Color32::from_rgba_unmultiplied(255, 100, 100, 180))
            })
            .collect();

        let max_y = distribution.iter().map(|(_, c)| *c).max().unwrap_or(1) as f64;

        Plot::new("discard_reasons")
            .show_axes([true, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(0.0)
            .include_y(max_y + 1.0)
            .label_formatter(move |_name, value| {
                let idx = value.x.round() as usize;
                let name = reason_names.get(idx).unwrap_or(&"?");
                format!("{}: {:.0}", name, value.y)
            })
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new(bars));
            });
    }
}
