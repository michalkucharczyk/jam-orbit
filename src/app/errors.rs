//! Error Analysis panel
//!
//! Aggregated error view with rates by type and scrolling recent errors list.

use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use crate::core::event_name;
use crate::theme::colors;
use crate::time::now_seconds;
use super::{JamApp, with_data};

/// Error event type IDs
const ERROR_TYPES: &[u8] = &[
    22,  // ConnectInFailed
    25,  // ConnectOutFailed
    41,  // AuthoringFailed
    44,  // BlockVerificationFailed
    46,  // BlockExecutionFailed
    65,  // BlockRequestFailed
    81,  // TicketGenerationFailed
    92,  // WorkPackageFailed
    93,  // WorkPackageSharingFailed
    107, // GuaranteeSendFailed
    111, // GuaranteeReceiveFailed
    122, // ShardRequestFailed
    129, // AssuranceSendFailed
    130, // AssuranceReceiveFailed
];

impl JamApp {
    pub(crate) fn render_errors_tab(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();

        ui.horizontal(|ui| {
            let chart_width = available.x * 0.6;
            let list_width = available.x * 0.4 - 10.0;

            // Left: Error rate chart
            ui.allocate_ui(egui::vec2(chart_width, available.y), |ui| {
                self.render_error_rates(ui);
            });

            ui.add_space(10.0);

            // Right: Recent errors list
            ui.allocate_ui(egui::vec2(list_width, available.y), |ui| {
                self.render_recent_errors(ui);
            });
        });
    }

    /// Error rate by type (one colored line per error type)
    fn render_error_rates(&self, ui: &mut egui::Ui) {
        let now = now_seconds();

        let total_count: u64 = with_data!(self, |data| {
            data.events.count_events(ERROR_TYPES, now, 60.0)
        });

        ui.label(
            egui::RichText::new(format!("Error Rate by Type ({} errors in 60s)", total_count))
                .color(if total_count > 0 {
                    egui::Color32::from_rgb(255, 100, 100)
                } else {
                    colors::TEXT_MUTED
                })
                .size(14.0),
        );

        // Compute rate for each error type
        let rates_per_type: Vec<(u8, Vec<f64>)> = with_data!(self, |data| {
            ERROR_TYPES
                .iter()
                .map(|&et| {
                    let rates = data.events.compute_aggregate_rate(&[et], now, 1.0, 60);
                    (et, rates)
                })
                .filter(|(_, rates)| rates.iter().any(|&r| r > 0.0))
                .collect()
        });

        Plot::new("error_rates")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(60.0)
            .include_y(0.0)
            .legend(egui_plot::Legend::default())
            .label_formatter(|_name, value| {
                format!("t=-{:.0}s rate={:.1}/s", 60.0 - value.x, value.y)
            })
            .show(ui, |plot_ui| {
                for (et, rates) in &rates_per_type {
                    let points: Vec<[f64; 2]> = rates
                        .iter()
                        .enumerate()
                        .map(|(x, &y)| [x as f64, y])
                        .collect();
                    let color = self.get_event_color(*et);
                    let name = event_name(*et);
                    plot_ui.line(
                        Line::new(PlotPoints::from(points))
                            .color(color)
                            .width(2.0)
                            .name(name),
                    );
                }
            });
    }

    /// Recent errors scrolling list
    fn render_recent_errors(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Recent Errors")
                .color(colors::TEXT_MUTED)
                .size(14.0),
        );

        let now = now_seconds();

        let recent = with_data!(self, |data| {
            data.events.recent_errors(ERROR_TYPES, 50, now, 120.0)
        });

        if recent.is_empty() {
            ui.label(
                egui::RichText::new("No recent errors")
                    .color(colors::TEXT_MUTED)
                    .size(12.0),
            );
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("recent_errors")
            .show(ui, |ui| {
                for error in &recent {
                    let age = now - error.timestamp;
                    let color = self.get_event_color(error.event_type);
                    let name = event_name(error.event_type);

                    ui.horizontal(|ui| {
                        // Age
                        ui.label(
                            egui::RichText::new(format!("-{:.1}s", age))
                                .color(colors::TEXT_MUTED)
                                .monospace()
                                .size(12.0),
                        );

                        // Event type badge
                        ui.label(
                            egui::RichText::new(format!("[{}]", name))
                                .color(color)
                                .monospace()
                                .size(12.0),
                        );

                        // Node
                        ui.label(
                            egui::RichText::new(format!("node #{}", error.node_index))
                                .color(colors::TEXT_SECONDARY)
                                .monospace()
                                .size(12.0),
                        );

                        // Reason
                        if !error.reason.is_empty() {
                            ui.label(
                                egui::RichText::new(&error.reason)
                                    .color(colors::TEXT_MUTED)
                                    .size(11.0),
                            );
                        }
                    });
                }
            });
    }
}
