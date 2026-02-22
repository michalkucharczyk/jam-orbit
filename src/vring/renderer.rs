//! GPU renderer for validators ring visualization
//!
//! Renders directed events as particles traveling between validators
//! arranged on a circle using GPU instancing. Draws directly into
//! egui's render pass via CallbackTrait.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu::{self, util::DeviceExt};
use std::sync::Arc;

use super::DirectedParticleInstance;

const BUFFER_CAPACITY: usize = 5_000_000; // 5M particles per buffer
const NUM_BUFFERS: usize = 4; // 4 buffers = 20M particles total
const MAX_INSTANCES: usize = BUFFER_CAPACITY * NUM_BUFFERS;

/// GPU-compatible particle instance (must match DirectedParticleInstance layout)
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuParticle {
    pub source_index: f32,
    pub target_index: f32,
    pub birth_time: f32,
    pub travel_duration: f32,
    pub event_type: f32,
    pub curve_seed: f32,
}

impl From<&DirectedParticleInstance> for GpuParticle {
    fn from(p: &DirectedParticleInstance) -> Self {
        Self {
            source_index: p.source_index,
            target_index: p.target_index,
            birth_time: p.birth_time,
            travel_duration: p.travel_duration,
            event_type: p.event_type,
            curve_seed: p.curve_seed,
        }
    }
}

/// Uniform buffer layout for the shader
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Uniforms {
    pub current_time: f32,
    pub num_validators: f32,
    pub aspect_ratio: f32,
    pub point_size: f32,
    pub speed_factor: f32,
    pub _pad: [f32; 3],
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            current_time: 0.0,
            num_validators: 1024.0,
            aspect_ratio: 1.0,
            point_size: 0.005,
            speed_factor: 1.0,
            _pad: [0.0; 3],
        }
    }
}

/// Color lookup table indexed directly by event_type (0–255).
/// CPU fills this dynamically based on filter state; GPU does a simple array lookup.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ColorLut {
    pub colors: [[f32; 4]; 256],
}

impl Default for ColorLut {
    fn default() -> Self {
        // All entries default to transparent; app fills via build_color_lut()
        Self {
            colors: [[0.5, 0.5, 0.5, 0.8]; 256],
        }
    }
}

/// Predefined color schema for event categories and per-event distinct palettes.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSchema {
    #[default]
    Vivid,
    Accessible,
    Pipeline,
    Monochrome,
}

impl ColorSchema {
    pub const ALL: &[ColorSchema] = &[
        ColorSchema::Vivid,
        ColorSchema::Accessible,
        ColorSchema::Pipeline,
        ColorSchema::Monochrome,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Vivid => "Vivid",
            Self::Accessible => "Accessible",
            Self::Pipeline => "Pipeline",
            Self::Monochrome => "Mono",
        }
    }

    /// Category colors [f32; 4] RGBA, matching EVENT_CATEGORIES order:
    /// Status, Connection, Block Auth/Import, Block Distribution, Safrole Tickets,
    /// Work Package, Guaranteeing, Availability, Bundle Recovery, Segment Recovery,
    /// Preimages, Meta
    pub fn colors(self) -> &'static [[f32; 4]; 12] {
        match self {
            Self::Vivid => &VIVID_COLORS,
            Self::Accessible => &ACCESSIBLE_COLORS,
            Self::Pipeline => &PIPELINE_COLORS,
            Self::Monochrome => &MONOCHROME_COLORS,
        }
    }

    /// Generate `n` distinct per-event colors appropriate for this schema.
    pub fn generate_distinct_palette(self, n: usize) -> Vec<[f32; 4]> {
        if n == 0 { return vec![]; }
        if n == 1 { return vec![[1.0, 1.0, 1.0, 0.8]]; }

        match self {
            Self::Vivid => {
                // Full HSL rainbow, high saturation
                (0..n).map(|i| {
                    let hue = i as f32 / n as f32;
                    let (r, g, b) = hsl_to_rgb(hue, 0.8, 0.65);
                    [r, g, b, 0.8]
                }).collect()
            }
            Self::Accessible => {
                // Cycle through Okabe-Ito 8-color set (colorblind-safe)
                const OI: [[f32; 3]; 8] = [
                    [0.90, 0.62, 0.00], // orange
                    [0.34, 0.71, 0.91], // sky blue
                    [0.00, 0.62, 0.45], // bluish green
                    [0.94, 0.89, 0.26], // yellow
                    [0.00, 0.45, 0.70], // blue
                    [0.84, 0.37, 0.00], // vermillion
                    [0.80, 0.47, 0.65], // reddish purple
                    [0.60, 0.60, 0.20], // olive
                ];
                (0..n).map(|i| {
                    let base = OI[i % OI.len()];
                    let cycle = (i / OI.len()) as f32;
                    // Shift lightness on subsequent cycles
                    let factor = 1.0 - cycle * 0.2;
                    [base[0] * factor, base[1] * factor, base[2] * factor, 0.8]
                }).collect()
            }
            Self::Pipeline => {
                // HSL rainbow, slightly desaturated
                (0..n).map(|i| {
                    let hue = i as f32 / n as f32;
                    let (r, g, b) = hsl_to_rgb(hue, 0.6, 0.55);
                    [r, g, b, 0.8]
                }).collect()
            }
            Self::Monochrome => {
                // Cyan hue, luminance steps only
                (0..n).map(|i| {
                    let lightness = 0.3 + 0.6 * (i as f32 / (n - 1) as f32);
                    let (r, g, b) = hsl_to_rgb(0.5, 0.5, lightness);
                    [r, g, b, 0.8]
                }).collect()
            }
        }
    }
}

impl std::fmt::Display for ColorSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Vivid: infrastructure=cool/muted, consensus=warm/gold, pipeline=saturated primaries, verification=warm
const VIVID_COLORS: [[f32; 4]; 12] = [
    [0.40, 0.80, 0.40, 0.8], // Status - muted green
    [0.40, 0.60, 1.00, 0.8], // Connection - steel blue
    [1.00, 0.80, 0.40, 0.8], // Block Auth/Import - amber/gold
    [0.80, 0.40, 1.00, 0.8], // Block Distribution - purple
    [1.00, 0.40, 0.40, 0.8], // Safrole Tickets - red
    [0.20, 0.87, 0.67, 0.8], // Work Package - teal (pipeline start)
    [0.20, 0.73, 0.93, 0.8], // Guaranteeing - bright blue (pipeline mid)
    [0.93, 0.87, 0.20, 0.8], // Availability - yellow (pipeline end)
    [0.93, 0.47, 0.20, 0.8], // Bundle Recovery - orange
    [0.80, 0.40, 0.47, 0.8], // Segment Recovery - rose
    [0.80, 0.80, 0.80, 0.8], // Preimages - light gray
    [0.50, 0.50, 0.50, 0.8], // Meta - gray
];

/// Accessible: Okabe-Ito palette, safe for deuteranopia/protanopia/tritanopia
const ACCESSIBLE_COLORS: [[f32; 4]; 12] = [
    [0.34, 0.71, 0.91, 0.8], // Status - sky blue
    [0.00, 0.45, 0.70, 0.8], // Connection - blue
    [0.90, 0.62, 0.00, 0.8], // Block Auth/Import - orange
    [0.94, 0.89, 0.26, 0.8], // Block Distribution - yellow
    [0.80, 0.47, 0.65, 0.8], // Safrole Tickets - reddish purple
    [0.00, 0.62, 0.45, 0.8], // Work Package - bluish green
    [0.34, 0.71, 0.91, 0.8], // Guaranteeing - sky blue (brighter)
    [0.84, 0.37, 0.00, 0.8], // Availability - vermillion
    [0.53, 0.13, 0.33, 0.8], // Bundle Recovery - wine
    [0.60, 0.60, 0.20, 0.8], // Segment Recovery - olive
    [0.73, 0.73, 0.73, 0.8], // Preimages - light gray
    [0.53, 0.53, 0.53, 0.8], // Meta - gray
];

/// Pipeline: cool-to-warm gradient encoding lifecycle stage, infrastructure grayed out
const PIPELINE_COLORS: [[f32; 4]; 12] = [
    [0.47, 0.47, 0.47, 0.8], // Status - neutral gray
    [0.53, 0.53, 0.53, 0.8], // Connection - neutral gray
    [0.67, 0.67, 0.67, 0.8], // Block Auth/Import - light gray
    [0.60, 0.60, 0.60, 0.8], // Block Distribution - mid gray
    [0.73, 0.60, 0.80, 0.8], // Safrole Tickets - lavender
    [0.20, 0.40, 0.80, 0.8], // Work Package - deep blue (lifecycle start)
    [0.20, 0.67, 0.47, 0.8], // Guaranteeing - green (early-mid)
    [0.80, 0.80, 0.20, 0.8], // Availability - yellow (mid)
    [0.93, 0.47, 0.20, 0.8], // Bundle Recovery - orange (late)
    [0.87, 0.27, 0.27, 0.8], // Segment Recovery - red (recovery)
    [0.40, 0.40, 0.40, 0.8], // Preimages - dark gray
    [0.33, 0.33, 0.33, 0.8], // Meta - dark gray
];

/// Monochrome: single cyan hue, luminance differentiation only
const MONOCHROME_COLORS: [[f32; 4]; 12] = [
    [0.20, 0.40, 0.40, 0.8], // Status - darkest
    [0.27, 0.47, 0.47, 0.8], // Connection
    [0.33, 0.53, 0.53, 0.8], // Block Auth/Import
    [0.27, 0.60, 0.60, 0.8], // Block Distribution
    [0.33, 0.67, 0.67, 0.8], // Safrole Tickets
    [0.40, 0.80, 0.80, 0.8], // Work Package - bright (most important)
    [0.47, 0.87, 0.87, 0.8], // Guaranteeing
    [0.53, 0.93, 0.93, 0.8], // Availability - brightest
    [0.33, 0.67, 0.67, 0.8], // Bundle Recovery
    [0.27, 0.60, 0.60, 0.8], // Segment Recovery
    [0.27, 0.47, 0.47, 0.8], // Preimages
    [0.20, 0.40, 0.40, 0.8], // Meta - darkest
];

/// Backward-compatible alias
#[allow(dead_code)]
pub const CATEGORY_COLORS: [[f32; 4]; 12] = VIVID_COLORS;

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h * 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (r1 + m, g1 + m, b1 + m)
}

/// Event filter bitfield (256 bits = 8 x u32), matches shader's array<vec4<u32>, 2>
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct FilterBitfield {
    pub words: [u32; 8],
}

impl FilterBitfield {
    /// Convert from [u64; 4] bitfield used by DirectedEventBuffer
    pub fn from_u64_bitfield(bits: &[u64; 4]) -> Self {
        let mut words = [0u32; 8];
        for (i, &qword) in bits.iter().enumerate() {
            words[i * 2] = qword as u32;
            words[i * 2 + 1] = (qword >> 32) as u32;
        }
        Self { words }
    }

    /// All bits set (all event types enabled)
    pub fn all_enabled() -> Self {
        Self { words: [u32::MAX; 8] }
    }
}

/// GPU renderer for the validators ring.
/// Renders directly into egui's render pass (no intermediate texture).
pub struct RingRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    color_lut_buffer: wgpu::Buffer,
    filter_buffer: wgpu::Buffer,
    instance_buffers: Vec<wgpu::Buffer>,
    buffer_counts: Vec<u32>,

    // Incremental upload tracking
    gpu_write_head: usize,
    total_instances: u32,
}

impl RingRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ring_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ring_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ring_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ring_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuParticle>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32, // source_index
                        },
                        wgpu::VertexAttribute {
                            offset: 4,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32, // target_index
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32, // birth_time
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32, // travel_duration
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32, // event_type
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32, // curve_seed
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            // NOTE: Using TriangleList with 6 vertices/instance for round, sizable particles.
            // If performance is too slow, switch to PointList (1 vertex/instance, ~6x less work)
            // by changing topology to PointList and draw(0..1, ..) in paint(), but points are
            // fixed-size (1px) and cannot be rounded or resized via the shader.
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ring_uniforms"),
            contents: bytemuck::bytes_of(&Uniforms::default()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });

        let color_lut_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ring_color_lut"),
            contents: bytemuck::bytes_of(&ColorLut::default()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });

        let filter_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ring_event_filter"),
            contents: bytemuck::bytes_of(&FilterBitfield::all_enabled()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });

        // Create multiple instance buffers
        let instance_buffers: Vec<wgpu::Buffer> = (0..NUM_BUFFERS)
            .map(|i| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("ring_instances_{}", i)),
                    size: (BUFFER_CAPACITY * std::mem::size_of::<GpuParticle>()) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
                    mapped_at_creation: false,
                })
            })
            .collect();

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ring_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: color_lut_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: filter_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            color_lut_buffer,
            filter_buffer,
            instance_buffers,
            buffer_counts: vec![0; NUM_BUFFERS],
            gpu_write_head: 0,
            total_instances: 0,
        }
    }

    pub fn reset(&mut self) {
        self.gpu_write_head = 0;
        self.total_instances = 0;
        self.buffer_counts.fill(0);
    }

    /// Upload new particles, uniforms, and filter to GPU buffers.
    /// Particles are appended incrementally using a circular buffer.
    pub fn upload_data(
        &mut self,
        queue: &wgpu::Queue,
        new_particles: &[GpuParticle],
        uniforms: &Uniforms,
        filter: &FilterBitfield,
        color_lut: &ColorLut,
    ) {
        // Upload new particles
        if !new_particles.is_empty() {
            let particle_size = std::mem::size_of::<GpuParticle>();
            let mut remaining = new_particles;
            let mut write_pos = self.gpu_write_head;

            while !remaining.is_empty() {
                let buffer_idx = write_pos / BUFFER_CAPACITY;
                let offset_in_buffer = write_pos % BUFFER_CAPACITY;
                let space_in_buffer = BUFFER_CAPACITY - offset_in_buffer;
                let to_write = remaining.len().min(space_in_buffer);

                let (chunk, rest) = remaining.split_at(to_write);
                remaining = rest;

                let byte_offset = (offset_in_buffer * particle_size) as u64;
                queue.write_buffer(
                    &self.instance_buffers[buffer_idx % NUM_BUFFERS],
                    byte_offset,
                    bytemuck::cast_slice(chunk),
                );

                let new_end = offset_in_buffer + to_write;
                self.buffer_counts[buffer_idx % NUM_BUFFERS] =
                    self.buffer_counts[buffer_idx % NUM_BUFFERS].max(new_end as u32);

                write_pos = (write_pos + to_write) % MAX_INSTANCES;
            }

            self.gpu_write_head = write_pos;
            self.total_instances =
                (self.total_instances + new_particles.len() as u32).min(MAX_INSTANCES as u32);

            if self.total_instances >= MAX_INSTANCES as u32 {
                for count in &mut self.buffer_counts {
                    *count = BUFFER_CAPACITY as u32;
                }
            }
        }

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
        queue.write_buffer(&self.filter_buffer, 0, bytemuck::bytes_of(filter));
        queue.write_buffer(&self.color_lut_buffer, 0, bytemuck::bytes_of(color_lut));
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn instance_buffers(&self) -> &[wgpu::Buffer] {
        &self.instance_buffers
    }

    pub fn buffer_counts(&self) -> &[u32] {
        &self.buffer_counts
    }
}

/// Callback for egui integration
pub struct RingCallback {
    pub new_particles: Arc<Vec<GpuParticle>>,
    pub uniforms: Uniforms,
    pub filter: FilterBitfield,
    pub color_lut: ColorLut,
    pub reset: bool,
}

impl egui_wgpu::CallbackTrait for RingCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(renderer): Option<&mut RingRenderer> = callback_resources.get_mut() else {
            return vec![];
        };

        if self.reset {
            renderer.reset();
        }

        renderer.upload_data(queue, &self.new_particles, &self.uniforms, &self.filter, &self.color_lut);
        vec![]
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(renderer): Option<&RingRenderer> = callback_resources.get() else {
            return;
        };

        render_pass.set_pipeline(renderer.pipeline());
        render_pass.set_bind_group(0, renderer.bind_group(), &[]);

        for (i, buffer) in renderer.instance_buffers().iter().enumerate() {
            let count = renderer.buffer_counts()[i];
            if count > 0 {
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..96, 0..count);
            }
        }
    }
}
