//! Accordion-style event filter with tri-state + errors checkboxes

use eframe::egui;
use crate::core::{event_name, EVENT_CATEGORIES};
use crate::core::events::ERROR_EVENT_TYPES;
use crate::theme::colors;
use super::JamApp;

// ── Pure state-transition functions (testable without egui) ──

/// Tri-state checkbox click: all→none, else→all.
pub fn toggle_category_all(selected: &mut [bool], event_types: &[u8]) {
    let all_on = event_types.iter().all(|&et| selected[et as usize]);
    let new_val = !all_on;
    for &et in event_types {
        selected[et as usize] = new_val;
    }
}

/// Errors checkbox click: if already errors-only→all, else→errors-only.
pub fn toggle_category_errors(selected: &mut [bool], event_types: &[u8]) {
    let is_errors_only = event_types.iter().all(|&et| {
        let is_err = ERROR_EVENT_TYPES.contains(&et);
        selected[et as usize] == is_err
    });
    if is_errors_only {
        // Restore to all
        for &et in event_types {
            selected[et as usize] = true;
        }
    } else {
        // Set errors only
        for &et in event_types {
            selected[et as usize] = ERROR_EVENT_TYPES.contains(&et);
        }
    }
}

/// Check if a category is in "errors only" state.
pub fn is_errors_only(selected: &[bool], event_types: &[u8]) -> bool {
    let has_any_error = event_types.iter().any(|&et| ERROR_EVENT_TYPES.contains(&et));
    if !has_any_error {
        return false;
    }
    event_types.iter().all(|&et| {
        let is_err = ERROR_EVENT_TYPES.contains(&et);
        selected[et as usize] == is_err
    })
}

// ── UI rendering ──

impl JamApp {
    pub(crate) fn render_event_selector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("event_filter")
            .default_width(300.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY).inner_margin(8.0))
            .show(ctx, |ui| {
                // Global buttons
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("All")).clicked() {
                        self.apply_all_filter();
                    }
                    if ui.button(egui::RichText::new("None")).clicked() {
                        self.selected_events.fill(false);
                        self.errors_only = false;
                    }
                    if ui.button(egui::RichText::new("Errors")).clicked() {
                        self.apply_errors_filter();
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("filter_accordion")
                    .show(ui, |ui| {
                        let mut new_expanded = self.expanded_category;

                        // Detect single-category mode (exactly one category has any enabled event)
                        let active_cat_count = EVENT_CATEGORIES.iter()
                            .filter(|cat| cat.event_types.iter().any(|&et|
                                (et as usize) < self.selected_events.len() && self.selected_events[et as usize]
                            ))
                            .count();
                        let is_single_category = active_cat_count == 1;

                        for (cat_idx, category) in EVENT_CATEGORIES.iter().enumerate() {
                            let selected_count = category
                                .event_types
                                .iter()
                                .filter(|&&et| self.selected_events[et as usize])
                                .count();
                            let total = category.event_types.len();
                            let all_selected = selected_count == total;
                            let none_selected = selected_count == 0;
                            let is_expanded = self.expanded_category == Some(cat_idx);
                            let has_errors = category
                                .event_types
                                .iter()
                                .any(|et| ERROR_EVENT_TYPES.contains(et));
                            let errors_active = is_errors_only(
                                &self.selected_events,
                                category.event_types,
                            );

                            // ── Category row ──
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;

                                // Left checkbox (tri-state)
                                let mut cat_checked = all_selected;
                                let cb = ui.checkbox(&mut cat_checked, "");
                                // Draw dash for partial state
                                if !all_selected && !none_selected {
                                    let rect = cb.rect;
                                    let c = rect.center();
                                    let h = rect.width() * 0.2;
                                    ui.painter().line_segment(
                                        [egui::pos2(c.x - h, c.y), egui::pos2(c.x + h, c.y)],
                                        egui::Stroke::new(2.0, colors::TEXT_PRIMARY),
                                    );
                                }
                                let left_clicked = cb.clicked();
                                let modifier = ui.input(|i| i.modifiers.ctrl || i.modifiers.shift);
                                let left_tooltip = if modifier {
                                    "Solo this category (Ctrl/Shift+click)"
                                } else if all_selected {
                                    "Deselect all"
                                } else {
                                    "Select all"
                                };
                                cb.on_hover_text(left_tooltip);
                                if left_clicked {
                                    if modifier {
                                        // Solo: deselect everything, then enable only this category
                                        self.selected_events.fill(false);
                                        for &et in category.event_types {
                                            self.selected_events[et as usize] = true;
                                        }
                                    } else {
                                        toggle_category_all(
                                            &mut self.selected_events,
                                            category.event_types,
                                        );
                                    }
                                }

                                // Right checkbox (errors, red-tinted)
                                if has_errors {
                                    let size = 16.0;
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(size, size),
                                        egui::Sense::click(),
                                    );
                                    let right_tooltip = if errors_active {
                                        "Select all"
                                    } else {
                                        "Select errors only"
                                    };
                                    response.clone().on_hover_text(right_tooltip);
                                    if response.clicked() {
                                        toggle_category_errors(
                                            &mut self.selected_events,
                                            category.event_types,
                                        );
                                    }

                                    // Paint the errors checkbox
                                    let rounding = 2.0;
                                    if errors_active {
                                        ui.painter().rect_filled(
                                            rect,
                                            rounding,
                                            egui::Color32::from_rgb(180, 60, 60),
                                        );
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "E",
                                            egui::FontId::proportional(10.0),
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().rect_stroke(
                                            rect,
                                            rounding,
                                            egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 40, 40)),
                                            egui::StrokeKind::Outside,
                                        );
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "E",
                                            egui::FontId::proportional(10.0),
                                            egui::Color32::from_rgb(100, 40, 40),
                                        );
                                    }
                                }

                                // Color dot — only in multi-category mode
                                if !is_single_category {
                                    let base_color = self.get_event_color(category.event_types[0]);
                                    let alpha = if none_selected { 60 } else { 220 };
                                    let dot_color = egui::Color32::from_rgba_unmultiplied(
                                        base_color.r(), base_color.g(), base_color.b(), alpha,
                                    );
                                    let (dot_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, 10.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(
                                        dot_rect.center(),
                                        5.0,
                                        dot_color,
                                    );
                                }

                                // Category name + count + arrow (single clickable element)
                                let text_color = if none_selected {
                                    colors::TEXT_MUTED
                                } else {
                                    colors::TEXT_SECONDARY
                                };
                                let arrow = if is_expanded { "▾" } else { "▸" };
                                let label_text = format!(
                                    "{} ({}/{}) {}",
                                    category.name, selected_count, total, arrow
                                );
                                let label = ui.selectable_label(
                                    is_expanded,
                                    egui::RichText::new(label_text).color(text_color),
                                );
                                if label.clicked() {
                                    new_expanded = if is_expanded { None } else { Some(cat_idx) };
                                }
                            });

                            // ── Expanded events ──
                            if is_expanded {
                                ui.indent(cat_idx, |ui| {
                                    for &et in category.event_types {
                                        let mut enabled = self.selected_events[et as usize];
                                        let name = event_name(et);
                                        let text_color = if enabled {
                                            colors::TEXT_PRIMARY
                                        } else {
                                            colors::TEXT_MUTED
                                        };
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 4.0;
                                            if ui
                                                .checkbox(
                                                    &mut enabled,
                                                    "",
                                                )
                                                .changed()
                                            {
                                                self.selected_events[et as usize] = enabled;
                                            }

                                            // Color dot — only in single-category mode
                                            if is_single_category {
                                                let evt_color = self.get_event_color(et);
                                                let alpha = if enabled { 220 } else { 60 };
                                                let dot_color = egui::Color32::from_rgba_unmultiplied(
                                                    evt_color.r(), evt_color.g(), evt_color.b(), alpha,
                                                );
                                                let (dot_rect, _) = ui.allocate_exact_size(
                                                    egui::vec2(10.0, 10.0),
                                                    egui::Sense::hover(),
                                                );
                                                ui.painter().circle_filled(
                                                    dot_rect.center(),
                                                    5.0,
                                                    dot_color,
                                                );
                                            }

                                            ui.label(egui::RichText::new(name).color(text_color));
                                        });
                                    }
                                });
                            }
                        }

                        self.expanded_category = new_expanded;
                    });
            });

    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a selected_events array with all true.
    fn all_selected() -> Vec<bool> {
        vec![true; 256]
    }

    /// Helper: create a selected_events array with all false.
    fn none_selected() -> Vec<bool> {
        vec![false; 256]
    }

    // Use Connection category: event_types = [20..28]
    // Error events in Connection: 22 (ConnectFailed), 26 (ConnectionDropped)
    const CONNECTION_EVENTS: &[u8] = &[20, 21, 22, 23, 24, 25, 26, 27, 28];

    fn connection_error_types() -> Vec<u8> {
        CONNECTION_EVENTS
            .iter()
            .copied()
            .filter(|et| ERROR_EVENT_TYPES.contains(et))
            .collect()
    }

    #[test]
    fn click_left_when_all_selected_turns_none() {
        let mut sel = all_selected();
        toggle_category_all(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| !sel[et as usize]));
    }

    #[test]
    fn click_left_when_none_selected_turns_all() {
        let mut sel = none_selected();
        toggle_category_all(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| sel[et as usize]));
    }

    #[test]
    fn click_left_when_partial_turns_all() {
        let mut sel = none_selected();
        sel[20] = true;
        sel[21] = true;
        toggle_category_all(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| sel[et as usize]));
    }

    #[test]
    fn click_left_when_errors_only_turns_all() {
        let mut sel = none_selected();
        for &et in &connection_error_types() {
            sel[et as usize] = true;
        }
        assert!(is_errors_only(&sel, CONNECTION_EVENTS));
        toggle_category_all(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| sel[et as usize]));
    }

    #[test]
    fn click_right_when_all_turns_errors_only() {
        let mut sel = all_selected();
        toggle_category_errors(&mut sel, CONNECTION_EVENTS);
        let errs = connection_error_types();
        for &et in CONNECTION_EVENTS {
            assert_eq!(sel[et as usize], errs.contains(&et));
        }
    }

    #[test]
    fn click_right_when_none_turns_errors_only() {
        let mut sel = none_selected();
        toggle_category_errors(&mut sel, CONNECTION_EVENTS);
        let errs = connection_error_types();
        for &et in CONNECTION_EVENTS {
            assert_eq!(sel[et as usize], errs.contains(&et));
        }
    }

    #[test]
    fn click_right_when_errors_only_turns_all() {
        let mut sel = none_selected();
        for &et in &connection_error_types() {
            sel[et as usize] = true;
        }
        toggle_category_errors(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| sel[et as usize]));
    }

    #[test]
    fn click_right_when_partial_turns_errors_only() {
        let mut sel = none_selected();
        sel[20] = true;
        sel[23] = true;
        toggle_category_errors(&mut sel, CONNECTION_EVENTS);
        let errs = connection_error_types();
        for &et in CONNECTION_EVENTS {
            assert_eq!(sel[et as usize], errs.contains(&et));
        }
    }

    #[test]
    fn is_errors_only_false_for_category_without_errors() {
        let sel = all_selected();
        // Meta has event_type [0], and 0 IS in ERROR_EVENT_TYPES (Dropped),
        // so use a fake category with no errors
        let fake_types: &[u8] = &[10, 11, 12, 13]; // Status — check if any are errors
        let has_errors = fake_types.iter().any(|et| ERROR_EVENT_TYPES.contains(et));
        if !has_errors {
            assert!(!is_errors_only(&sel, fake_types));
        }
    }
}
