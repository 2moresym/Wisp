//! Tiny forward 3D renderer for Wisp.
//!
//! The scene intentionally stays primitive: one cube mesh, one octahedron mesh,
//! one render pipeline, one dynamic uniform buffer and one depth texture. All
//! geometry is submitted in a single render pass with no textures or post FX.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::camera::{Camera, Mat4};
use crate::physics::{Food, Fly};

const UNIFORM_ALIGN: u64 = 256;
const OBJECT_SLOTS: u64 = 7;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

const CUBE_VERTICES: &[Vertex] = &[
    Vertex { position: [-0.5, -0.5, -0.5] },
    Vertex { position: [0.5, -0.5, -0.5] },
    Vertex { position: [0.5, 0.5, -0.5] },
    Vertex { position: [-0.5, 0.5, -0.5] },
    Vertex { position: [-0.5, -0.5, 0.5] },
    Vertex { position: [0.5, -0.5, 0.5] },
    Vertex { position: [0.5, 0.5, 0.5] },
    Vertex { position: [-0.5, 0.5, 0.5] },
];

const CUBE_INDICES: &[u16] = &[
    0, 1, 2, 2, 3, 0,
    4, 5, 6, 6, 7, 4,
    0, 4, 7, 7, 3, 0,
    1, 5, 6, 6, 2, 1,
    3, 2, 6, 6, 7, 3,
    0, 1, 5, 5, 4, 0,
];

const FOOD_VERTICES: &[Vertex] = &[
    Vertex { position: [0.0, 0.45, 0.0] },
    Vertex { position: [-0.45, 0.0, 0.0] },
    Vertex { position: [0.0, 0.0, -0.45] },
    Vertex { position: [0.45, 0.0, 0.0] },
    Vertex { position: [0.0, 0.0, 0.45] },
    Vertex { position: [0.0, -0.45, 0.0] },
];

const FOOD_INDICES: &[u16] = &[
    0, 2, 1, 0, 3, 2, 0, 4, 3, 0, 1, 4,
    5, 1, 2, 5, 2, 3, 5, 3, 4, 5, 4, 1,
];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

const SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VSOut {
    var out: VSOut;
    out.position = uniforms.view_proj * uniforms.model * vec4<f32>(position, 1.0);
    out.color = uniforms.color;
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    cube_vertex: wgpu::Buffer,
    cube_index: wgpu::Buffer,
    food_vertex: wgpu::Buffer,
    food_index: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    camera: Camera,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("surface creation failed: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "no compatible Vulkan/OpenGL adapter found".to_string())?;

        log::info!("Wisp GPU adapter: {}", adapter.get_info().name);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("wisp-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| format!("device creation failed: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let present_mode = caps
            .present_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::PresentMode::Fifo)
            .unwrap_or(caps.present_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth_view = create_depth_view(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wisp-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wisp-dynamic-uniforms"),
            size: UNIFORM_ALIGN * OBJECT_SLOTS,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wisp-uniform-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wisp-uniform-bind-group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: std::num::NonZeroU64::new(std::mem::size_of::<Uniforms>() as u64),
                },
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wisp-pipeline-layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wisp-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let cube_vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wisp-cube-vertices"),
            contents: bytemuck::cast_slice(CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let cube_index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wisp-cube-indices"),
            contents: bytemuck::cast_slice(CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let food_vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wisp-food-vertices"),
            contents: bytemuck::cast_slice(FOOD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let food_index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wisp-food-indices"),
            contents: bytemuck::cast_slice(FOOD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let camera = Camera {
            position: [0.0, 7.0, 10.0],
            target: [0.0, 0.0, 0.0],
            aspect: config.width as f32 / config.height as f32,
            fov_y: crate::camera::DEFAULT_FOV,
            near: 0.1,
            far: 100.0,
        };

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            cube_vertex,
            cube_index,
            food_vertex,
            food_index,
            uniform_buffer,
            uniform_bind_group,
            depth_view,
            camera,
        })
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.camera.aspect = size.width as f32 / size.height as f32;
        self.depth_view = create_depth_view(&self.device, &self.config);
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self, fly: &Fly, food: &Food) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("wisp-frame"),
            });

        let vp = self.camera.view_projection();
        let objects = [
            (
                Mat4::translation(0.0, -0.35, 0.0).mul(Mat4::scale(18.0, 0.3, 18.0)),
                [0.07, 0.10, 0.07, 1.0],
            ),
            (
                Mat4::translation(8.0, 0.475, 0.0).mul(Mat4::scale(0.25, 1.25, 16.0)),
                [0.10, 0.12, 0.10, 1.0],
            ),
            (
                Mat4::translation(-8.0, 0.475, 0.0).mul(Mat4::scale(0.25, 1.25, 16.0)),
                [0.10, 0.12, 0.10, 1.0],
            ),
            (
                Mat4::translation(0.0, 0.475, 8.0).mul(Mat4::scale(16.0, 1.25, 0.25)),
                [0.10, 0.12, 0.10, 1.0],
            ),
            (
                Mat4::translation(0.0, 0.475, -8.0).mul(Mat4::scale(16.0, 1.25, 0.25)),
                [0.10, 0.12, 0.10, 1.0],
            ),
            (
                Mat4::translation(fly.position[0], fly.position[1] + 0.18, fly.position[2])
                    .mul(Mat4::rotation_y(fly.rotation_y))
                    .mul(Mat4::scale(0.55, 0.35, 0.8)),
                [0.20, 0.72, 0.34, 1.0],
            ),
        ];

        for (slot, (model, color)) in objects.iter().enumerate() {
            let uniforms = Uniforms {
                view_proj: vp.m,
                model: model.m,
                color: *color,
            };
            self.queue.write_buffer(
                &self.uniform_buffer,
                slot as u64 * UNIFORM_ALIGN,
                bytemuck::bytes_of(&uniforms),
            );
        }

        let food_uniform = Uniforms {
            view_proj: vp.m,
            model: Mat4::translation(
                food.position[0],
                food.position[1] + 0.45,
                food.position[2],
            )
            .m,
            color: [0.95, 0.55, 0.06, 1.0],
        };
        self.queue.write_buffer(
            &self.uniform_buffer,
            6 * UNIFORM_ALIGN,
            bytemuck::bytes_of(&food_uniform),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wisp-arena-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.018,
                        g: 0.024,
                        b: 0.032,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[0]);
        pass.set_vertex_buffer(0, self.cube_vertex.slice(..));
        pass.set_index_buffer(self.cube_index.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);

        for slot in 1..6u32 {
            pass.set_bind_group(0, &self.uniform_bind_group, &[slot * UNIFORM_ALIGN as u32]);
            pass.set_vertex_buffer(0, self.cube_vertex.slice(..));
            pass.set_index_buffer(self.cube_index.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);
        }

        pass.set_bind_group(0, &self.uniform_bind_group, &[6 * UNIFORM_ALIGN as u32]);
        pass.set_vertex_buffer(0, self.food_vertex.slice(..));
        pass.set_index_buffer(self.food_index.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..FOOD_INDICES.len() as u32, 0, 0..1);

        drop(pass);
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

fn create_depth_view(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wisp-depth"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
