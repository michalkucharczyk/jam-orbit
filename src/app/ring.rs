//! Validators ring visualization

use eframe::egui;
use crate::theme::colors;
use crate::time::now_seconds;
use super::{JamApp, with_data};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use crate::vring::{FilterBitfield, GpuParticle, RingCallback, Uniforms};

impl JamApp {
    /// Render the Ring tab — routes to GPU or CPU path (native)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn render_ring_tab(&mut self, ui: &mut egui::Ui) {
        if self.use_cpu {
            self.render_ring_tab_cpu(ui);
        } else {
            self.render_ring_tab_gpu(ui);
        }
    }

    /// Render the Ring tab — always CPU on WASM
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn render_ring_tab(&mut self, ui: &mut egui::Ui) {
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
                .size(14.0),
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

            let pos = if particle.source_index == particle.target_index {
                // Radial: straight outward from validator to outer circle
                let angle = (particle.source_index / num_nodes_f) * 2.0 * PI - PI * 0.5;
                let dir = egui::vec2(angle.cos(), angle.sin());
                let r = radius + (radius * 0.2) * t;
                center + dir * r
            } else {
                // Directed: bezier curve between source and target
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
                egui::Pos2::new(
                    source_pos.x * (one_minus_t * one_minus_t)
                        + control.x * (2.0 * one_minus_t * t)
                        + target_pos.x * (t * t),
                    source_pos.y * (one_minus_t * one_minus_t)
                        + control.y * (2.0 * one_minus_t * t)
                        + target_pos.y * (t * t),
                )
            };

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
                .size(14.0),
            );
        });
    }

    /// Draw collapsing pulse circles as CPU overlay on the ring.
    pub(crate) fn draw_pulses(
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
            // Respect event type filter
            let et = pulse.event_type as usize;
            if et < self.selected_events.len() && !self.selected_events[et] {
                continue;
            }

            let age = now - pulse.birth_time;
            if age < 0.0 || age >= PULSE_DURATION {
                continue;
            }

            let t = age / PULSE_DURATION;
            let radius_factor = (1.0 - t) * (1.0 - t);
            let pulse_radius = MAX_PULSE_RADIUS * radius_factor;

            let angle = (pulse.node_index as f32 / num_nodes) * 2.0 * PI - PI * 0.5;
            let pos = center + egui::vec2(angle.cos(), angle.sin()) * pixel_radius;

            let base_color = self.get_event_color(pulse.event_type);
            let alpha = (180.0 * (1.0 - t)) as u8;
            let color = egui::Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                alpha,
            );

            let stroke_width = 1.0 + 1.5 * (1.0 - t);
            painter.circle_stroke(pos, pulse_radius, egui::Stroke::new(stroke_width, color));
        }
    }
}
