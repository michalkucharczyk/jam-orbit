//! Shared JAM Visualization App
//!
//! This module contains the egui app that runs on both native and WASM platforms.

use eframe::egui;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use crate::core::{parse_event, BestBlockData, EventStore, TimeSeriesData, EVENT_CATEGORIES};
use crate::theme::{colors, minimal_visuals};
use crate::time::now_seconds;
use crate::vring::{DirectedEventBuffer, PulseEvent};
use crate::ws_state::WsState;

#[cfg(target_arch = "wasm32")]
use crate::websocket_wasm::WsClient;

#[cfg(not(target_arch = "wasm32"))]
use crate::websocket_native::NativeWsClient;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use crate::scatter::{ScatterCallback, ScatterParticle, ScatterRenderer, ScatterUniforms};
#[cfg(not(target_arch = "wasm32"))]
use crate::vring::{FilterBitfield, GpuParticle, RingCallback, RingRenderer, Uniforms};

/// Default WebSocket URL for jamtart
pub const DEFAULT_WS_URL: &str = "ws://127.0.0.1:8080/api/ws";

/// Active tab in the visualization
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    Graphs,
    Ring,
}

/// An active collapsing-pulse animation on the ring.
struct CollapsingPulse {
    node_index: u16,
    event_type: u8,
    birth_time: f32,
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
    data: Rc<RefCell<SharedData>>,
    #[cfg(not(target_arch = "wasm32"))]
    data: SharedData,

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
    fps_counter: FpsCounter,
    /// Event filter: [event_type] = enabled
    selected_events: Vec<bool>,
    /// Toggle event selector panel visibility
    show_event_selector: bool,
    /// Toggle legend overlay visibility
    show_legend: bool,
    /// Currently active tab
    active_tab: ActiveTab,
    /// Use CPU rendering for all visualizations (--use-cpu flag, native only)
    #[cfg(not(target_arch = "wasm32"))]
    use_cpu: bool,
    /// Cursor for incremental GPU particle upload
    #[cfg(not(target_arch = "wasm32"))]
    gpu_upload_cursor: u64,
    /// Off-screen texture for GPU scatter renderer (None in CPU mode)
    #[cfg(not(target_arch = "wasm32"))]
    scatter_texture_id: Option<egui::TextureId>,
    /// Active collapsing-pulse animations on the ring
    active_pulses: Vec<CollapsingPulse>,
}

impl JamApp {
    /// Create new app for WASM platform
    #[cfg(target_arch = "wasm32")]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());

        let data = Rc::new(RefCell::new(SharedData {
            time_series: TimeSeriesData::new(1024, 200),
            blocks: BestBlockData::new(1024),
            events: EventStore::new(50000, 60.0),
            directed_buffer: DirectedEventBuffer::default(),
            pulse_events: Vec::new(),
        }));

        let ws_state = Rc::new(RefCell::new(WsState::Connecting));

        // Connect WebSocket with callback that updates shared data
        let data_clone = data.clone();
        let ws_client = WsClient::connect(
            DEFAULT_WS_URL,
            move |msg| {
                let now = now_seconds();
                let mut data = data_clone.borrow_mut();
                let SharedData {
                    ref mut time_series,
                    ref mut blocks,
                    ref mut events,
                    ref mut directed_buffer,
                    ref mut pulse_events,
                } = *data;
                parse_event(
                    &msg,
                    time_series,
                    blocks,
                    events,
                    directed_buffer,
                    pulse_events,
                    now,
                );
            },
            ws_state.clone(),
        )
        .ok();

        Self {
            data,
            ws_state,
            ws_client,
            fps_counter: FpsCounter::new(),
            selected_events: Self::default_selected_events(),
            show_event_selector: false,
            show_legend: true,
            active_tab: ActiveTab::default(),
            active_pulses: Vec::new(),
        }
    }

    /// Create new app for native platform
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(cc: &eframe::CreationContext<'_>, use_cpu: bool) -> Self {
        cc.egui_ctx.set_visuals(minimal_visuals());

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

        let ws_client = NativeWsClient::connect(DEFAULT_WS_URL);
        let ws_state = ws_client.state.clone();

        Self {
            data,
            ws_state,
            ws_client: Some(ws_client),
            fps_counter: FpsCounter::new(),
            selected_events: Self::default_selected_events(),
            show_event_selector: false,
            show_legend: true,
            active_tab: ActiveTab::default(),
            use_cpu,
            gpu_upload_cursor: 0,
            scatter_texture_id,
            active_pulses: Vec::new(),
        }
    }

    fn default_selected_events() -> Vec<bool> {
        // Enable all events by default
        vec![true; 200]
    }

    /// Build a [u64; 4] bitfield from selected_events for DirectedEventBuffer filtering
    fn build_filter_bitfield(&self) -> [u64; 4] {
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
        if let Some(ref client) = self.ws_client {
            while let Ok(msg) = client.rx.try_recv() {
                let now = now_seconds();
                parse_event(
                    &msg,
                    &mut self.data.time_series,
                    &mut self.data.blocks,
                    &mut self.data.events,
                    &mut self.data.directed_buffer,
                    &mut self.data.pulse_events,
                    now,
                );
            }
        }
    }

    /// Get the current WebSocket state
    fn get_ws_state(&self) -> WsState {
        #[cfg(target_arch = "wasm32")]
        {
            self.ws_state.borrow().clone()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.ws_state.lock().unwrap().clone()
        }
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

        // Prune old events periodically
        let now = now_seconds();
        #[cfg(target_arch = "wasm32")]
        self.data.borrow_mut().events.prune(now);
        #[cfg(not(target_arch = "wasm32"))]
        self.data.events.prune(now);

        // Sync event filter to directed buffer for ring visualization
        let filter = self.build_filter_bitfield();
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

                if self.show_event_selector {
                    ui.add_space(4.0);
                    self.render_event_selector(ui);
                }

                ui.add_space(8.0);

                match self.active_tab {
                    ActiveTab::Graphs => self.render_graphs_tab(ui),
                    ActiveTab::Ring => self.render_ring_tab(ui),
                }
            });

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

impl JamApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
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
            // Connection status indicator
            let (status_color, status_text) = match &ws_state {
                WsState::Connected => (egui::Color32::from_rgb(100, 200, 100), "Connected"),
                WsState::Connecting => (egui::Color32::from_rgb(200, 200, 100), "Connecting..."),
                WsState::Disconnected => (egui::Color32::from_rgb(200, 100, 100), "Disconnected"),
                WsState::Error(_) => (egui::Color32::from_rgb(200, 100, 100), "Error"),
            };

            ui.colored_label(status_color, egui::RichText::new(status_text).size(11.0));

            ui.add_space(10.0);

            ui.label(
                egui::RichText::new(format!("{:.0} fps", self.fps_counter.fps()))
                    .color(colors::TEXT_SECONDARY)
                    .monospace()
                    .size(11.0),
            );

            ui.label(
                egui::RichText::new("/")
                    .color(colors::TEXT_MUTED)
                    .size(11.0),
            );

            ui.label(
                egui::RichText::new(format!("{} validators", validator_count))
                    .color(colors::TEXT_MUTED)
                    .monospace()
                    .size(11.0),
            );

            if let Some(slot) = highest_slot {
                ui.label(
                    egui::RichText::new("/")
                        .color(colors::TEXT_MUTED)
                        .size(11.0),
                );
                ui.label(
                    egui::RichText::new(format!("slot {}", slot))
                        .color(colors::TEXT_MUTED)
                        .monospace()
                        .size(11.0),
                );
            }

            ui.label(
                egui::RichText::new("/")
                    .color(colors::TEXT_MUTED)
                    .size(11.0),
            );
            ui.label(
                egui::RichText::new(format!("{} nodes", event_count))
                    .color(colors::TEXT_MUTED)
                    .monospace()
                    .size(11.0),
            );

            ui.add_space(20.0);

            // Tab buttons
            let graphs_color = if self.active_tab == ActiveTab::Graphs {
                colors::TEXT_PRIMARY
            } else {
                colors::TEXT_MUTED
            };
            let ring_color = if self.active_tab == ActiveTab::Ring {
                colors::TEXT_PRIMARY
            } else {
                colors::TEXT_MUTED
            };

            if ui
                .selectable_label(
                    self.active_tab == ActiveTab::Graphs,
                    egui::RichText::new("Graphs").color(graphs_color).size(11.0),
                )
                .clicked()
            {
                self.active_tab = ActiveTab::Graphs;
            }
            if ui
                .selectable_label(
                    self.active_tab == ActiveTab::Ring,
                    egui::RichText::new("Ring").color(ring_color).size(11.0),
                )
                .clicked()
            {
                self.active_tab = ActiveTab::Ring;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("JAM")
                        .color(colors::TEXT_PRIMARY)
                        .size(12.0),
                );

                ui.add_space(10.0);

                let filter_text = if self.show_event_selector {
                    "Filter ▲"
                } else {
                    "Filter ▼"
                };
                if ui
                    .button(egui::RichText::new(filter_text).size(11.0))
                    .clicked()
                {
                    self.show_event_selector = !self.show_event_selector;
                }

                let legend_text = if self.show_legend {
                    "Legend ●"
                } else {
                    "Legend ○"
                };
                if ui
                    .button(egui::RichText::new(legend_text).size(11.0))
                    .clicked()
                {
                    self.show_legend = !self.show_legend;
                }
            });
        });
    }

    /// Render the Graphs tab content
    fn render_graphs_tab(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let graph_height = (available.y - 40.0) / 5.0;

        // Time series (num_peers)
        ui.allocate_ui(egui::vec2(available.x, graph_height), |ui| {
            self.render_time_series(ui);
        });

        ui.add_space(4.0);

        // Particle trails
        ui.allocate_ui(egui::vec2(available.x, graph_height), |ui| {
            self.render_particle_trails(ui);
        });

        ui.add_space(4.0);

        // Event rate lines
        ui.allocate_ui(egui::vec2(available.x, graph_height), |ui| {
            self.render_event_rates(ui);
        });

        ui.add_space(4.0);

        // Bottom row: Block scatter plots side by side
        ui.horizontal(|ui| {
            let half_width = (available.x - 10.0) / 2.0;

            ui.allocate_ui(egui::vec2(half_width, graph_height * 2.0 - 10.0), |ui| {
                self.render_best_blocks(ui);
            });

            ui.add_space(10.0);

            ui.allocate_ui(egui::vec2(half_width, graph_height * 2.0 - 10.0), |ui| {
                self.render_finalized_blocks(ui);
            });
        });

        // Draw legend overlay on graphs tab
        if self.show_legend {
            let panel_rect = ui.min_rect();
            self.draw_legend(ui.painter(), panel_rect);
        }
    }

    /// Render the Ring tab — routes to GPU or CPU path (native)
    #[cfg(not(target_arch = "wasm32"))]
    fn render_ring_tab(&mut self, ui: &mut egui::Ui) {
        if self.use_cpu {
            self.render_ring_tab_cpu(ui);
        } else {
            self.render_ring_tab_gpu(ui);
        }
    }

    /// Render the Ring tab — always CPU on WASM
    #[cfg(target_arch = "wasm32")]
    fn render_ring_tab(&mut self, ui: &mut egui::Ui) {
        self.render_ring_tab_cpu(ui);
    }

    /// GPU ring rendering path (native only).
    /// Particles rendered by GPU shader, overlays (ring, dots, legend) drawn by CPU.
    #[cfg(not(target_arch = "wasm32"))]
    fn render_ring_tab_gpu(&mut self, ui: &mut egui::Ui) {
        use std::f32::consts::PI;

        let now = now_seconds() as f32;

        let (particle_count, num_nodes, new_particles, new_cursor) = {
            let data = &self.data;
            let (particles, cursor, skip) =
                data.directed_buffer.get_new_since(self.gpu_upload_cursor);
            let gpu_particles: Vec<GpuParticle> =
                particles.iter().skip(skip).map(GpuParticle::from).collect();
            let new_cursor = cursor;
            (
                data.directed_buffer.len(),
                data.events.node_count().max(1),
                gpu_particles,
                new_cursor,
            )
        };
        self.gpu_upload_cursor = new_cursor;

        // Stats header
        self.render_ring_stats(ui, num_nodes, particle_count);

        // Allocate canvas
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, egui::Sense::hover());
        let rect = response.rect;

        // Ring geometry must match GPU shader (vring/shader.wgsl RING_RADIUS = 0.75).
        // NDC y-range (-1..1) maps to rect.height(), so 0.75 in NDC = 0.75 * height/2 pixels.
        // Aspect ratio is handled by the shader (x /= aspect_ratio), and by the painter
        // naturally since rect.width() absorbs horizontal stretch — both use height for radius.
        let center = rect.center();
        let pixel_radius = 0.75 * rect.height() * 0.5;
        let num_nodes_f = num_nodes as f32;

        // Draw ring outline and node dots (CPU overlay, matched to GPU coords)
        painter.circle_stroke(
            center,
            pixel_radius,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 100, 100, 40)),
        );
        let num_dots = num_nodes.min(256);
        for i in 0..num_dots {
            let angle = (i as f32 / num_nodes_f) * 2.0 * PI - PI * 0.5;
            let pos = center + egui::vec2(angle.cos(), angle.sin()) * pixel_radius;
            painter.circle_filled(
                pos,
                4.0,
                egui::Color32::from_rgba_unmultiplied(150, 150, 150, 100),
            );
        }

        // Draw collapsing pulse overlays
        self.draw_pulses(&painter, center, pixel_radius, num_nodes_f, now);

        // GPU paint callback for particles
        let filter = FilterBitfield::from_u64_bitfield(&self.build_filter_bitfield());
        let aspect_ratio = rect.width() / rect.height();
        let uniforms = Uniforms {
            current_time: now,
            num_validators: num_nodes as f32,
            aspect_ratio,
            point_size: 0.005,
        };
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            RingCallback {
                new_particles: Arc::new(new_particles),
                uniforms,
                filter,
                reset: false,
            },
        ));

        // Draw color legend (CPU overlay)
        if self.show_legend {
            self.draw_legend(&painter, rect);
        }
    }

    /// CPU ring rendering path (WASM + native --use-cpu fallback)
    fn render_ring_tab_cpu(&self, ui: &mut egui::Ui) {
        use std::f32::consts::PI;

        let now = now_seconds() as f32;
        let max_age = 5.0_f32;

        let (particle_count, num_nodes, active_particles) =
            with_data!(self, |data| {
                let particles = data.directed_buffer.get_active_particles(now, max_age);
                (
                    data.directed_buffer.len(),
                    data.events.node_count().max(1),
                    particles,
                )
            });

        // Stats header
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} nodes / {} particles ({} active)",
                    num_nodes,
                    particle_count,
                    active_particles.len()
                ))
                .color(colors::TEXT_MUTED)
                .monospace()
                .size(10.0),
            );
        });

        // Allocate canvas
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, egui::Sense::hover());
        let rect = response.rect;

        let center = rect.center();
        let radius = rect.width().min(rect.height()) * 0.4;
        let num_nodes_f = num_nodes as f32;

        // Draw ring outline
        painter.circle_stroke(
            center,
            radius,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 100, 100, 40)),
        );

        // Draw node dots
        let num_dots = num_nodes.min(256);
        for i in 0..num_dots {
            let angle = (i as f32 / num_nodes_f) * 2.0 * PI - PI * 0.5;
            let pos = center + egui::vec2(angle.cos(), angle.sin()) * radius;
            painter.circle_filled(
                pos,
                4.0,
                egui::Color32::from_rgba_unmultiplied(150, 150, 150, 100),
            );
        }

        // Draw active particles (CPU bezier computation)
        for particle in &active_particles {
            let age = now - particle.birth_time;
            let t = (age / particle.travel_duration).clamp(0.0, 1.0);

            let source_angle = (particle.source_index / num_nodes_f) * 2.0 * PI - PI * 0.5;
            let target_angle = (particle.target_index / num_nodes_f) * 2.0 * PI - PI * 0.5;

            let source_pos = center + egui::vec2(source_angle.cos(), source_angle.sin()) * radius;
            let target_pos = center + egui::vec2(target_angle.cos(), target_angle.sin()) * radius;

            let mid = source_pos + (target_pos - source_pos) * 0.5;
            let diff = target_pos - source_pos;
            let perp = egui::vec2(-diff.y, diff.x).normalized();
            let curve_amount = particle.curve_seed * diff.length() * 0.3;
            let control = mid + perp * curve_amount;

            let one_minus_t = 1.0 - t;
            let pos = egui::Pos2::new(
                source_pos.x * (one_minus_t * one_minus_t)
                    + control.x * (2.0 * one_minus_t * t)
                    + target_pos.x * (t * t),
                source_pos.y * (one_minus_t * one_minus_t)
                    + control.y * (2.0 * one_minus_t * t)
                    + target_pos.y * (t * t),
            );

            let color = self.get_event_color(particle.event_type as u8);
            let fade_in = (t / 0.1).min(1.0);
            let fade_out = 1.0 - ((t - 0.9) / 0.1).max(0.0);
            let alpha = (color.a() as f32 * fade_in * fade_out) as u8;
            let final_color = egui::Color32::from_rgba_unmultiplied(
                color.r(),
                color.g(),
                color.b(),
                alpha,
            );

            painter.circle_filled(pos, 3.0, final_color);
        }

        // Draw collapsing pulse overlays
        self.draw_pulses(&painter, center, radius, num_nodes_f, now);

        // Draw color legend
        if self.show_legend {
            self.draw_legend(&painter, rect);
        }
    }

    fn render_ring_stats(&self, ui: &mut egui::Ui, node_count: usize, particle_count: usize) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} nodes / {} particles",
                    node_count, particle_count
                ))
                .color(colors::TEXT_MUTED)
                .monospace()
                .size(10.0),
            );
        });
    }

    fn draw_legend(&self, painter: &egui::Painter, rect: egui::Rect) {
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

    /// Draw collapsing pulse circles as CPU overlay on the ring.
    fn draw_pulses(
        &self,
        painter: &egui::Painter,
        center: egui::Pos2,
        pixel_radius: f32,
        num_nodes: f32,
        now: f32,
    ) {
        use std::f32::consts::PI;
        const PULSE_DURATION: f32 = 0.4;
        const MAX_PULSE_RADIUS: f32 = 40.0;

        for pulse in &self.active_pulses {
            let age = now - pulse.birth_time;
            if age < 0.0 || age >= PULSE_DURATION {
                continue;
            }

            // t goes from 0 (just born, circle is big) to 1 (collapsed to dot)
            let t = age / PULSE_DURATION;

            // Circle radius: starts at MAX, collapses to 0 (quadratic ease-in)
            let radius_factor = (1.0 - t) * (1.0 - t);
            let pulse_radius = MAX_PULSE_RADIUS * radius_factor;

            // Validator position on ring
            let angle = (pulse.node_index as f32 / num_nodes) * 2.0 * PI - PI * 0.5;
            let pos = center + egui::vec2(angle.cos(), angle.sin()) * pixel_radius;

            // Color from event type, alpha fades out
            let base_color = self.get_event_color(pulse.event_type);
            let alpha = (180.0 * (1.0 - t)) as u8;
            let color = egui::Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                alpha,
            );

            // Stroke width: thicker at start, thinner as it collapses
            let stroke_width = 1.0 + 1.5 * (1.0 - t);

            painter.circle_stroke(pos, pulse_radius, egui::Stroke::new(stroke_width, color));
        }
    }

    /// Get color for event type
    fn get_event_color(&self, event_type: u8) -> egui::Color32 {
        match event_type {
            0 => egui::Color32::from_rgb(128, 128, 128),       // Meta - gray
            10..=13 => egui::Color32::from_rgb(100, 200, 100), // Status - green
            20..=28 => egui::Color32::from_rgb(100, 150, 255), // Connection - blue
            40..=47 => egui::Color32::from_rgb(255, 200, 100), // Block auth - orange
            60..=68 => egui::Color32::from_rgb(200, 100, 255), // Block dist - purple
            80..=84 => egui::Color32::from_rgb(255, 100, 100), // Tickets - red
            90..=104 => egui::Color32::from_rgb(100, 255, 200), // Work Package - cyan
            105..=113 => egui::Color32::from_rgb(50, 200, 180), // Guaranteeing - teal
            120..=131 => egui::Color32::from_rgb(255, 255, 100), // Availability - yellow
            140..=153 => egui::Color32::from_rgb(255, 150, 150), // Bundle - pink
            160..=178 => egui::Color32::from_rgb(150, 200, 255), // Segment - light blue
            190..=199 => egui::Color32::from_rgb(200, 200, 200), // Preimage - light gray
            _ => egui::Color32::from_rgb(255, 255, 255),
        }
    }

    fn render_time_series(&self, ui: &mut egui::Ui) {
        use egui_plot::{Line, Plot, PlotPoints};

        ui.label(
            egui::RichText::new("Peer Count")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let (point_count, y_min, y_max, series_data) = with_data!(self, |data| {
            let point_count = data.time_series.point_count();
            let (y_min, y_max) = data
                .time_series
                .series
                .iter()
                .flat_map(|s| s.iter())
                .fold((f32::MAX, f32::MIN), |(min, max), &v| {
                    (min.min(v), max.max(v))
                });

            let series_data: Vec<Vec<f32>> =
                data.time_series.series.iter().map(|s| s.clone()).collect();

            (point_count, y_min, y_max, series_data)
        });

        let (y_min, y_max) = if y_min > y_max {
            (0.0, 100.0)
        } else {
            let pad = (y_max - y_min).max(10.0) * 0.1;
            (y_min - pad, y_max + pad)
        };

        Plot::new("time_series")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(point_count.max(1) as f64)
            .include_y(y_min as f64)
            .include_y(y_max as f64)
            .label_formatter(|_name, value| {
                format!("t={} peers={:.0}", value.x as u32, value.y)
            })
            .show(ui, |plot_ui| {
                for series in &series_data {
                    if series.len() < 2 {
                        continue;
                    }

                    let points: PlotPoints = series
                        .iter()
                        .enumerate()
                        .map(|(x, &y)| [x as f64, y as f64])
                        .collect();

                    let color =
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, colors::LINE_ALPHA);
                    plot_ui.line(Line::new(points).color(color).width(1.0));
                }
            });
    }

    fn render_best_blocks(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        ui.label(
            egui::RichText::new("Best Block")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let (max_block, points_data) = with_data!(self, |data| {
            let max_block = data.blocks.highest_slot().unwrap_or(1) as f64;
            let points_data: Vec<[f64; 2]> = data
                .blocks
                .best_blocks
                .iter()
                .enumerate()
                .filter(|(_, &slot)| slot > 0)
                .map(|(id, &slot)| [id as f64, slot as f64])
                .collect();
            (max_block, points_data)
        });

        Plot::new("best_blocks")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(max_block - 10.0)
            .include_y(max_block + 5.0)
            .label_formatter(|_name, value| {
                format!("validator={} slot={:.0}", value.x as u32, value.y)
            })
            .show(ui, |plot_ui| {
                plot_ui.points(
                    Points::new(PlotPoints::from(points_data))
                        .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180))
                        .radius(2.0)
                        .filled(true),
                );
            });
    }

    fn render_finalized_blocks(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        ui.label(
            egui::RichText::new("Finalized Block")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let (max_finalized, points_data) = with_data!(self, |data| {
            let max_finalized = data.blocks.highest_finalized().unwrap_or(1) as f64;
            let points_data: Vec<[f64; 2]> = data
                .blocks
                .finalized_blocks
                .iter()
                .enumerate()
                .filter(|(_, &slot)| slot > 0)
                .map(|(id, &slot)| [id as f64, slot as f64])
                .collect();
            (max_finalized, points_data)
        });

        Plot::new("finalized_blocks")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(max_finalized - 10.0)
            .include_y(max_finalized + 5.0)
            .label_formatter(|_name, value| {
                format!("validator={} finalized={:.0}", value.x as u32, value.y)
            })
            .show(ui, |plot_ui| {
                plot_ui.points(
                    Points::new(PlotPoints::from(points_data))
                        .color(egui::Color32::from_rgba_unmultiplied(150, 150, 150, 180))
                        .radius(2.0)
                        .filled(true),
                );
            });
    }

    /// Render Event Particles — routes to GPU or CPU path (native)
    #[cfg(not(target_arch = "wasm32"))]
    fn render_particle_trails(&self, ui: &mut egui::Ui) {
        if self.scatter_texture_id.is_some() && !self.use_cpu {
            self.render_particle_trails_gpu(ui);
        } else {
            self.render_particle_trails_cpu(ui);
        }
    }

    /// Render Event Particles — always CPU on WASM
    #[cfg(target_arch = "wasm32")]
    fn render_particle_trails(&self, ui: &mut egui::Ui) {
        self.render_particle_trails_cpu(ui);
    }

    /// GPU scatter rendering path (native only)
    #[cfg(not(target_arch = "wasm32"))]
    fn render_particle_trails_gpu(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Event Particles")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let now = now_seconds();
        let max_age = 10.0;
        let cutoff = now - max_age;

        // Collect scatter particles from EventStore
        let (new_particles, node_count) = {
            let data = &self.data;
            let mut particles = Vec::new();
            for (_, node) in data.events.nodes() {
                for (&event_type, events) in &node.by_type {
                    if (event_type as usize) >= self.selected_events.len()
                        || !self.selected_events[event_type as usize]
                    {
                        continue;
                    }
                    for stored in events {
                        if stored.timestamp < cutoff {
                            continue;
                        }
                        particles.push(ScatterParticle {
                            node_index: node.index as f32,
                            birth_time: stored.timestamp as f32,
                            event_type: event_type as f32,
                        });
                    }
                }
            }
            (particles, data.events.node_count().max(1) as f32)
        };

        // Allocate canvas area
        let available = ui.available_size();
        let (rect, _response) = ui.allocate_exact_size(available, egui::Sense::hover());

        // Display the off-screen texture
        let texture_id = self.scatter_texture_id.unwrap();
        ui.painter().image(
            texture_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        // Submit callback for GPU upload + render
        let filter = FilterBitfield::from_u64_bitfield(&self.build_filter_bitfield());
        let aspect_ratio = rect.width() / rect.height();
        let x_margin = 0.5; // padding so edge nodes aren't clipped
        let uniforms = ScatterUniforms {
            x_range: [-x_margin, node_count - 1.0 + x_margin],
            y_range: [0.0, max_age as f32],
            point_size: 0.008,
            current_time: now as f32,
            max_age: max_age as f32,
            aspect_ratio,
        };

        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ScatterCallback {
                new_particles: Arc::new(new_particles),
                uniforms,
                filter,
                rect,
                reset: true, // Full upload each frame (not incremental for scatter)
            },
        ));
    }

    /// CPU scatter rendering path (WASM + native --use-cpu fallback)
    fn render_particle_trails_cpu(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        ui.label(
            egui::RichText::new("Event Particles")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let now = now_seconds();
        let max_age = 10.0;
        let cutoff = now - max_age;

        // Collect points grouped by event category for colored rendering
        let category_points: Vec<(egui::Color32, Vec<[f64; 2]>)> = with_data!(self, |data| {
            let mut result = Vec::new();

            for category in EVENT_CATEGORIES {
                let color = self.get_event_color(category.event_types[0]);
                let mut points: Vec<[f64; 2]> = Vec::new();

                for &event_type in category.event_types {
                    if (event_type as usize) >= self.selected_events.len()
                        || !self.selected_events[event_type as usize]
                    {
                        continue;
                    }

                    for (_, node) in data.events.nodes() {
                        if let Some(events) = node.by_type.get(&event_type) {
                            for stored in events {
                                if stored.timestamp >= cutoff {
                                    let age = now - stored.timestamp;
                                    points.push([node.index as f64, age]);
                                }
                            }
                        }
                    }
                }

                if !points.is_empty() {
                    result.push((color, points));
                }
            }

            result
        });

        Plot::new("particle_trails")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_y(0.0)
            .include_y(max_age)
            .label_formatter(|_name, value| {
                format!("node={} age={:.1}s", value.x as u32, value.y)
            })
            .show(ui, |plot_ui| {
                for (color, points) in &category_points {
                    plot_ui.points(
                        Points::new(PlotPoints::from(points.clone()))
                            .color(*color)
                            .radius(2.0)
                            .filled(true),
                    );
                }
            });
    }

    fn render_event_rates(&self, ui: &mut egui::Ui) {
        use egui_plot::{Line, Plot, PlotPoints};

        ui.label(
            egui::RichText::new("Event Rate (per node)")
                .color(colors::TEXT_MUTED)
                .size(10.0),
        );

        let now = now_seconds();

        let rates: Vec<(u16, Vec<u32>)> = with_data!(self, |data| {
            data.events
                .compute_rates_per_node(now, 1.0, 60, &self.selected_events)
        });

        Plot::new("event_rates")
            .show_axes([false, true])
            .show_grid(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .show_background(false)
            .include_x(0.0)
            .include_x(60.0)
            .include_y(0.0)
            .include_y(50.0)
            .label_formatter(|_name, value| {
                format!("t=-{:.0}s rate={:.0}/s", 60.0 - value.x, value.y)
            })
            .show(ui, |plot_ui| {
                let num_nodes = rates.len().max(1);
                let alpha = (255.0_f32 / num_nodes as f32).max(10.0).min(200.0) as u8;

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
    }

    fn render_event_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .button(
                    egui::RichText::new("All")
                        .color(colors::TEXT_SECONDARY)
                        .size(11.0),
                )
                .clicked()
            {
                self.selected_events.fill(true);
            }
            if ui
                .button(
                    egui::RichText::new("None")
                        .color(colors::TEXT_SECONDARY)
                        .size(11.0),
                )
                .clicked()
            {
                self.selected_events.fill(false);
            }
            ui.add_space(10.0);

            for category in EVENT_CATEGORIES {
                let all_selected = category
                    .event_types
                    .iter()
                    .all(|&et| self.selected_events[et as usize]);
                let mut cat_checked = all_selected;

                let text_color = if all_selected {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_SECONDARY
                };

                if ui
                    .checkbox(
                        &mut cat_checked,
                        egui::RichText::new(category.name)
                            .color(text_color)
                            .size(11.0),
                    )
                    .on_hover_text(format!("{} events", category.event_types.len()))
                    .changed()
                {
                    for &et in category.event_types {
                        self.selected_events[et as usize] = cat_checked;
                    }
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
        let now = now_seconds() * 1000.0; // Convert to ms for compatibility
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
