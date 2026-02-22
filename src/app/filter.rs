//! Accordion-style event filter with tri-state + errors checkboxes

use eframe::egui;
use crate::core::{event_name, EVENT_CATEGORIES, OUTBOUND_EVENTS, INBOUND_EVENTS, BIDIR_EVENTS};
use crate::core::events::{ERROR_EVENT_TYPES, EventType};
use crate::theme::colors;
use super::JamApp;

// ── Pure state-transition functions (testable without egui) ──

/// Tri-state checkbox click: all→none, else→all.
pub fn toggle_category_all(selected: &mut [bool], event_types: &[EventType]) {
    let all_on = event_types.iter().all(|&et| selected[et.idx()]);
    let new_val = !all_on;
    for &et in event_types {
        selected[et.idx()] = new_val;
    }
}

/// Errors checkbox click: if already errors-only→all, else→errors-only.
pub fn toggle_category_errors(selected: &mut [bool], event_types: &[EventType]) {
    let is_errors_only = event_types.iter().all(|&et| {
        let is_err = ERROR_EVENT_TYPES.contains(&et);
        selected[et.idx()] == is_err
    });
    if is_errors_only {
        for &et in event_types {
            selected[et.idx()] = true;
        }
    } else {
        for &et in event_types {
            selected[et.idx()] = ERROR_EVENT_TYPES.contains(&et);
        }
    }
}

/// Check if a category is in "errors only" state.
pub fn is_errors_only(selected: &[bool], event_types: &[EventType]) -> bool {
    let has_any_error = event_types.iter().any(|et| ERROR_EVENT_TYPES.contains(et));
    if !has_any_error {
        return false;
    }
    event_types.iter().all(|&et| {
        let is_err = ERROR_EVENT_TYPES.contains(&et);
        selected[et.idx()] == is_err
    })
}

/// Narrow selection: keep only events in `keep` set, disable all others.
pub fn narrow_keep_only(selected: &mut [bool], keep: &[EventType]) {
    for (i, sel) in selected.iter_mut().enumerate() {
        if !keep.iter().any(|&et| et.idx() == i) {
            *sel = false;
        }
    }
}

/// Narrow selection: remove events in `remove` set.
pub fn narrow_remove(selected: &mut [bool], remove: &[EventType]) {
    for &et in remove {
        selected[et.idx()] = false;
    }
}

// ── UI rendering ──

impl JamApp {
    pub(crate) fn render_event_selector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("event_filter")
            .default_width(300.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY).inner_margin(8.0))
            .show(ctx, |ui| {
                let group_frame = egui::Frame::new()
                    .stroke(egui::Stroke::new(1.0, colors::TEXT_MUTED.gamma_multiply(0.6)))
                    .corner_radius(4.0)
                    .inner_margin(6.0);

                // ── Select group ──
                group_frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new("Select:").color(colors::TEXT_MUTED));
                    ui.horizontal(|ui| {
                        if ui.button("All").clicked() {
                            self.apply_all_filter();
                        }
                        if ui.button("None").clicked() {
                            self.selected_events.fill(false);
                            self.errors_only = false;
                        }
                        if ui.button("Errors").clicked() {
                            self.apply_errors_filter();
                        }
                    });
                });

                ui.add_space(4.0);

                // ── Narrow group ──
                group_frame.show(ui, |ui| {
                    ui.label(egui::RichText::new("Narrow:").color(colors::TEXT_MUTED));
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Outbound").clicked() {
                            narrow_keep_only(&mut self.selected_events, OUTBOUND_EVENTS);
                        }
                        if ui.button("Inbound").clicked() {
                            narrow_keep_only(&mut self.selected_events, INBOUND_EVENTS);
                        }
                        if ui.button("Bidir").clicked() {
                            narrow_keep_only(&mut self.selected_events, BIDIR_EVENTS);
                        }
                        if ui.button("Local").clicked() {
                            narrow_remove(&mut self.selected_events, OUTBOUND_EVENTS);
                            narrow_remove(&mut self.selected_events, INBOUND_EVENTS);
                            narrow_remove(&mut self.selected_events, BIDIR_EVENTS);
                        }
                        if ui.button("No Errors").clicked() {
                            narrow_remove(&mut self.selected_events, ERROR_EVENT_TYPES);
                        }
                    });
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
                                et.idx() < self.selected_events.len() && self.selected_events[et.idx()]
                            ))
                            .count();
                        let is_single_category = active_cat_count == 1;

                        for (cat_idx, category) in EVENT_CATEGORIES.iter().enumerate() {
                            let selected_count = category
                                .event_types
                                .iter()
                                .filter(|&&et| self.selected_events[et.idx()])
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
                                            self.selected_events[et.idx()] = true;
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
                                        let mut enabled = self.selected_events[et.idx()];
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
                                                self.selected_events[et.idx()] = enabled;
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

    use crate::core::events::EventType;

    const CONNECTION_EVENTS: &[EventType] = &[
        EventType::ConnectionRefused, EventType::ConnectingIn, EventType::ConnectInFailed,
        EventType::ConnectedIn, EventType::ConnectingOut, EventType::ConnectOutFailed,
        EventType::ConnectedOut, EventType::Disconnected, EventType::PeerMisbehaved,
    ];

    fn connection_error_types() -> Vec<EventType> {
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
        assert!(CONNECTION_EVENTS.iter().all(|&et| !sel[et.idx()]));
    }

    #[test]
    fn click_left_when_none_selected_turns_all() {
        let mut sel = none_selected();
        toggle_category_all(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| sel[et.idx()]));
    }

    #[test]
    fn click_left_when_partial_turns_all() {
        let mut sel = none_selected();
        sel[EventType::ConnectionRefused.idx()] = true;
        sel[EventType::ConnectingIn.idx()] = true;
        toggle_category_all(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| sel[et.idx()]));
    }

    #[test]
    fn click_left_when_errors_only_turns_all() {
        let mut sel = none_selected();
        for &et in &connection_error_types() {
            sel[et.idx()] = true;
        }
        assert!(is_errors_only(&sel, CONNECTION_EVENTS));
        toggle_category_all(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| sel[et.idx()]));
    }

    #[test]
    fn click_right_when_all_turns_errors_only() {
        let mut sel = all_selected();
        toggle_category_errors(&mut sel, CONNECTION_EVENTS);
        let errs = connection_error_types();
        for &et in CONNECTION_EVENTS {
            assert_eq!(sel[et.idx()], errs.contains(&et));
        }
    }

    #[test]
    fn click_right_when_none_turns_errors_only() {
        let mut sel = none_selected();
        toggle_category_errors(&mut sel, CONNECTION_EVENTS);
        let errs = connection_error_types();
        for &et in CONNECTION_EVENTS {
            assert_eq!(sel[et.idx()], errs.contains(&et));
        }
    }

    #[test]
    fn click_right_when_errors_only_turns_all() {
        let mut sel = none_selected();
        for &et in &connection_error_types() {
            sel[et.idx()] = true;
        }
        toggle_category_errors(&mut sel, CONNECTION_EVENTS);
        assert!(CONNECTION_EVENTS.iter().all(|&et| sel[et.idx()]));
    }

    #[test]
    fn click_right_when_partial_turns_errors_only() {
        let mut sel = none_selected();
        sel[EventType::ConnectionRefused.idx()] = true;
        sel[EventType::ConnectedIn.idx()] = true;
        toggle_category_errors(&mut sel, CONNECTION_EVENTS);
        let errs = connection_error_types();
        for &et in CONNECTION_EVENTS {
            assert_eq!(sel[et.idx()], errs.contains(&et));
        }
    }

    #[test]
    fn narrow_keep_only_intersects_selection() {
        use crate::core::events::EventType;
        let mut sel = all_selected();
        let keep = &[EventType::ConnectedIn, EventType::ConnectingOut, EventType::Disconnected];
        narrow_keep_only(&mut sel, keep);
        assert!(sel[EventType::ConnectedIn as usize]);
        assert!(sel[EventType::ConnectingOut as usize]);
        assert!(sel[EventType::Disconnected as usize]);
        assert!(!sel[EventType::Status as usize]);
        assert!(!sel[EventType::SendingGuarantee as usize]);
    }

    #[test]
    fn narrow_keep_only_preserves_already_disabled() {
        use crate::core::events::EventType;
        let mut sel = none_selected();
        sel[EventType::ConnectedIn as usize] = true;
        sel[EventType::ConnectingOut as usize] = true;
        sel[EventType::Status as usize] = true;
        let keep = &[EventType::ConnectedIn, EventType::ConnectingOut, EventType::Disconnected];
        narrow_keep_only(&mut sel, keep);
        assert!(sel[EventType::ConnectedIn as usize]);
        assert!(sel[EventType::ConnectingOut as usize]);
        assert!(!sel[EventType::Disconnected as usize]); // was false, stays false
        assert!(!sel[EventType::Status as usize]); // not in keep set
    }

    #[test]
    fn narrow_remove_disables_specified() {
        use crate::core::events::EventType;
        let mut sel = all_selected();
        let remove = &[EventType::ConnectionRefused, EventType::ConnectInFailed, EventType::ConnectOutFailed];
        narrow_remove(&mut sel, remove);
        assert!(!sel[EventType::ConnectionRefused as usize]);
        assert!(!sel[EventType::ConnectInFailed as usize]);
        assert!(!sel[EventType::ConnectOutFailed as usize]);
        assert!(sel[EventType::ConnectedIn as usize]); // not in remove set
    }

    #[test]
    fn narrow_local_removes_all_directed() {
        use crate::core::{OUTBOUND_EVENTS, INBOUND_EVENTS, BIDIR_EVENTS};
        use crate::core::events::EventType;
        let mut sel = all_selected();
        narrow_remove(&mut sel, OUTBOUND_EVENTS);
        narrow_remove(&mut sel, INBOUND_EVENTS);
        narrow_remove(&mut sel, BIDIR_EVENTS);
        // All directed events should be off
        for &et in OUTBOUND_EVENTS { assert!(!sel[et as u8 as usize]); }
        for &et in INBOUND_EVENTS { assert!(!sel[et as u8 as usize]); }
        for &et in BIDIR_EVENTS { assert!(!sel[et as u8 as usize]); }
        // Non-directed events should still be on
        assert!(sel[EventType::Status as usize]);
        assert!(sel[EventType::Authoring as usize]);
        assert!(sel[EventType::GuaranteeBuilt as usize]);
    }

    #[test]
    fn is_errors_only_false_for_category_without_errors() {
        let sel = all_selected();
        let fake_types: &[EventType] = &[
            EventType::Status, EventType::BestBlockChanged,
            EventType::FinalizedBlockChanged, EventType::SyncStatusChanged,
        ];
        let has_errors = fake_types.iter().any(|et| ERROR_EVENT_TYPES.contains(et));
        if !has_errors {
            assert!(!is_errors_only(&sel, fake_types));
        }
    }
}
