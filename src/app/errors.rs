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
        let chart_height = available.y * 0.55;
        let list_height = available.y * 0.45 - 8.0;

        // Top: Error rate chart (full width)
        ui.allocate_ui(egui::vec2(available.x, chart_height), |ui| {
            self.render_error_rates(ui);
        });

        ui.add_space(4.0);

        // Bottom: Recent errors list (full width)
        ui.allocate_ui(egui::vec2(available.x, list_height), |ui| {
            self.render_recent_errors(ui);
        });
    }

    /// Error rate by type (one colored line per error type)
    fn render_error_rates(&self, ui: &mut egui::Ui) {
        let now = now_seconds();

        let total_count: u64 = with_data!(self, |data| {
            data.events.count_events(ERROR_TYPES, now, 60.0)
        });

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

        let max_y = rates_per_type.iter()
            .flat_map(|(_, rates)| rates.iter())
            .copied()
            .fold(0.0_f64, f64::max);

        let resp = Plot::new("error_rates")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(60.0)
            .include_y(0.0)
            .include_y((max_y + 1.0).max(2.0))
            .y_axis_formatter(|mark, _| {
                if (mark.value - mark.value.round()).abs() < 0.01 {
                    format!("{:.0}", mark.value)
                } else {
                    String::new()
                }
            })
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

        let title = format!("Error Rate by Type ({} errors in 60s)", total_count);
        let title_color = if total_count > 0 {
            egui::Color32::from_rgb(255, 100, 100)
        } else {
            colors::TEXT_MUTED
        };
        Self::paint_plot_title(ui, resp.response.rect, &title, title_color);
    }

    /// Recent errors scrolling list
    fn render_recent_errors(&self, ui: &mut egui::Ui) {
        let now = now_seconds();

        let recent = with_data!(self, |data| {
            data.events.recent_errors(ERROR_TYPES, 50, now, 120.0)
        });

        // Title bar
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Recent Errors ({})", recent.len()))
                    .color(colors::TEXT_MUTED)
                    .size(13.0),
            );
        });

        if recent.is_empty() {
            ui.label(
                egui::RichText::new("No recent errors")
                    .color(colors::TEXT_MUTED)
                    .size(12.0),
            );
            return;
        }

        ui.add_space(2.0);

        egui::ScrollArea::vertical()
            .id_salt("recent_errors_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for error in &recent {
                    let age = now - error.timestamp;
                    let color = self.get_event_color(error.event_type);
                    let name = event_name(error.event_type);

                    let reason_text = if error.reason.is_empty() {
                        String::new()
                    } else {
                        format!("  {}", error.reason)
                    };

                    // Single-line entry using RichText layout
                    let text = format!(
                        "{:>6.1}s  [{:<24}]  node #{:<4}{}",
                        -age, name, error.node_index, reason_text
                    );

                    ui.label(
                        egui::RichText::new(text)
                            .color(color)
                            .monospace()
                            .size(12.0),
                    );
                }
            });
    }
}
