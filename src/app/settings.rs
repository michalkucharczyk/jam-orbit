//! Settings sidebar — ring visualization toggles, particle speed, color schema

use eframe::egui;
use crate::theme::colors;
use crate::vring::ColorSchema;
use super::JamApp;

impl JamApp {
    pub(crate) fn render_settings(&mut self, ctx: &egui::Context) {
        let half_width = ctx.screen_rect().width() * 0.18;
        egui::SidePanel::left("settings")
            .default_width(half_width)
            .min_width(240.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY).inner_margin(8.0))
            .show(ctx, |ui| {
                let group_frame = egui::Frame::new()
                    .stroke(egui::Stroke::new(1.0, colors::TEXT_MUTED.gamma_multiply(0.6)))
                    .corner_radius(4.0)
                    .inner_margin(6.0);

                group_frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new("Ring:").color(colors::TEXT_MUTED));

                    ui.checkbox(&mut self.slot_pulse_enabled, "Slot pulse");
                    ui.checkbox(
                        &mut self.node_brightness_enabled,
                        egui::RichText::new("Node brightness").color(colors::TEXT_PRIMARY),
                    );
                    if self.node_brightness_enabled {
                        ui.label(
                            egui::RichText::new("  Dot brightness reflects peer count")
                                .color(colors::TEXT_MUTED)
                                .small(),
                        );
                    }

                    ui.add_space(4.0);
                    let speed_label = format!("Particle speed: {:.1}x", self.speed_factor);
                    ui.label(egui::RichText::new(speed_label).color(colors::TEXT_MUTED));
                    let full_width = ui.available_width();
                    ui.spacing_mut().slider_width = full_width;
                    let speed_response = ui.add(
                        egui::Slider::new(&mut self.speed_factor, 0.1..=2.0)
                            .logarithmic(true)
                            .clamping(egui::SliderClamping::Always)
                            .show_value(false),
                    );
                    if speed_response.double_clicked() {
                        self.speed_factor = 1.0;
                    }
                });

                ui.add_space(8.0);

                group_frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new("Color schema:").color(colors::TEXT_MUTED));
                    for &schema in ColorSchema::ALL {
                        ui.radio_value(&mut self.color_schema, schema, schema.label());
                    }
                });
            });
    }
}
