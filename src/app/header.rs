//! Header bar with controls, tabs, and status

use eframe::egui;
use crate::theme::colors;
use crate::time::now_seconds;
use super::{JamApp, ActiveTab, with_data};

impl JamApp {
    pub(crate) fn render_header(&mut self, ui: &mut egui::Ui) {
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
            // LEFT: Control buttons — Filter first
            let filter_text = if self.show_event_selector { "Filter <<<" } else { "Filter >>>" };
            if ui.button(egui::RichText::new(filter_text)).clicked() {
                self.show_event_selector = !self.show_event_selector;
            }

            ui.add_space(10.0);

            // Tab buttons
            const TABS: &[(ActiveTab, &str)] = &[
                (ActiveTab::Ring, "Ring"),
                (ActiveTab::Graphs, "Graphs"),
            ];

            for &(tab, label) in TABS {
                let color = if self.active_tab == tab {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_MUTED
                };

                if ui
                    .selectable_label(
                        self.active_tab == tab,
                        egui::RichText::new(label).color(color),
                    )
                    .clicked()
                {
                    self.active_tab = tab;
                }
            }

            // RIGHT: Status and stats (right-to-left order)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.particle_max > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}/{} particles", self.particle_count, self.particle_max
                        ))
                        .color(colors::TEXT_MUTED),
                    );
                    ui.label(egui::RichText::new("/").color(colors::TEXT_MUTED));
                }

                ui.label(
                    egui::RichText::new(format!("{} nodes", event_count))
                        .color(colors::TEXT_MUTED),
                );
                ui.label(egui::RichText::new("/").color(colors::TEXT_MUTED));

                if let Some(slot) = highest_slot {
                    ui.label(
                        egui::RichText::new(format!("slot {}", slot))
                            .color(colors::TEXT_MUTED),
                    );
                    ui.label(egui::RichText::new("/").color(colors::TEXT_MUTED));
                }

                ui.label(
                    egui::RichText::new(format!("{} validators", validator_count))
                        .color(colors::TEXT_MUTED),
                );
                ui.label(egui::RichText::new("/").color(colors::TEXT_MUTED));

                ui.label(
                    egui::RichText::new(format!("{:.0} fps", self.fps_counter.fps()))
                        .color(colors::TEXT_SECONDARY),
                );

                ui.add_space(10.0);

                let (status_color, status_text) = match &ws_state {
                    crate::ws_state::WsState::Connected => (egui::Color32::from_rgb(100, 200, 100), "Connected"),
                    crate::ws_state::WsState::Connecting => (egui::Color32::from_rgb(200, 200, 100), "Connecting..."),
                    crate::ws_state::WsState::Disconnected => (egui::Color32::from_rgb(200, 100, 100), "Disconnected"),
                    crate::ws_state::WsState::Error(_) => (egui::Color32::from_rgb(200, 100, 100), "Error"),
                };
                ui.colored_label(status_color, egui::RichText::new(status_text));
            });
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
        let now = now_seconds() * 1000.0;
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
