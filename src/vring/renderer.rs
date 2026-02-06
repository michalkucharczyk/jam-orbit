//! GPU renderer for validators ring visualization
//!
//! Renders directed events as particles traveling between validators
//! arranged on a circle using GPU instancing.

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
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            current_time: 0.0,
            num_validators: 1024.0,
            aspect_ratio: 1.0,
            point_size: 2.0,
        }
    }
}

/// Color lookup table for event categories
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ColorLut {
    pub colors: [[f32; 4]; 16],
}

impl Default for ColorLut {
    fn default() -> Self {
        Self {
            colors: [
                [0.5, 0.5, 0.5, 0.8],   // 0: Meta - gray
                [0.4, 0.8, 0.4, 0.8],   // 1: Status - green
                [0.4, 0.6, 1.0, 0.8],   // 2: Connection - blue
                [1.0, 0.8, 0.4, 0.8],   // 3: Block auth - orange
                [0.8, 0.4, 1.0, 0.8],   // 4: Block dist - purple
                [1.0, 0.4, 0.4, 0.8],   // 5: Tickets - red
                [0.4, 1.0, 0.8, 0.8],   // 6: Work Package - cyan
                [0.2, 0.8, 0.7, 0.8],   // 7: Guaranteeing - teal
                [1.0, 1.0, 0.4, 0.8],   // 8: Availability - yellow
                [1.0, 0.6, 0.6, 0.8],   // 9: Bundle - pink
                [0.6, 0.8, 1.0, 0.8],   // 10: Segment - light blue
                [0.8, 0.8, 0.8, 0.8],   // 11: Preimage - light gray
                [0.7, 0.7, 0.7, 0.8],   // 12: Reserved
                [0.7, 0.7, 0.7, 0.8],   // 13: Reserved
                [0.7, 0.7, 0.7, 0.8],   // 14: Reserved
                [1.0, 1.0, 1.0, 0.8],   // 15: Unknown - white
            ],
        }
    }
}

/// GPU renderer for the validators ring
pub struct RingRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    color_lut_buffer: wgpu::Buffer,
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
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
            ],
        });

        let (texture, texture_view) = Self::create_texture(device, target_format, 1, 1);

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            color_lut_buffer,
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
            label: Some("ring_render_texture"),
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

    #[allow(dead_code)]
    pub fn create_view(&self) -> wgpu::TextureView {
        self.texture
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Upload new particles and render to texture
    pub fn prepare_incremental(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dimensions: [u32; 2],
        new_particles: &[GpuParticle],
        uniforms: &Uniforms,
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

        // Render to texture
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ring_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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
                    render_pass.draw(0..1, 0..count);
                }
            }
        }
    }
}

/// Callback for egui integration
pub struct RingCallback {
    pub new_particles: Arc<Vec<GpuParticle>>,
    pub uniforms: Uniforms,
    pub rect: egui::Rect,
    pub reset: bool,
}

impl egui_wgpu::CallbackTrait for RingCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer: &mut RingRenderer = callback_resources.get_mut().unwrap();

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
        );
        vec![]
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        _render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        // We render to our own texture in prepare(), not to the egui render pass
    }
}
