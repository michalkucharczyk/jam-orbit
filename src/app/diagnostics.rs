//! Diagnostics window — connection status, rates, and drop counts

use eframe::egui;
use crate::theme::colors;
use super::{JamApp, with_data};

/// Format a count with human-readable suffix (1234 → "1.2k", 5000000 → "5.0M")
fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format a rate with human-readable suffix
fn format_rate(r: f64) -> String {
    if r >= 1_000_000.0 {
        format!("{:.1}M", r / 1_000_000.0)
    } else if r >= 10_000.0 {
        format!("{:.1}k", r / 1_000.0)
    } else if r >= 100.0 {
        format!("{:.0}", r)
    } else if r >= 1.0 {
        format!("{:.1}", r)
    } else {
        format!("{:.2}", r)
    }
}

impl JamApp {
    pub(crate) fn draw_diagnostics(&self, ctx: &egui::Context) {
        let ws_state = self.get_ws_state();

        let (node_count, highest_slot) = with_data!(self, |data| {
            (
                data.events.node_count(),
                data.blocks.highest_slot(),
            )
        });

        // Status indicator and text
        let (indicator, status_text, status_color) = match &ws_state {
            crate::ws_state::WsState::Connected => (
                "●",
                "Connected",
                egui::Color32::from_rgb(100, 200, 100),
            ),
            crate::ws_state::WsState::Connecting => (
                "●",
                "Connecting...",
                egui::Color32::from_rgb(200, 200, 100),
            ),
            crate::ws_state::WsState::Disconnected => (
                "✕",
                "Disconnected",
                egui::Color32::from_rgb(200, 100, 100),
            ),
            crate::ws_state::WsState::Error(_) => (
                "✕",
                "Error",
                egui::Color32::from_rgb(200, 100, 100),
            ),
        };

        let title = egui::RichText::new(format!("{} {}", indicator, status_text))
            .color(status_color);

        egui::Area::new(egui::Id::new("diagnostics_area"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 36.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 200))
                    .corner_radius(4.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_min_width(360.0);
                        let header = egui::CollapsingHeader::new(title)
                            .default_open(true);

                        header.show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(format!("{:.0} fps", self.fps_counter.fps()))
                                    .color(colors::TEXT_SECONDARY),
                            );

                            ui.label(
                                egui::RichText::new(format!("{} nodes", node_count))
                                    .color(colors::TEXT_MUTED),
                            );

                            if let Some(slot) = highest_slot {
                                ui.label(
                                    egui::RichText::new(format!("slot {}", slot))
                                        .color(colors::TEXT_MUTED),
                                );
                            }

                            ui.label(
                                egui::RichText::new(format!(
                                    "{}/s WS events",
                                    format_rate(self.diag_events_sec),
                                ))
                                .color(colors::TEXT_MUTED),
                            );

                            // Dropped events — always show rate, highlight in red if > 0
                            let total_dropped = self.diag_server_dropped_total;
                            let dropped_text = format!(
                                "{} dropped ({}/s)",
                                format_count(total_dropped),
                                format_rate(self.diag_dropped_sec),
                            );
                            let dropped_color = if total_dropped > 0 {
                                egui::Color32::from_rgb(200, 100, 100)
                            } else {
                                colors::TEXT_MUTED
                            };
                            ui.label(
                                egui::RichText::new(dropped_text).color(dropped_color),
                            );

                            if self.particle_max > 0 {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}/{} events in GPU",
                                        format_count(self.particle_count as u64),
                                        format_count(self.particle_max as u64),
                                    ))
                                    .color(colors::TEXT_MUTED),
                                );
                            }
                        });
                    });
            });
    }
}
