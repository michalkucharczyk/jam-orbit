//! Shared JAM Visualization App
//!
//! This module contains the egui app that runs on both native and WASM platforms.

mod header;
mod filter;
mod ring;
mod pipeline;
mod network;
mod consensus;
mod errors;

use eframe::egui;
use tracing::info;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use crate::core::{
    parse_event, ParserContext, BestBlockData, EventStore, TimeSeriesData,
    EVENT_CATEGORIES, GuaranteeQueueData, SyncStatusData, ShardMetrics,
};
use crate::theme::{colors, minimal_visuals};
use crate::time::now_seconds;
use crate::vring::{DirectedEventBuffer, PulseEvent};
use crate::ws_state::WsState;

#[cfg(target_arch = "wasm32")]
use crate::websocket_wasm::WsClient;

#[cfg(not(target_arch = "wasm32"))]
use crate::websocket_native::NativeWsClient;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use parking_lot::Mutex;

#[cfg(not(target_arch = "wasm32"))]
use crate::scatter::ScatterRenderer;
#[cfg(not(target_arch = "wasm32"))]
use crate::vring::RingRenderer;

/// Default WebSocket URL for jamtart (override with JAMTART_WS env var)
pub const DEFAULT_WS_URL: &str = "ws://127.0.0.1:38080/api/ws";

/// Active tab in the visualization
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    Ring,
    Pipeline,
    Network,
    Consensus,
    Errors,
}

/// An active collapsing-pulse animation on the ring.
pub(crate) struct CollapsingPulse {
    pub node_index: u16,
    pub event_type: u8,
    pub birth_time: f32,
}

/// Shared state that can be updated from WebSocket callbacks
pub struct SharedData {
    pub time_series: TimeSeriesData,
    pub blocks: BestBlockData,
    pub events: EventStore,
    pub directed_buffer: DirectedEventBuffer,
    pub pulse_events: Vec<PulseEvent>,
    pub guarantee_queue: GuaranteeQueueData,
    pub sync_status: SyncStatusData,
    pub shard_metrics: ShardMetrics,
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
    pub(crate) selected_category: usize,
    /// Toggle legend overlay visibility
    pub(crate) show_legend: bool,
    /// Currently active tab
    pub(crate) active_tab: ActiveTab,
    /// Use CPU rendering for all visualizations (--use-cpu flag, native only)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) use_cpu: bool,
    /// Cursor for incremental GPU particle upload
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) gpu_upload_cursor: u64,
    /// Off-screen texture for GPU scatter renderer (None in CPU mode)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) scatter_texture_id: Option<egui::TextureId>,
    /// Previous filter bitfield for change detection
    prev_filter_bitfield: [u64; 4],
    /// Stats: messages received since last log
    #[cfg(not(target_arch = "wasm32"))]
    stats_msg_count: u64,
    /// Stats: particles uploaded since last log
    #[cfg(not(target_arch = "wasm32"))]
    stats_uploaded: u64,
    /// Stats: last log timestamp
    #[cfg(not(target_arch = "wasm32"))]
    stats_last_log: f64,
    /// Active collapsing-pulse animations on the ring
    pub(crate) active_pulses: Vec<CollapsingPulse>,
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

impl JamApp {
    /// Create new app for WASM platform
    #[cfg(target_arch = "wasm32")]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());
        let mut style = (*cc.egui_ctx.style()).clone();
        for (_text_style, font_id) in style.text_styles.iter_mut() {
            font_id.size *= 1.5;
        }
        cc.egui_ctx.set_style(style);

        let data = Rc::new(RefCell::new(SharedData {
            time_series: TimeSeriesData::new(1024, 200),
            blocks: BestBlockData::new(1024),
            events: EventStore::new(50000, 60.0),
            directed_buffer: DirectedEventBuffer::default(),
            pulse_events: Vec::new(),
            guarantee_queue: GuaranteeQueueData::new(1024),
            sync_status: SyncStatusData::new(1024),
            shard_metrics: ShardMetrics::new(1024, 200),
        }));

        let ws_state = Rc::new(RefCell::new(WsState::Connecting));

        // Connect WebSocket with callback that updates shared data
        let data_clone = data.clone();
        let ws_url = js_sys::eval("new URLSearchParams(window.location.search).get('ws_url')")
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| DEFAULT_WS_URL.to_string());
        let ws_client = WsClient::connect(
            &ws_url,
            move |msg| {
                let now = now_seconds();
                let mut data = data_clone.borrow_mut();
                let d = &mut *data;
                let mut ctx = ParserContext {
                    time_series: &mut d.time_series,
                    blocks: &mut d.blocks,
                    events: &mut d.events,
                    directed_buffer: &mut d.directed_buffer,
                    pulse_events: &mut d.pulse_events,
                    guarantee_queue: &mut d.guarantee_queue,
                    sync_status: &mut d.sync_status,
                    shard_metrics: &mut d.shard_metrics,
                };
                parse_event(&msg, &mut ctx, now);
            },
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
            selected_category: 0,
            show_legend: true,
            active_tab: ActiveTab::default(),
            prev_filter_bitfield: [u64::MAX; 4],
            active_pulses: Vec::new(),
        }
    }

    /// Create new app for native platform
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(cc: &eframe::CreationContext<'_>, use_cpu: bool) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());
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
            guarantee_queue: GuaranteeQueueData::new(1024),
            sync_status: SyncStatusData::new(1024),
            shard_metrics: ShardMetrics::new(1024, 200),
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
            selected_category: 0,
            show_legend: true,
            active_tab: ActiveTab::default(),
            use_cpu,
            gpu_upload_cursor: 0,
            scatter_texture_id,
            prev_filter_bitfield: [u64::MAX; 4],
            stats_msg_count: 0,
            stats_uploaded: 0,
            stats_last_log: 0.0,
            active_pulses: Vec::new(),
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

    /// Process incoming WebSocket messages (native only)
    #[cfg(not(target_arch = "wasm32"))]
    fn process_messages(&mut self) {
        // Time-budget message processing: yield after ~12ms to maintain 60fps.
        // Remaining messages stay in the channel for the next frame.
        use std::time::{Duration, Instant};
        const BUDGET: Duration = Duration::from_millis(12);
        let deadline = Instant::now() + BUDGET;
        if let Some(ref client) = self.ws_client {
            while let Ok(msg) = client.rx.try_recv() {
                self.stats_msg_count += 1;
                let now = now_seconds();
                let d = &mut self.data;
                let mut ctx = ParserContext {
                    time_series: &mut d.time_series,
                    blocks: &mut d.blocks,
                    events: &mut d.events,
                    directed_buffer: &mut d.directed_buffer,
                    pulse_events: &mut d.pulse_events,
                    guarantee_queue: &mut d.guarantee_queue,
                    sync_status: &mut d.sync_status,
                    shard_metrics: &mut d.shard_metrics,
                };
                parse_event(&msg, &mut ctx, now);
                if Instant::now() >= deadline {
                    break;
                }
            }
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

    /// Get color for event type
    pub(crate) fn get_event_color(&self, event_type: u8) -> egui::Color32 {
        match event_type {
            0 => egui::Color32::from_rgb(128, 128, 128),       // Meta - gray
            10..=13 => egui::Color32::from_rgb(100, 200, 100), // Status - green
            20..=28 => egui::Color32::from_rgb(100, 150, 255), // Connection - blue
            40..=47 => egui::Color32::from_rgb(255, 200, 100), // Block auth - orange
            60..=68 => egui::Color32::from_rgb(200, 100, 255), // Block dist - purple
            80..=84 => egui::Color32::from_rgb(255, 100, 100), // Tickets - red
            90..=104 => egui::Color32::from_rgb(100, 255, 200), // Work Package - cyan
            105..=113 => egui::Color32::from_rgb(255, 100, 200), // Guaranteeing - magenta
            120..=131 => egui::Color32::from_rgb(255, 255, 100), // Availability - yellow
            140..=153 => egui::Color32::from_rgb(255, 150, 150), // Bundle - pink
            160..=178 => egui::Color32::from_rgb(150, 200, 255), // Segment - light blue
            190..=199 => egui::Color32::from_rgb(200, 200, 200), // Preimage - light gray
            _ => egui::Color32::from_rgb(255, 255, 255),
        }
    }

    /// Draw event category color legend overlay
    pub(crate) fn draw_legend(&self, painter: &egui::Painter, rect: egui::Rect) {
        let swatch_size = 8.0;
        let row_height = 14.0;
        let padding = 6.0;
        let font = egui::FontId::monospace(10.0);
        let num_rows = EVENT_CATEGORIES.len() as f32;
        let legend_width = 150.0;
        let legend_height = num_rows * row_height + padding * 2.0;

        // Position: bottom-left with margin
        let legend_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 8.0, rect.bottom() - legend_height - 8.0),
            egui::vec2(legend_width, legend_height),
        );

        // Dark semi-transparent background
        painter.rect_filled(
            legend_rect,
            4.0,
            egui::Color32::from_rgba_unmultiplied(20, 20, 20, 200),
        );

        let legend_x = legend_rect.left() + padding;
        let mut legend_y = legend_rect.top() + padding;

        for category in EVENT_CATEGORIES {
            let color = self.get_event_color(category.event_types[0]);
            let enabled = category
                .event_types
                .iter()
                .any(|&et| (et as usize) < self.selected_events.len() && self.selected_events[et as usize]);

            let alpha = if enabled { 200u8 } else { 40 };
            let swatch_color = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
            let text_color = egui::Color32::from_rgba_unmultiplied(160, 160, 160, alpha);

            painter.circle_filled(
                egui::pos2(legend_x + swatch_size * 0.5, legend_y + swatch_size * 0.5),
                swatch_size * 0.5,
                swatch_color,
            );
            painter.text(
                egui::pos2(legend_x + swatch_size + 6.0, legend_y),
                egui::Align2::LEFT_TOP,
                category.name,
                font.clone(),
                text_color,
            );

            legend_y += row_height;
        }
    }

    /// Render a rate graph for specific event types (reusable helper).
    /// Returns per-node rates for the given event types.
    pub(crate) fn compute_filtered_rates(&self, event_types: &[u8]) -> Vec<(u16, Vec<u32>)> {
        let now = now_seconds();
        let mut filter = vec![false; 200];
        for &et in event_types {
            if (et as usize) < filter.len() {
                filter[et as usize] = true;
            }
        }
        with_data!(self, |data| {
            data.events.compute_rates_per_node(now, 1.0, 60, &filter)
        })
    }

    /// Paint a title overlay at the top-left inside a plot rect
    pub(crate) fn paint_plot_title(ui: &egui::Ui, rect: egui::Rect, title: &str, color: egui::Color32) {
        ui.painter().text(
            rect.left_top() + egui::vec2(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(13.0),
            color,
        );
    }

    /// Render a standard per-node rate plot (transparent white lines)
    pub(crate) fn render_rate_plot(&self, ui: &mut egui::Ui, id: &str, title: &str, event_types: &[u8]) {
        use egui_plot::{Line, Plot, PlotPoints};

        let rates = self.compute_filtered_rates(event_types);

        let max_y = rates.iter()
            .flat_map(|(_, r)| r.iter())
            .copied()
            .max()
            .unwrap_or(0) as f64;

        let resp = Plot::new(id)
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
            .label_formatter(|_name, value| {
                format!("t=-{:.0}s rate={:.0}/s", 60.0 - value.x, value.y)
            })
            .show(ui, |plot_ui| {
                let num_nodes = rates.len().max(1);
                let alpha = (255.0_f32 / num_nodes as f32).clamp(10.0, 200.0) as u8;

                for (_node_idx, node_rates) in rates.iter() {
                    if node_rates.len() < 2 {
                        continue;
                    }

                    let line_points: Vec<[f64; 2]> = node_rates
                        .iter()
                        .enumerate()
                        .map(|(x, &count)| [x as f64, count as f64])
                        .collect();

                    let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                    plot_ui.line(Line::new(PlotPoints::from(line_points)).color(color).width(1.0));
                }
            });

        Self::paint_plot_title(ui, resp.response.rect, title, colors::TEXT_MUTED);
    }
}

impl eframe::App for JamApp {
    #[allow(unused_variables)]
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Request continuous repaint for real-time updates
        ctx.request_repaint();

        // Process WebSocket messages (native only - WASM uses callbacks)
        #[cfg(not(target_arch = "wasm32"))]
        self.process_messages();

        // Periodic stats log (~1s)
        let now = now_seconds();
        #[cfg(not(target_arch = "wasm32"))]
        if now - self.stats_last_log >= 1.0 {
            let active = self.data.directed_buffer.active_count(now as f32, 5.0);
            let buffer_len = self.data.directed_buffer.len();
            let nodes = self.data.events.node_count();
            let enabled = self.prev_filter_bitfield.iter().map(|w| w.count_ones()).sum::<u32>();
            info!(
                ws_events_per_sec = self.stats_msg_count,
                gpu_uploaded_per_sec = self.stats_uploaded,
                active_particles = active,
                buffer_len,
                nodes,
                enabled_types = enabled,
                "stats"
            );
            self.stats_msg_count = 0;
            self.stats_uploaded = 0;
            self.stats_last_log = now;
        }

        // Prune old events periodically
        #[cfg(target_arch = "wasm32")]
        self.data.borrow_mut().events.prune(now);
        #[cfg(not(target_arch = "wasm32"))]
        self.data.events.prune(now);

        // Sync event filter to directed buffer for ring visualization
        let filter = self.build_filter_bitfield();
        if filter != self.prev_filter_bitfield {
            let enabled = filter.iter().map(|w| w.count_ones()).sum::<u32>();
            info!(
                enabled_types = enabled,
                bitfield = ?filter,
                "filter changed"
            );
            self.prev_filter_bitfield = filter;
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

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors::BG_PRIMARY))
            .show(ctx, |ui| {
                self.render_header(ui);

                ui.add_space(8.0);

                match self.active_tab {
                    ActiveTab::Pipeline => self.render_pipeline_tab(ui),
                    ActiveTab::Network => self.render_network_tab(ui),
                    ActiveTab::Consensus => self.render_consensus_tab(ui),
                    ActiveTab::Errors => self.render_errors_tab(ui),
                    ActiveTab::Ring => self.render_ring_tab(ui),
                }
            });

        // Filter modal window
        if self.show_event_selector {
            self.render_event_selector(ctx);
        }

        // Update scatter texture reference after callback has rendered
        #[cfg(not(target_arch = "wasm32"))]
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
