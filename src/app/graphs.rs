//! Graphs tab: peer count, particle trails, event rates, block scatter plots

use eframe::egui;
use crate::core::EVENT_CATEGORIES;
use crate::theme::colors;
use crate::time::now_seconds;
use super::{JamApp, with_data};

use std::sync::Arc;
use crate::vring::FilterBitfield;
use crate::scatter::{ScatterCallback, ScatterParticle, ScatterUniforms};

impl JamApp {
    pub(crate) fn render_graphs_tab(&self, ui: &mut egui::Ui) {
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

    }

    fn render_time_series(&self, ui: &mut egui::Ui) {
        use egui_plot::{Line, Plot, PlotPoints};

        ui.label(
            egui::RichText::new("Peer Count")
                .color(colors::TEXT_MUTED)
                .size(14.0),
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
                data.time_series.series.to_vec();

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
                .size(14.0),
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
                .size(14.0),
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

    /// Render Event Particles — routes to GPU or CPU path.
    fn render_particle_trails(&self, ui: &mut egui::Ui) {
        if self.scatter_texture_id.is_some() && !self.use_cpu {
            self.render_particle_trails_gpu(ui);
        } else {
            self.render_particle_trails_cpu(ui);
        }
    }

    /// GPU scatter rendering path.
    fn render_particle_trails_gpu(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Event Particles")
                .color(colors::TEXT_MUTED)
                .size(14.0),
        );

        let now = now_seconds();
        let max_age = 10.0;
        let cutoff = now - max_age;

        // Collect scatter particles from EventStore
        let (new_particles, node_count) = with_data!(self, |data| {
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
        });

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
        let x_margin = 0.5;
        let uniforms = ScatterUniforms {
            x_range: [-x_margin, node_count - 1.0 + x_margin],
            y_range: [0.0, max_age as f32],
            point_size: 0.008,
            current_time: now as f32,
            max_age: max_age as f32,
            aspect_ratio,
            speed_factor: self.speed_factor,
            _pad: [0.0; 3],
        };

        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ScatterCallback {
                new_particles: Arc::new(new_particles),
                uniforms,
                filter,
                color_lut: self.color_lut,
                rect,
                reset: true,
            },
        ));
    }

    /// CPU scatter rendering path (WASM + native --use-cpu fallback)
    fn render_particle_trails_cpu(&self, ui: &mut egui::Ui) {
        use egui_plot::{Plot, PlotPoints, Points};

        ui.label(
            egui::RichText::new("Event Particles")
                .color(colors::TEXT_MUTED)
                .size(14.0),
        );

        let now = now_seconds();
        let max_age = 10.0;
        let cutoff = now - max_age;

        let category_points: Vec<(egui::Color32, Vec<[f64; 2]>)> = with_data!(self, |data| {
            let mut result = Vec::new();

            for category in EVENT_CATEGORIES {
                let color = self.get_event_color(category.event_types[0]);
                let mut points: Vec<[f64; 2]> = Vec::new();

                for &event_type in category.event_types {
                    if event_type.idx() >= self.selected_events.len()
                        || !self.selected_events[event_type.idx()]
                    {
                        continue;
                    }

                    let et_u8 = event_type as u8;
                    for (_, node) in data.events.nodes() {
                        if let Some(events) = node.by_type.get(&et_u8) {
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
                .size(14.0),
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
    }
}
