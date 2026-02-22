//! GPU instanced scatter renderer for Event Particles graph.
//!
//! Renders to an off-screen texture, composited into egui via painter.image().
//! Adapted from render-perf-comparison/instanced/ with event_type coloring.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu::{self, util::DeviceExt};
use std::sync::Arc;

use crate::vring::{ColorLut, FilterBitfield};

const BUFFER_CAPACITY: usize = 2_500_000; // 2.5M particles per buffer
const NUM_BUFFERS: usize = 2; // 2 buffers = 5M particles total
const MAX_INSTANCES: usize = BUFFER_CAPACITY * NUM_BUFFERS;

/// GPU-compatible scatter particle (12 bytes)
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ScatterParticle {
    pub node_index: f32,
    pub birth_time: f32,
    pub event_type: f32,
}

/// Uniform buffer for scatter shader
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ScatterUniforms {
    pub x_range: [f32; 2],
    pub y_range: [f32; 2],
    pub point_size: f32,
    pub current_time: f32,
    pub max_age: f32,
    pub aspect_ratio: f32,
    pub speed_factor: f32,
    pub _pad: [f32; 3],
}

/// GPU scatter renderer with off-screen texture
pub struct ScatterRenderer {
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

    // Render target texture
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
}

impl ScatterRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scatter_bind_group_layout"),
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
            label: Some("scatter_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scatter_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ScatterParticle>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32, // node_index
                        },
                        wgpu::VertexAttribute {
                            offset: 4,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32, // birth_time
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32, // event_type
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
            // TriangleList with 6 vertices per instance (2 triangles = 1 quad).
            // PointList point_size is capped at 1px on many GPUs, so we use
            // screen-space quads and clip to a circle in the fragment shader.
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let default_uniforms = ScatterUniforms {
            x_range: [0.0, 100.0],
            y_range: [0.0, 10.0],
            point_size: 0.005,
            current_time: 0.0,
            max_age: 10.0,
            aspect_ratio: 1.0,
            speed_factor: 1.0,
            _pad: [0.0; 3],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scatter_uniforms"),
            contents: bytemuck::bytes_of(&default_uniforms),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });

        let color_lut_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scatter_color_lut"),
            contents: bytemuck::bytes_of(&ColorLut::default()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });

        let filter_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scatter_event_filter"),
            contents: bytemuck::bytes_of(&FilterBitfield::all_enabled()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });

        let instance_buffers: Vec<wgpu::Buffer> = (0..NUM_BUFFERS)
            .map(|i| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("scatter_instances_{}", i)),
                    size: (BUFFER_CAPACITY * std::mem::size_of::<ScatterParticle>()) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
                    mapped_at_creation: false,
                })
            })
            .collect();

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scatter_bind_group"),
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

        let (texture, texture_view) = Self::create_texture(device, target_format, 1, 1);

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
            texture,
            texture_view,
            target_format,
            width: 1,
            height: 1,
        }
    }

    pub fn reset(&mut self) {
        self.gpu_write_head = 0;
        self.total_instances = 0;
        self.buffer_counts.fill(0);
    }

    fn create_texture(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scatter_render_texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn create_view(&self) -> wgpu::TextureView {
        self.texture
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Upload new particles incrementally and render to off-screen texture
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_incremental(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dimensions: [u32; 2],
        new_particles: &[ScatterParticle],
        uniforms: &ScatterUniforms,
        filter: &FilterBitfield,
        color_lut: &ColorLut,
    ) {
        // Resize texture if needed
        if dimensions[0] != self.width || dimensions[1] != self.height {
            self.width = dimensions[0].max(1);
            self.height = dimensions[1].max(1);
            let (texture, view) =
                Self::create_texture(device, self.target_format, self.width, self.height);
            self.texture = texture;
            self.texture_view = view;
        }

        // Upload new particles across multiple buffers
        if !new_particles.is_empty() {
            let particle_size = std::mem::size_of::<ScatterParticle>();
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

        // Render to off-screen texture
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scatter_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);

            for (i, buffer) in self.instance_buffers.iter().enumerate() {
                let count = self.buffer_counts[i];
                if count > 0 {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.draw(0..6, 0..count);
                }
            }
        }
    }
}

/// Callback for egui integration — renders to off-screen texture in prepare()
pub struct ScatterCallback {
    pub new_particles: Arc<Vec<ScatterParticle>>,
    pub uniforms: ScatterUniforms,
    pub filter: FilterBitfield,
    pub color_lut: ColorLut,
    pub rect: egui::Rect,
    pub reset: bool,
}

impl egui_wgpu::CallbackTrait for ScatterCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(renderer): Option<&mut ScatterRenderer> = callback_resources.get_mut() else {
            return vec![];
        };

        if self.reset {
            renderer.reset();
        }

        renderer.prepare_incremental(
            device,
            queue,
            encoder,
            [self.rect.width() as u32, self.rect.height() as u32],
            &self.new_particles,
            &self.uniforms,
            &self.filter,
            &self.color_lut,
        );
        vec![]
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        _render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        // Rendering done in prepare() to own texture, not to egui's render pass
    }
}
