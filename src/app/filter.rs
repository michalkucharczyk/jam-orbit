//! Event filter selector modal

use eframe::egui;
use crate::core::{event_color_rgb, event_name, EVENT_CATEGORIES};
use crate::core::events::ERROR_EVENT_TYPES;
use crate::theme::colors;
use super::JamApp;

impl JamApp {
    pub(crate) fn render_event_selector(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("Event Filter")
            .open(&mut open)
            .resizable(true)
            .default_width(600.0)
            .default_height(500.0)
            .default_pos(egui::pos2(300.0, 100.0))
            .show(ctx, |ui| {
                // Global All/None buttons
                ui.horizontal(|ui| {
                    if ui
                        .button(egui::RichText::new("All").size(16.0))
                        .clicked()
                    {
                        self.apply_all_filter();
                    }
                    if ui
                        .button(egui::RichText::new("None").size(16.0))
                        .clicked()
                    {
                        self.selected_events.fill(false);
                        self.errors_only = false;
                    }
                    if ui
                        .button(egui::RichText::new("Errors").size(16.0))
                        .clicked()
                    {
                        self.apply_errors_filter();
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Two-panel layout using columns
                ui.columns(2, |columns| {
                    // Left column: category list
                    columns[0].vertical(|ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("filter_categories")
                            .show(ui, |ui| {
                                for (cat_idx, category) in EVENT_CATEGORIES.iter().enumerate() {
                                    let selected_count = category
                                        .event_types
                                        .iter()
                                        .filter(|&&et| self.selected_events[et as usize])
                                        .count();
                                    let total = category.event_types.len();
                                    let all_selected = selected_count == total;
                                    let none_selected = selected_count == 0;

                                    let is_active = self.selected_category == cat_idx;

                                    ui.horizontal(|ui| {
                                        // Group checkbox
                                        let mut cat_checked = all_selected;
                                        let checkbox_response = ui.checkbox(&mut cat_checked, "");
                                        // Partial indicator (dash) for mixed state
                                        if !all_selected && !none_selected {
                                            let rect = checkbox_response.rect;
                                            let center = rect.center();
                                            let half = rect.width() * 0.2;
                                            ui.painter().line_segment(
                                                [
                                                    egui::pos2(center.x - half, center.y),
                                                    egui::pos2(center.x + half, center.y),
                                                ],
                                                egui::Stroke::new(2.0, colors::TEXT_PRIMARY),
                                            );
                                        }
                                        if checkbox_response.changed() {
                                            for &et in category.event_types {
                                                self.selected_events[et as usize] = cat_checked;
                                            }
                                        }

                                        // Color swatch
                                        let (r, g, b) = event_color_rgb(category.event_types[0]);
                                        let swatch_alpha = if none_selected { 60 } else { 200 };
                                        let swatch_color = egui::Color32::from_rgba_unmultiplied(
                                            r,
                                            g,
                                            b,
                                            swatch_alpha,
                                        );
                                        let (swatch_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(12.0, 12.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            swatch_rect,
                                            2.0,
                                            swatch_color,
                                        );

                                        // Clickable category label
                                        let text_color = if is_active {
                                            colors::TEXT_PRIMARY
                                        } else if none_selected {
                                            colors::TEXT_MUTED
                                        } else {
                                            colors::TEXT_SECONDARY
                                        };

                                        let label_text = format!(
                                            "{} ({}/{})",
                                            category.name, selected_count, total
                                        );
                                        let label = ui.selectable_label(
                                            is_active,
                                            egui::RichText::new(label_text)
                                                .color(text_color)
                                                .size(16.0),
                                        );
                                        if label.clicked() {
                                            self.selected_category = cat_idx;
                                        }
                                    });
                                }
                            });
                    });

                    // Right column: individual events for selected category
                    columns[1].vertical(|ui| {
                        if self.selected_category < EVENT_CATEGORIES.len() {
                            let category = &EVENT_CATEGORIES[self.selected_category];

                            // Per-category header with All/None
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(category.name)
                                        .color(colors::TEXT_PRIMARY)
                                        .size(16.0)
                                        .strong(),
                                );
                                if ui
                                    .button(egui::RichText::new("All").size(14.0))
                                    .clicked()
                                {
                                    for &et in category.event_types {
                                        self.selected_events[et as usize] = true;
                                    }
                                }
                                if ui
                                    .button(egui::RichText::new("None").size(14.0))
                                    .clicked()
                                {
                                    for &et in category.event_types {
                                        self.selected_events[et as usize] = false;
                                    }
                                }
                                // Only show Errors button if this category has error events
                                let has_errors = category.event_types.iter().any(|et| ERROR_EVENT_TYPES.contains(et));
                                if has_errors {
                                    if ui
                                        .button(egui::RichText::new("Errors").size(14.0))
                                        .clicked()
                                    {
                                        for &et in category.event_types {
                                            self.selected_events[et as usize] = ERROR_EVENT_TYPES.contains(&et);
                                        }
                                    }
                                }
                            });

                            ui.add_space(4.0);

                            egui::ScrollArea::vertical()
                                .id_salt("filter_events")
                                .show(ui, |ui| {
                                    for &et in category.event_types {
                                        let mut enabled = self.selected_events[et as usize];
                                        let name = event_name(et);
                                        let text_color = if enabled {
                                            colors::TEXT_PRIMARY
                                        } else {
                                            colors::TEXT_MUTED
                                        };
                                        if ui
                                            .checkbox(
                                                &mut enabled,
                                                egui::RichText::new(name)
                                                    .color(text_color)
                                                    .size(16.0),
                                            )
                                            .changed()
                                        {
                                            self.selected_events[et as usize] = enabled;
                                        }
                                    }
                                });
                        }
                    });
                });
            });

        if !open {
            self.show_event_selector = false;
        }
    }
}
