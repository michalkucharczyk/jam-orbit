//! Header bar with controls and tabs

use eframe::egui;
use crate::theme::colors;
use crate::time::now_seconds;
use super::{JamApp, ActiveTab};

impl JamApp {
    pub(crate) fn render_header(&mut self, ui: &mut egui::Ui) {
        self.fps_counter.tick();

        ui.horizontal(|ui| {
            // LEFT: Control buttons — Filter, Settings, then tabs
            let filter_text = if self.show_event_selector { "Filter <" } else { "Filter >" };
            if ui.button(egui::RichText::new(filter_text)).clicked() {
                self.show_event_selector = !self.show_event_selector;
            }

            // Settings toggle
            let settings_color = if self.show_settings {
                colors::TEXT_PRIMARY
            } else {
                colors::TEXT_MUTED
            };
            if ui
                .selectable_label(
                    self.show_settings,
                    egui::RichText::new("Settings").color(settings_color),
                )
                .clicked()
            {
                self.show_settings = !self.show_settings;
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
