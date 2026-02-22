//! Shared JAM Visualization App
//!
//! This module contains the egui app that runs on both native and WASM platforms.

mod header;
mod filter;
mod ring;
mod graphs;
mod settings;
mod diagnostics;

use eframe::egui;
use tracing::info;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::collections::VecDeque;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use crate::core::{
    parse_event, ParseResult, ParserContext, BestBlockData, EventStore, TimeSeriesData,
    EventType, EVENT_CATEGORIES,
};
use crate::theme::{colors, minimal_visuals};
use crate::time::now_seconds;
use crate::vring::{DirectedEventBuffer, PulseEvent, ColorLut, ColorSchema};
use crate::ws_state::WsState;

#[cfg(target_arch = "wasm32")]
use crate::websocket_wasm::WsClient;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use crate::scatter::ScatterRenderer;
use crate::vring::RingRenderer;

#[cfg(not(target_arch = "wasm32"))]
use crate::websocket_native::NativeWsClient;
#[cfg(not(target_arch = "wasm32"))]
use parking_lot::Mutex;

/// Default WebSocket URL for jamtart (override with JAMTART_WS env var)
pub const DEFAULT_WS_URL: &str = "ws://127.0.0.1:38080/api/ws";

/// Active tab in the visualization
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    Ring,
    Graphs,
}

/// An active collapsing-pulse animation on the ring.
pub(crate) struct CollapsingPulse {
    pub node_index: u16,
    pub event_type: EventType,
    pub birth_time: f32,
}

/// Shared state that can be updated from WebSocket callbacks
pub struct SharedData {
    pub time_series: TimeSeriesData,
    pub blocks: BestBlockData,
    pub events: EventStore,
    pub directed_buffer: DirectedEventBuffer,
    pub pulse_events: Vec<PulseEvent>,
}

/// JAM Visualization App - runs on both native and WASM
pub struct JamApp {
    /// Shared data (platform-specific wrapper)
    #[cfg(target_arch = "wasm32")]
    pub(crate) data: Rc<RefCell<SharedData>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) data: SharedData,

    /// WebSocket state (platform-specific wrapper)
    #[cfg(target_arch = "wasm32")]
    ws_state: Rc<RefCell<WsState>>,
    #[cfg(not(target_arch = "wasm32"))]
    ws_state: Arc<Mutex<WsState>>,

    /// WebSocket client (kept alive)
    #[cfg(target_arch = "wasm32")]
    #[allow(dead_code)]
    ws_client: Option<WsClient>,
    #[cfg(not(target_arch = "wasm32"))]
    ws_client: Option<NativeWsClient>,

    /// FPS counter
    pub(crate) fps_counter: header::FpsCounter,
    /// Event filter: [event_type] = enabled
    pub(crate) selected_events: Vec<bool>,
    /// Toggle event selector panel visibility
    pub(crate) show_event_selector: bool,
    /// Currently selected category index in the filter panel
    pub(crate) expanded_category: Option<usize>,
    /// Currently active tab
    pub(crate) active_tab: ActiveTab,
    /// Use CPU rendering (--use-cpu on native, fallback if no wgpu on WASM)
    pub(crate) use_cpu: bool,
    /// Cursor for incremental GPU particle upload
    pub(crate) gpu_upload_cursor: u64,
    /// Off-screen texture for GPU scatter renderer (None in CPU mode)
    pub(crate) scatter_texture_id: Option<egui::TextureId>,
    /// Previous filter bitfield for change detection
    prev_filter_bitfield: [u64; 4],
    /// Previous color schema for change detection
    prev_color_schema: ColorSchema,
    /// Diagnostics: total events received (accumulated each tick)
    pub(crate) diag_events_total: u64,
    /// Diagnostics: events/sec (computed each tick)
    pub(crate) diag_events_sec: f64,
    /// Diagnostics: node-reported drops (sum of Event::Dropped.num)
    /// Diagnostics: jamtart-side drops (detected via id gaps)
    pub(crate) diag_server_dropped_total: u64,
    /// Diagnostics: total dropped/sec (both sources)
    pub(crate) diag_dropped_sec: f64,
    /// Internal: events since last 1-second tick
    diag_events_counter: u64,
    /// Internal: drops since last 1-second tick
    diag_dropped_counter: u64,
    /// Internal: timestamp of last 1-second tick
    diag_last_tick: f64,
    /// Internal: last seen data.id for gap detection
    diag_last_event_id: Option<u64>,
    /// Active collapsing-pulse animations on the ring
    pub(crate) active_pulses: Vec<CollapsingPulse>,
    /// Errors-only filter preset active
    pub(crate) errors_only: bool,
    /// Last known particle count (for header display)
    pub(crate) particle_count: usize,
    /// Last known particle capacity (for header display)
    pub(crate) particle_max: usize,
    /// Active color schema (selectable via header dropdown)
    pub(crate) color_schema: ColorSchema,
    /// Dynamic color lookup table (recomputed on filter/schema change)
    pub(crate) color_lut: ColorLut,
    /// Show settings sidebar
    pub(crate) show_settings: bool,
    /// Slot pulse animation enabled
    pub(crate) slot_pulse_enabled: bool,
    /// Node brightness by peer count enabled
    pub(crate) node_brightness_enabled: bool,
    /// Particle speed factor (0.1 = 10x slow, 1.0 = normal, 2.0 = 2x fast)
    pub(crate) speed_factor: f32,
    /// Buffered WebSocket messages for time-budgeted processing (WASM only)
    #[cfg(target_arch = "wasm32")]
    msg_buffer: Rc<RefCell<VecDeque<String>>>,
}

// Helper macro to access data on both platforms
macro_rules! with_data {
    ($self:expr, |$data:ident| $body:expr) => {{
        #[cfg(target_arch = "wasm32")]
        {
            let $data = $self.data.borrow();
            $body
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let $data = &$self.data;
            $body
        }
    }};
}
pub(crate) use with_data;

/// Load Overpass Mono as the single font for all UI text.
fn load_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "OverpassMono".into(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/OverpassMono.ttf")).into(),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "OverpassMono".into());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.insert(0, "OverpassMono".into());
    }
    ctx.set_fonts(fonts);
}

impl JamApp {
    /// Create new app for WASM platform
    #[cfg(target_arch = "wasm32")]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());
        load_custom_fonts(&cc.egui_ctx);

        // Register GPU renderers (wgpu backend on WASM via WebGPU)
        let (scatter_texture_id, use_cpu) =
            if let Some(render_state) = cc.wgpu_render_state.as_ref() {
                let device = &render_state.device;
                let format = render_state.target_format;

                let ring_renderer = RingRenderer::new(device, format);
                render_state
                    .renderer
                    .write()
                    .callback_resources
                    .insert(ring_renderer);

                let scatter_renderer = ScatterRenderer::new(device, format);
                let texture_id = {
                    let mut egui_renderer = render_state.renderer.write();
                    egui_renderer.register_native_texture(
                        device,
                        &scatter_renderer.create_view(),
                        egui_wgpu::wgpu::FilterMode::Linear,
                    )
                };
                render_state
                    .renderer
                    .write()
                    .callback_resources
                    .insert(scatter_renderer);

                (Some(texture_id), false)
            } else {
                (None, true) // fallback to CPU if wgpu unavailable
            };

        let data = Rc::new(RefCell::new(SharedData {
            time_series: TimeSeriesData::new(1024, 200),
            blocks: BestBlockData::new(1024),
            events: EventStore::new(50000, 60.0),
            directed_buffer: DirectedEventBuffer::default(),
            pulse_events: Vec::new(),
        }));

        let ws_state = Rc::new(RefCell::new(WsState::Connecting));
        let msg_buffer: Rc<RefCell<VecDeque<String>>> =
            Rc::new(RefCell::new(VecDeque::new()));

        // Connect WebSocket — messages buffered, drained in update()
        let ws_url = js_sys::eval("window.__jam_ws_url")
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| DEFAULT_WS_URL.to_string());
        let ws_client = WsClient::connect(
            &ws_url,
            msg_buffer.clone(),
            ws_state.clone(),
        )
        .ok();

        Self {
            data,
            ws_state,
            ws_client,
            fps_counter: header::FpsCounter::new(),
            selected_events: Self::default_selected_events(),
            show_event_selector: false,
            expanded_category: None,
            active_tab: ActiveTab::default(),
            use_cpu,
            gpu_upload_cursor: 0,
            scatter_texture_id,
            prev_filter_bitfield: [u64::MAX; 4],
            prev_color_schema: ColorSchema::default(),
            diag_events_total: 0,
            diag_events_sec: 0.0,
            diag_server_dropped_total: 0,
            diag_dropped_sec: 0.0,
            diag_events_counter: 0,
            diag_dropped_counter: 0,
            diag_last_tick: 0.0,
            diag_last_event_id: None,
            active_pulses: Vec::new(),
            errors_only: false,
            particle_count: 0,
            particle_max: 0,
            color_schema: ColorSchema::default(),
            color_lut: build_color_lut(&Self::default_selected_events(), ColorSchema::default()),
            show_settings: false,
            slot_pulse_enabled: true,
            node_brightness_enabled: true,
            speed_factor: 1.0,
            msg_buffer,
        }
    }

    /// Create new app for native platform
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(cc: &eframe::CreationContext<'_>, use_cpu: bool) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());
        load_custom_fonts(&cc.egui_ctx);
        let mut style = (*cc.egui_ctx.style()).clone();
        for (_text_style, font_id) in style.text_styles.iter_mut() {
            font_id.size *= 1.5;
        }
        cc.egui_ctx.set_style(style);

        // Register GPU renderers unless CPU mode requested
        let scatter_texture_id = if !use_cpu {
            if let Some(render_state) = cc.wgpu_render_state.as_ref() {
                let device = &render_state.device;
                let format = render_state.target_format;

                // Ring renderer
                let ring_renderer = RingRenderer::new(device, format);
                render_state
                    .renderer
                    .write()
                    .callback_resources
                    .insert(ring_renderer);

                // Scatter renderer (off-screen texture)
                let scatter_renderer = ScatterRenderer::new(device, format);
                let texture_id = {
                    let mut egui_renderer = render_state.renderer.write();
                    egui_renderer.register_native_texture(
                        device,
                        &scatter_renderer.create_view(),
                        egui_wgpu::wgpu::FilterMode::Linear,
                    )
                };
                render_state
                    .renderer
                    .write()
                    .callback_resources
                    .insert(scatter_renderer);

                Some(texture_id)
            } else {
                None
            }
        } else {
            None
        };

        let data = SharedData {
            time_series: TimeSeriesData::new(1024, 200),
            blocks: BestBlockData::new(1024),
            events: EventStore::new(50000, 60.0),
            directed_buffer: DirectedEventBuffer::default(),
            pulse_events: Vec::new(),
        };

        let ws_url = std::env::var("JAMTART_WS").unwrap_or_else(|_| DEFAULT_WS_URL.to_string());
        info!(url = %ws_url, env_set = std::env::var("JAMTART_WS").is_ok(), "WebSocket URL resolved");
        let ws_client = NativeWsClient::connect(&ws_url);
        let ws_state = ws_client.state.clone();

        Self {
            data,
            ws_state,
            ws_client: Some(ws_client),
            fps_counter: header::FpsCounter::new(),
            selected_events: Self::default_selected_events(),
            show_event_selector: false,
            expanded_category: None,
            active_tab: ActiveTab::default(),
            use_cpu,
            gpu_upload_cursor: 0,
            scatter_texture_id,
            prev_filter_bitfield: [u64::MAX; 4],
            prev_color_schema: ColorSchema::default(),
            diag_events_total: 0,
            diag_events_sec: 0.0,
            diag_server_dropped_total: 0,
            diag_dropped_sec: 0.0,
            diag_events_counter: 0,
            diag_dropped_counter: 0,
            diag_last_tick: 0.0,
            diag_last_event_id: None,
            active_pulses: Vec::new(),
            errors_only: false,
            particle_count: 0,
            particle_max: 0,
            color_schema: ColorSchema::default(),
            color_lut: build_color_lut(&Self::default_selected_events(), ColorSchema::default()),
            show_settings: false,
            slot_pulse_enabled: true,
            node_brightness_enabled: true,
            speed_factor: 1.0,
        }
    }

    pub(crate) fn default_selected_events() -> Vec<bool> {
        // Enable all events by default
        vec![true; 200]
    }

    /// Build a [u64; 4] bitfield from selected_events for DirectedEventBuffer filtering
    pub(crate) fn build_filter_bitfield(&self) -> [u64; 4] {
        let mut bitfield = [0u64; 4];
        for (i, &enabled) in self.selected_events.iter().enumerate() {
            if enabled {
                let idx = i / 64;
                let bit = i % 64;
                bitfield[idx] |= 1 << bit;
            }
        }
        bitfield
    }

    /// Apply errors-only filter preset: enable only error/failure events.
    pub(crate) fn apply_errors_filter(&mut self) {
        use crate::core::events::ERROR_EVENT_TYPES;
        self.selected_events.fill(false);
        for &et in ERROR_EVENT_TYPES {
            self.selected_events[et.idx()] = true;
        }
        self.errors_only = true;
    }

    /// Restore all events and clear errors-only mode.
    pub(crate) fn apply_all_filter(&mut self) {
        self.selected_events.fill(true);
        self.errors_only = false;
    }

    /// Track parse result for diagnostics (jamtart-side gap detection only)
    fn track_parse_result(&mut self, result: &ParseResult) {
        self.diag_events_counter += 1;
        // Server-side gap detection via data.id
        if let Some(id) = result.event_id {
            if let Some(last_id) = self.diag_last_event_id {
                let gap = id.saturating_sub(last_id).saturating_sub(1);
                if gap > 0 {
                    self.diag_dropped_counter += gap;
                    self.diag_server_dropped_total += gap;
                }
            }
            self.diag_last_event_id = Some(id);
        }
    }

    /// Process incoming WebSocket messages (native)
    #[cfg(not(target_arch = "wasm32"))]
    fn process_messages(&mut self) {
        // Time-budget message processing: yield after ~12ms to maintain 60fps.
        // Remaining messages stay in the channel for the next frame.
        use std::time::{Duration, Instant};
        const BUDGET: Duration = Duration::from_millis(12);
        let deadline = Instant::now() + BUDGET;
        let mut results = Vec::new();
        if let Some(ref client) = self.ws_client {
            while let Ok(msg) = client.rx.try_recv() {
                let now = now_seconds();
                let d = &mut self.data;
                let mut ctx = ParserContext {
                    time_series: &mut d.time_series,
                    blocks: &mut d.blocks,
                    events: &mut d.events,
                    directed_buffer: &mut d.directed_buffer,
                    pulse_events: &mut d.pulse_events,
                };
                if let Some(result) = parse_event(&msg, &mut ctx, now) {
                    results.push(result);
                }
                if Instant::now() >= deadline {
                    break;
                }
            }
        }
        for result in &results {
            self.track_parse_result(result);
        }
    }

    /// Process buffered WebSocket messages (WASM)
    #[cfg(target_arch = "wasm32")]
    fn process_messages(&mut self) {
        const BUDGET_MS: f64 = 12.0;
        let deadline = js_sys::Date::now() + BUDGET_MS;
        let mut results = Vec::new();
        {
            let mut buf = self.msg_buffer.borrow_mut();
            let mut data = self.data.borrow_mut();
            let d = &mut *data;
            while let Some(msg) = buf.pop_front() {
                let now = now_seconds();
                let mut ctx = ParserContext {
                    time_series: &mut d.time_series,
                    blocks: &mut d.blocks,
                    events: &mut d.events,
                    directed_buffer: &mut d.directed_buffer,
                    pulse_events: &mut d.pulse_events,
                };
                if let Some(result) = parse_event(&msg, &mut ctx, now) {
                    results.push(result);
                }
                if js_sys::Date::now() >= deadline {
                    break;
                }
            }
        }
        for result in &results {
            self.track_parse_result(result);
        }
    }

    /// Get the current WebSocket state
    pub(crate) fn get_ws_state(&self) -> WsState {
        #[cfg(target_arch = "wasm32")]
        {
            self.ws_state.borrow().clone()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.ws_state.lock().clone()
        }
    }

    /// Get color for event type from the dynamic ColorLut
    pub(crate) fn get_event_color(&self, event_type: EventType) -> egui::Color32 {
        let [r, g, b, a] = self.color_lut.colors[event_type.idx()];
        egui::Color32::from_rgba_unmultiplied(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        )
    }

    /// Draw event category color legend as an auto-sized egui Window.
    /// Single-category mode: shows individual event names with distinct colors.
    /// Multi-category mode: shows category names with shared category colors.
    pub(crate) fn draw_legend(&self, ctx: &egui::Context) {
        use crate::core::event_name;

        // Determine if single-category mode
        let active_categories: Vec<usize> = EVENT_CATEGORIES.iter().enumerate()
            .filter(|(_, cat)| cat.event_types.iter().any(|&et|
                et.idx() < self.selected_events.len() && self.selected_events[et.idx()]
            ))
            .map(|(i, _)| i)
            .collect();

        let single_category = if active_categories.len() == 1 {
            Some(active_categories[0])
        } else {
            None
        };

        // Build legend entries: (name, color, enabled)
        let entries: Vec<(&str, egui::Color32, bool)> = if let Some(cat_idx) = single_category {
            let category = &EVENT_CATEGORIES[cat_idx];
            category.event_types.iter().map(|&et| {
                let enabled = et.idx() < self.selected_events.len()
                    && self.selected_events[et.idx()];
                (event_name(et), self.get_event_color(et), enabled)
            }).collect()
        } else {
            EVENT_CATEGORIES.iter().map(|cat| {
                let enabled = cat.event_types.iter().any(|&et|
                    et.idx() < self.selected_events.len() && self.selected_events[et.idx()]
                );
                (cat.name, self.get_event_color(cat.event_types[0]), enabled)
            }).collect()
        };

        egui::Area::new(egui::Id::new("legend_area"))
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(8.0, -8.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 200))
                    .corner_radius(4.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        let header = egui::CollapsingHeader::new(
                            egui::RichText::new("Legend").color(colors::TEXT_MUTED),
                        )
                        .default_open(true);

                        header.show(ui, |ui| {
                            for (name, color, enabled) in &entries {
                                let alpha = if *enabled { 200u8 } else { 40 };
                                let swatch_color = egui::Color32::from_rgba_unmultiplied(
                                    color.r(), color.g(), color.b(), alpha,
                                );
                                let text_color = egui::Color32::from_rgba_unmultiplied(160, 160, 160, alpha);

                                ui.horizontal(|ui| {
                                    let (dot_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, 10.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(dot_rect.center(), 5.0, swatch_color);
                                    ui.label(egui::RichText::new(*name).color(text_color));
                                });
                            }
                        });
                    });
            });
    }

}

/// Build a ColorLut based on current filter state and color schema.
/// Single-category mode (only one category has any enabled events): distinct colors per event type.
/// Multi-category mode: shared category color for all events in a category.
fn build_color_lut(selected_events: &[bool], schema: ColorSchema) -> ColorLut {
    let active_categories: Vec<usize> = EVENT_CATEGORIES.iter().enumerate()
        .filter(|(_, cat)| cat.event_types.iter().any(|&et|
            et.idx() < selected_events.len() && selected_events[et.idx()]
        ))
        .map(|(i, _)| i)
        .collect();

    let single_category = if active_categories.len() == 1 {
        Some(active_categories[0])
    } else {
        None
    };

    let mut lut = ColorLut { colors: [[0.0; 4]; 256] };
    let category_colors = schema.colors();

    if let Some(cat_idx) = single_category {
        // Single category: assign distinct palette colors to each enabled event
        let category = &EVENT_CATEGORIES[cat_idx];
        let enabled: Vec<EventType> = category.event_types.iter()
            .copied()
            .filter(|&et| et.idx() < selected_events.len() && selected_events[et.idx()])
            .collect();
        let palette = schema.generate_distinct_palette(enabled.len());
        for (i, &et) in enabled.iter().enumerate() {
            lut.colors[et.idx()] = palette[i];
        }
    } else {
        // Multi-category: each event gets its category color from the active schema
        for (cat_idx, category) in EVENT_CATEGORIES.iter().enumerate() {
            let color = category_colors[cat_idx];
            for &et in category.event_types {
                lut.colors[et.idx()] = color;
            }
        }
    }

    lut
}

impl eframe::App for JamApp {
    #[allow(unused_variables)]
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Request continuous repaint for real-time updates
        ctx.request_repaint();

        // Process WebSocket messages (time-budgeted on both platforms)
        self.process_messages();

        // Periodic diagnostics tick (~1s) — cross-platform
        let now = now_seconds();
        if now - self.diag_last_tick >= 1.0 {
            let elapsed = now - self.diag_last_tick;
            self.diag_events_sec = self.diag_events_counter as f64 / elapsed;
            self.diag_dropped_sec = self.diag_dropped_counter as f64 / elapsed;
            self.diag_events_total += self.diag_events_counter;

            #[cfg(not(target_arch = "wasm32"))]
            {
                let active = self.data.directed_buffer.active_count(now as f32, 5.0);
                let nodes = self.data.events.node_count();
                info!(
                    events_per_sec = self.diag_events_counter,
                    dropped_per_sec = self.diag_dropped_counter,
                    active_particles = active,
                    nodes,
                    "stats"
                );
            }

            self.diag_events_counter = 0;
            self.diag_dropped_counter = 0;
            self.diag_last_tick = now;
        }

        // Prune old events periodically
        #[cfg(target_arch = "wasm32")]
        self.data.borrow_mut().events.prune(now);
        #[cfg(not(target_arch = "wasm32"))]
        self.data.events.prune(now);

        // Sync event filter to directed buffer for ring visualization
        let filter = self.build_filter_bitfield();
        let schema_changed = self.color_schema != self.prev_color_schema;
        if filter != self.prev_filter_bitfield || schema_changed {
            if !schema_changed {
                let enabled = filter.iter().map(|w| w.count_ones()).sum::<u32>();
                info!(
                    enabled_types = enabled,
                    bitfield = ?filter,
                    "filter changed"
                );
            }
            self.prev_filter_bitfield = filter;
            self.prev_color_schema = self.color_schema;
            self.color_lut = build_color_lut(&self.selected_events, self.color_schema);
        }
        #[cfg(target_arch = "wasm32")]
        self.data.borrow_mut().directed_buffer.set_enabled_types(filter);
        #[cfg(not(target_arch = "wasm32"))]
        self.data.directed_buffer.set_enabled_types(filter);

        // Drain pending pulse events into active pulses
        {
            #[cfg(target_arch = "wasm32")]
            let pulses: Vec<PulseEvent> = self.data.borrow_mut().pulse_events.drain(..).collect();
            #[cfg(not(target_arch = "wasm32"))]
            let pulses: Vec<PulseEvent> = self.data.pulse_events.drain(..).collect();
            for pe in pulses {
                self.active_pulses.push(CollapsingPulse {
                    node_index: pe.node_index,
                    event_type: pe.event_type,
                    birth_time: pe.birth_time,
                });
            }
        }
        // Expire old pulses
        const PULSE_DURATION: f32 = 0.4;
        let now_f32 = now as f32;
        self.active_pulses.retain(|p| now_f32 - p.birth_time < PULSE_DURATION);

        // Header bar (TopBottomPanel spans full width, stays in place regardless of sidebar)
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY).inner_margin(4.0))
            .show(ctx, |ui| {
                self.render_header(ui);
            });

        // Filter sidebar (must be shown before CentralPanel)
        if self.show_event_selector {
            self.render_event_selector(ctx);
        }

        // Settings sidebar (left, must be shown before CentralPanel)
        if self.show_settings {
            self.render_settings(ctx);
        }

        // Legend window (collapsible, anchored bottom-left, hidden when filter sidebar open)
        if !self.show_event_selector {
            self.draw_legend(ctx);
        }

        // Diagnostics window (collapsible, anchored top-right)
        self.draw_diagnostics(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY))
            .show(ctx, |ui| {

                match self.active_tab {
                    ActiveTab::Ring => self.render_ring_tab(ui),
                    ActiveTab::Graphs => self.render_graphs_tab(ui),
                }
            });

        // Update scatter texture reference after callback has rendered
        if let Some(texture_id) = self.scatter_texture_id {
            if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                let mut egui_renderer = wgpu_render_state.renderer.write();
                if let Some(scatter_renderer) =
                    egui_renderer.callback_resources.get::<ScatterRenderer>()
                {
                    let texture_view = scatter_renderer.create_view();
                    egui_renderer.update_egui_texture_from_wgpu_texture(
                        &wgpu_render_state.device,
                        &texture_view,
                        egui_wgpu::wgpu::FilterMode::Linear,
                        texture_id,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_selected() -> Vec<bool> {
        vec![true; 200]
    }

    fn none_selected() -> Vec<bool> {
        vec![false; 200]
    }

    #[test]
    fn build_color_lut_multi_category_same_color_per_category() {
        let sel = all_selected();
        let lut = build_color_lut(&sel, ColorSchema::Vivid);
        // All Connection events (20..=28) should share the same color
        let color_20 = lut.colors[20];
        for et in 21..=28usize {
            assert_eq!(lut.colors[et], color_20,
                "Connection event {} should match event 20", et);
        }
    }

    #[test]
    fn build_color_lut_single_category_distinct_colors() {
        // Enable only Connection events (20..=28)
        let mut sel = none_selected();
        for et in 20..=28usize {
            sel[et] = true;
        }
        let lut = build_color_lut(&sel, ColorSchema::Vivid);
        // Each Connection event should have a distinct color
        let colors: Vec<[f32; 4]> = (20..=28usize).map(|et| lut.colors[et]).collect();
        for i in 0..colors.len() {
            assert_ne!(colors[i], [0.0; 4], "Event {} should have non-zero color", 20 + i);
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j],
                    "Events {} and {} should have distinct colors", 20 + i, 20 + j);
            }
        }
    }

    #[test]
    fn build_color_lut_single_category_unselected_are_zero() {
        let mut sel = none_selected();
        for et in 20..=28usize {
            sel[et] = true;
        }
        let lut = build_color_lut(&sel, ColorSchema::Vivid);
        // Status event 10 should be zero (not in selected category)
        assert_eq!(lut.colors[10], [0.0; 4]);
        // Meta event 0 should be zero
        assert_eq!(lut.colors[0], [0.0; 4]);
    }

    #[test]
    fn build_color_lut_no_events_selected() {
        let sel = none_selected();
        let lut = build_color_lut(&sel, ColorSchema::Vivid);
        // Multi-category mode (0 active categories) — all should use category colors
        let vivid = ColorSchema::Vivid.colors();
        for (cat_idx, category) in EVENT_CATEGORIES.iter().enumerate() {
            let expected = vivid[cat_idx];
            for &et in category.event_types {
                assert_eq!(lut.colors[et.idx()], expected);
            }
        }
    }

    #[test]
    fn build_color_lut_schema_changes_colors() {
        let sel = all_selected();
        let vivid_lut = build_color_lut(&sel, ColorSchema::Vivid);
        let accessible_lut = build_color_lut(&sel, ColorSchema::Accessible);
        // Work Package event should have different colors in different schemas
        let wp = EventType::WorkPackageSubmission.idx();
        assert_ne!(vivid_lut.colors[wp], accessible_lut.colors[wp],
            "Different schemas should produce different colors for WorkPackageSubmission");
    }

    #[test]
    fn generate_distinct_palette_correct_count() {
        for schema in ColorSchema::ALL {
            assert_eq!(schema.generate_distinct_palette(0).len(), 0);
            assert_eq!(schema.generate_distinct_palette(1).len(), 1);
            assert_eq!(schema.generate_distinct_palette(5).len(), 5);
            assert_eq!(schema.generate_distinct_palette(20).len(), 20);
        }
    }

    #[test]
    fn generate_distinct_palette_non_zero_alpha() {
        for schema in ColorSchema::ALL {
            let palette = schema.generate_distinct_palette(10);
            for (i, color) in palette.iter().enumerate() {
                assert!(color[3] > 0.0, "{}: Color {} should have non-zero alpha", schema, i);
            }
        }
    }

    #[test]
    fn generate_distinct_palette_all_distinct() {
        for schema in ColorSchema::ALL {
            let palette = schema.generate_distinct_palette(12);
            for i in 0..palette.len() {
                for j in (i + 1)..palette.len() {
                    assert_ne!(palette[i], palette[j],
                        "{}: Colors {} and {} should be distinct", schema, i, j);
                }
            }
        }
    }
}
