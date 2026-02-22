//! Validators ring visualization

use eframe::egui;
use crate::time::now_seconds;
use super::{JamApp, with_data};

use std::sync::Arc;
use crate::vring::{FilterBitfield, GpuParticle, RingCallback, Uniforms};

impl JamApp {
    /// Render the Ring tab — routes to GPU or CPU path.
    pub(crate) fn render_ring_tab(&mut self, ui: &mut egui::Ui) {
        if self.use_cpu {
            self.render_ring_tab_cpu(ui);
        } else {
            self.render_ring_tab_gpu(ui);
        }
    }

    /// GPU ring rendering path.
    /// Particles rendered by GPU shader, overlays (ring, dots, legend) drawn by CPU.
    fn render_ring_tab_gpu(&mut self, ui: &mut egui::Ui) {
        use std::f32::consts::PI;

        let now = now_seconds() as f32;

        let (particle_max, active_count, num_nodes, new_particles, new_cursor, peer_counts) =
            with_data!(self, |data| {
                let (particles, cursor, skip) =
                    data.directed_buffer.get_new_since(self.gpu_upload_cursor);
                let gpu_particles: Vec<GpuParticle> =
                    particles.iter().skip(skip).map(GpuParticle::from).collect();
                let nc = data.events.node_count().max(1);
                let mut counts = vec![0.0f32; nc];
                for (node_id, node) in data.events.nodes() {
                    let idx = node.index as usize;
                    if idx < nc {
                        if let Some(c) = data.time_series.latest_value(node_id) {
                            counts[idx] = c;
                        }
                    }
                }
                (
                    data.directed_buffer.capacity(),
                    data.directed_buffer.active_count(now, 5.0),
                    nc,
                    gpu_particles,
                    cursor,
                    counts,
                )
            });
        self.gpu_upload_cursor = new_cursor;

        #[cfg(not(target_arch = "wasm32"))]
        { self.stats_uploaded += new_particles.len() as u64; }

        // Update particle stats for header display
        self.particle_count = active_count;
        self.particle_max = particle_max;

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
        let max_peers = peer_counts.iter().cloned().fold(1.0f32, f32::max);
        for i in 0..num_dots {
            let angle = (i as f32 / num_dots as f32) * 2.0 * PI - PI * 0.5;
            let pos = center + egui::vec2(angle.cos(), angle.sin()) * pixel_radius;
            let brightness = (peer_counts[i] / max_peers).clamp(0.1, 1.0);
            let gray = (80.0 + brightness * 120.0) as u8;
            let alpha = (60.0 + brightness * 180.0) as u8;
            painter.circle_filled(
                pos,
                4.0,
                egui::Color32::from_rgba_unmultiplied(gray, gray, gray, alpha),
            );
        }

        // Draw collapsing pulse overlays
        self.draw_pulses(&painter, center, pixel_radius, num_nodes_f, now);

        // Draw slot pulse
        Self::draw_slot_pulse(&painter, center, pixel_radius);

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
                color_lut: self.color_lut,
                reset: false,
            },
        ));

    }

    /// CPU ring rendering path (WASM + native --use-cpu fallback)
    fn render_ring_tab_cpu(&mut self, ui: &mut egui::Ui) {
        use std::f32::consts::PI;

        let now = now_seconds() as f32;
        let max_age = 5.0_f32;

        let (particle_max, num_nodes, active_particles, peer_counts) =
            with_data!(self, |data| {
                let particles = data.directed_buffer.get_active_particles(now, max_age);
                let nc = data.events.node_count().max(1);
                let mut counts = vec![0.0f32; nc];
                for (node_id, node) in data.events.nodes() {
                    let idx = node.index as usize;
                    if idx < nc {
                        if let Some(c) = data.time_series.latest_value(node_id) {
                            counts[idx] = c;
                        }
                    }
                }
                (
                    data.directed_buffer.capacity(),
                    nc,
                    particles,
                    counts,
                )
            });

        // Update particle stats for header display
        self.particle_count = active_particles.len();
        self.particle_max = particle_max;

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

        // Draw node dots (brightness by peer count)
        let num_dots = num_nodes.min(256);
        let max_peers = peer_counts.iter().cloned().fold(1.0f32, f32::max);
        for i in 0..num_dots {
            let angle = (i as f32 / num_dots as f32) * 2.0 * PI - PI * 0.5;
            let pos = center + egui::vec2(angle.cos(), angle.sin()) * radius;
            let brightness = (peer_counts[i] / max_peers).clamp(0.1, 1.0);
            let gray = (80.0 + brightness * 120.0) as u8;
            let alpha = (60.0 + brightness * 180.0) as u8;
            painter.circle_filled(
                pos,
                4.0,
                egui::Color32::from_rgba_unmultiplied(gray, gray, gray, alpha),
            );
        }

        // Draw active particles (CPU path)
        const NUM_SAMPLES: usize = 16;
        const DIRECTED_SPEED: f32 = 8.0;
        for particle in &active_particles {
            let age = now - particle.birth_time;
            let et_idx = particle.event_type as usize;
            let [r, g, b, a] = self.color_lut.colors[et_idx];
            let color = egui::Color32::from_rgba_unmultiplied(
                (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, (a * 255.0) as u8,
            );

            if particle.source_index == particle.target_index {
                // ── Radial: circle particle (unchanged) ──
                let t = (age / particle.travel_duration).clamp(0.0, 1.0);
                if age > particle.travel_duration * 1.5 || age < 0.0 {
                    continue;
                }
                let angle = (particle.source_index / num_nodes_f) * 2.0 * PI - PI * 0.5;
                let dir = egui::vec2(angle.cos(), angle.sin());
                let r = radius + (radius * 0.2) * t;
                let pos = center + dir * r;

                let fade_in = (t / 0.1).min(1.0);
                let fade_out = 1.0 - ((t - 0.9) / 0.1).max(0.0);
                let alpha = (color.a() as f32 * fade_in * fade_out) as u8;
                let final_color = egui::Color32::from_rgba_unmultiplied(
                    color.r(), color.g(), color.b(), alpha,
                );
                painter.circle_filled(pos, 3.0, final_color);
            } else {
                // ── Directed: bezier trail line (4x speed) ──
                let eff_dur = particle.travel_duration / DIRECTED_SPEED;
                let t_head = (age / eff_dur).clamp(0.0, 1.0);
                let t_tail = ((age - eff_dur) / eff_dur).clamp(0.0, 1.0);
                if age > eff_dur * 2.5 || age < 0.0 || t_head <= t_tail {
                    continue;
                }

                let overall = age / (eff_dur * 2.0);
                let fade_in = (overall / 0.05).min(1.0);
                let fade_out = 1.0 - ((overall - 0.95) / 0.05).max(0.0);
                let base_alpha = color.a() as f32 * fade_in * fade_out;

                let source_angle =
                    (particle.source_index / num_nodes_f) * 2.0 * PI - PI * 0.5;
                let target_angle =
                    (particle.target_index / num_nodes_f) * 2.0 * PI - PI * 0.5;
                let source_pos =
                    center + egui::vec2(source_angle.cos(), source_angle.sin()) * radius;
                let target_pos =
                    center + egui::vec2(target_angle.cos(), target_angle.sin()) * radius;
                let mid = source_pos + (target_pos - source_pos) * 0.5;
                let diff = target_pos - source_pos;
                let perp = egui::vec2(-diff.y, diff.x).normalized();
                let curve_amount = particle.curve_seed * diff.length() * 0.3;
                let control = mid + perp * curve_amount;

                let points: Vec<egui::Pos2> = (0..=NUM_SAMPLES)
                    .map(|i| {
                        let frac = i as f32 / NUM_SAMPLES as f32;
                        let ct = t_tail + (t_head - t_tail) * frac;
                        let omt = 1.0 - ct;
                        egui::Pos2::new(
                            source_pos.x * (omt * omt)
                                + control.x * (2.0 * omt * ct)
                                + target_pos.x * (ct * ct),
                            source_pos.y * (omt * omt)
                                + control.y * (2.0 * omt * ct)
                                + target_pos.y * (ct * ct),
                        )
                    })
                    .collect();

                let trail_alpha = (base_alpha * 0.65) as u8;
                let stroke_color = egui::Color32::from_rgba_unmultiplied(
                    color.r(), color.g(), color.b(), trail_alpha,
                );
                if points.len() >= 2 {
                    painter.add(egui::Shape::line(
                        points,
                        egui::Stroke::new(1.0, stroke_color),
                    ));
                }
            }
        }

        // Draw collapsing pulse overlays
        self.draw_pulses(&painter, center, radius, num_nodes_f, now);

        // Draw slot pulse
        Self::draw_slot_pulse(&painter, center, radius);

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
            let et = pulse.event_type.idx();
            if et < self.selected_events.len() && !self.selected_events[et] {
                continue;
            }

            let age = now - pulse.birth_time;
            if !(0.0..PULSE_DURATION).contains(&age) {
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

    /// Draw a slot-boundary expanding ring (6-second JAM slot cycle).
    fn draw_slot_pulse(
        painter: &egui::Painter,
        center: egui::Pos2,
        pixel_radius: f32,
    ) {
        const JAM_EPOCH: f64 = 1_735_732_800.0; // Jan 1 2025 00:00:00 UTC
        const SLOT_DURATION: f64 = 6.0;

        let now_unix = crate::time::now_unix_seconds();
        let phase = ((now_unix - JAM_EPOCH) % SLOT_DURATION / SLOT_DURATION) as f32;

        // Ease-out: fast start, slow end (quadratic)
        let eased = 1.0 - (1.0 - phase) * (1.0 - phase);
        // Expand to ~1.5x ring radius (just past radial particles at ~1.44x)
        let ring_r = pixel_radius * (1.0 + 0.5 * eased);
        let fade_t = ((eased - 0.2) / 0.8).clamp(0.0, 1.0);
        let inv = 1.0 - fade_t;
        let fade = inv * inv * inv * inv; // quartic: very aggressive fade
        let alpha = (100.0 * fade) as u8;
        if alpha == 0 { return; }
        let stroke_width = 1.5_f32.max(2.0 * (1.0 - eased));

        let color = egui::Color32::from_rgba_unmultiplied(100, 100, 100, alpha);

        // Manual circle with enough segments for smooth rendering at large radii
        use std::f32::consts::PI;
        let num_segments = ((ring_r * 0.5) as usize).clamp(64, 256);
        let points: Vec<egui::Pos2> = (0..=num_segments)
            .map(|i| {
                let angle = (i as f32 / num_segments as f32) * 2.0 * PI;
                center + egui::vec2(angle.cos(), angle.sin()) * ring_r
            })
            .collect();
        painter.add(egui::Shape::line(points, egui::Stroke::new(stroke_width, color)));
    }
}
