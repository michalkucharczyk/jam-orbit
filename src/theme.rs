//! Minimal black & white theme inspired by polkadot.com

use egui::Color32;

/// Minimal black & white palette
/// No colors - only black, white, and greys
pub mod colors {
    use super::Color32;

    // === Backgrounds (black to dark grey) ===
    pub const BG_PRIMARY: Color32 = Color32::from_rgb(0, 0, 0);           // #000000 - pure black
    pub const BG_ELEVATED: Color32 = Color32::from_rgb(12, 12, 12);       // #0C0C0C - subtle elevation
    pub const BG_HOVER: Color32 = Color32::from_rgb(24, 24, 24);          // #181818 - hover states

    // === Text (white to grey) ===
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(255, 255, 255);   // #FFFFFF - primary text
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(160, 160, 160); // #A0A0A0 - secondary
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(80, 80, 80);        // #505050 - muted/disabled

    // === Lines & Borders ===
    pub const BORDER: Color32 = Color32::from_rgb(40, 40, 40);            // #282828 - subtle borders

    // === Data Lines ===
    // Pure white lines with low alpha for the "overlap = solid" effect
    pub const LINE_ALPHA: u8 = 1;  // 0.5% opacity (1/255 ≈ 0.004, closest to 0.5%)
}

/// Create minimal black & white egui Visuals
pub fn minimal_visuals() -> egui::Visuals {
    use colors::*;

    let mut visuals = egui::Visuals::dark();

    // Pure black backgrounds
    visuals.panel_fill = BG_PRIMARY;
    visuals.window_fill = BG_PRIMARY;
    visuals.extreme_bg_color = BG_PRIMARY;
    visuals.faint_bg_color = BG_ELEVATED;

    // White text
    visuals.override_text_color = Some(TEXT_PRIMARY);

    // Minimal widget styling - all greyscale
    visuals.widgets.noninteractive.bg_fill = BG_PRIMARY;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_MUTED);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);

    visuals.widgets.inactive.bg_fill = BG_PRIMARY;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.weak_bg_fill = BG_PRIMARY;

    visuals.widgets.hovered.bg_fill = BG_ELEVATED;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, TEXT_MUTED);
    visuals.widgets.hovered.weak_bg_fill = BG_ELEVATED;

    visuals.widgets.active.bg_fill = BG_HOVER;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.active.weak_bg_fill = BG_HOVER;

    // Selection - white on dark
    visuals.selection.bg_fill = Color32::from_rgb(60, 60, 60);
    visuals.selection.stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);

    // Hyperlinks - white, underlined
    visuals.hyperlink_color = TEXT_PRIMARY;

    // No shadows - flat design
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;

    visuals
}
