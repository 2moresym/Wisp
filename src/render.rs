use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

use crate::camera::{CameraRig, Mat4};
use crate::physics::{Fly, Food, ARENA_HALF_SIZE};

const STRIDE: u64 = 256;
const OBJECT_SLOTS: u64 = 12;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    p: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Object {
    vp: [f32; 16],
    model: [f32; 16],
    color: [f32; 4],
}

const CUBE_V: &[Vertex] = &[
    Vertex {
        p: [-0.5, -0.5, -0.5],
    },
    Vertex {
        p: [0.5, -0.5, -0.5],
    },
    Vertex {
        p: [0.5, 0.5, -0.5],
    },
    Vertex {
        p: [-0.5, 0.5, -0.5],
    },
    Vertex {
        p: [-0.5, -0.5, 0.5],
    },
    Vertex {
        p: [0.5, -0.5, 0.5],
    },
    Vertex { p: [0.5, 0.5, 0.5] },
    Vertex {
        p: [-0.5, 0.5, 0.5],
    },
];
const CUBE_I: &[u16] = &[
    0, 1, 2, 2, 3, 0, 4, 6, 5, 6, 7, 4, 0, 4, 7, 7, 3, 0, 1, 5, 6, 6, 2, 1, 3, 2, 6, 6, 7, 3, 0, 1,
    5, 5, 4, 0,
];

const FOOD_V: &[Vertex] = &[
    Vertex { p: [0.0, 0.7, 0.0] },
    Vertex { p: [0.7, 0.0, 0.0] },
    Vertex { p: [0.0, 0.0, 0.7] },
    Vertex {
        p: [-0.7, 0.0, 0.0],
    },
    Vertex {
        p: [0.0, 0.0, -0.7],
    },
    Vertex {
        p: [0.0, -0.7, 0.0],
    },
];
const FOOD_I: &[u16] = &[
    0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1, 5, 2, 1, 5, 3, 2, 5, 4, 3, 5, 1, 4,
];

const SHADER: &str = r#"
struct Object { vp: mat4x4<f32>, model: mat4x4<f32>, color: vec4<f32>, };
@group(0) @binding(0) var<uniform> o: Object;
struct Out { @builtin(position) p: vec4<f32>, };
@vertex fn vs(@location(0) p: vec3<f32>) -> Out {
  var x: Out;
  x.p = o.vp * o.model * vec4<f32>(p, 1.0);
  return x;
}
@fragment fn fs() -> @location(0) vec4<f32> { return o.color; }
"#;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    depth: wgpu::TextureView,
    cube_v: wgpu::Buffer,
    cube_i: wgpu::Buffer,
    food_v: wgpu::Buffer,
    food_i: wgpu::Buffer,
    ub: wgpu::Buffer,
    bg: wgpu::BindGroup,
    camera: CameraRig,
}

#[derive(Debug)]
pub enum RenderError {
    AdapterUnavailable,
    Device(String),
    Surface(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdapterUnavailable => write!(f, "no compatible GPU adapter"),
            Self::Device(error) => write!(f, "GPU device: {error}"),
            Self::Surface(error) => write!(f, "surface: {error}"),
        }
    }
}
impl std::error::Error for RenderError {}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, RenderError> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window)
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(RenderError::AdapterUnavailable)?;
        let info = adapter.get_info();
        log::info!("Wisp GPU: {} ({:?})", info.name, info.backend);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("wisp"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                },
                None,
            )
            .await
            .map_err(|e| RenderError::Device(e.to_string()))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wisp-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let ub = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("object-uniforms"),
            size: STRIDE * OBJECT_SLOTS,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object-layout"),
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
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("object-bind"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ub.as_entire_binding(),
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
                entry_point: "vs",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    }],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });

        let depth = Self::make_depth(&device, config.width, config.height);
        let cube_v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube-v"),
            contents: bytemuck::cast_slice(CUBE_V),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let cube_i = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube-i"),
            contents: bytemuck::cast_slice(CUBE_I),
            usage: wgpu::BufferUsages::INDEX,
        });
        let food_v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("food-v"),
            contents: bytemuck::cast_slice(FOOD_V),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let food_i = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("food-i"),
            contents: bytemuck::cast_slice(FOOD_I),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            depth,
            cube_v,
            cube_i,
            food_v,
            food_i,
            ub,
            bg,
            camera: CameraRig::default(),
        })
    }

    fn make_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
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
        self.surface.configure(&self.device, &self.config);
        self.depth = Self::make_depth(&self.device, size.width, size.height);
    }

    fn write(&self, slot: u32, vp: Mat4, model: Mat4, color: [f32; 4]) {
        let object = Object {
            vp: vp.to_cols_array(),
            model: model.to_cols_array(),
            color,
        };
        self.queue
            .write_buffer(&self.ub, slot as u64 * STRIDE, bytemuck::bytes_of(&object));
    }

    pub fn render(&mut self, fly: &Fly, food: &Food, dt: f32) -> Result<(), wgpu::SurfaceError> {
        self.camera.update(fly.position, fly.forward(), dt);
        let aspect = self.size.width as f32 / self.size.height.max(1) as f32;
        let vp = self.camera.camera(aspect).view_projection();
        self.write(
            0,
            vp,
            Mat4::translation(0.0, -0.3, 0.0).mul(Mat4::scale(16.0, 0.5, 16.0)),
            [0.08, 0.11, 0.09, 1.0],
        );
        self.write(
            1,
            vp,
            Mat4::translation(0.0, 0.75, -ARENA_HALF_SIZE).mul(Mat4::scale(16.0, 1.5, 0.35)),
            [0.15, 0.18, 0.17, 1.0],
        );
        self.write(
            2,
            vp,
            Mat4::translation(0.0, 0.75, ARENA_HALF_SIZE).mul(Mat4::scale(16.0, 1.5, 0.35)),
            [0.15, 0.18, 0.17, 1.0],
        );
        self.write(
            3,
            vp,
            Mat4::translation(-ARENA_HALF_SIZE, 0.75, 0.0).mul(Mat4::scale(0.35, 1.5, 16.0)),
            [0.15, 0.18, 0.17, 1.0],
        );
        self.write(
            4,
            vp,
            Mat4::translation(ARENA_HALF_SIZE, 0.75, 0.0).mul(Mat4::scale(0.35, 1.5, 16.0)),
            [0.15, 0.18, 0.17, 1.0],
        );

        let fly_model = Mat4::translation(fly.position[0], fly.position[1], fly.position[2])
            .mul(Mat4::rotation_y(fly.rotation_y));
        self.write(
            5,
            vp,
            fly_model.mul(Mat4::scale(0.38, 0.30, 0.75)),
            [0.18, 0.09, 0.04, 1.0],
        );
        self.write(
            6,
            vp,
            fly_model
                .mul(Mat4::translation(0.0, 0.0, 0.48))
                .mul(Mat4::scale(0.30, 0.24, 0.48)),
            [0.10, 0.05, 0.025, 1.0],
        );
        self.write(
            7,
            vp,
            fly_model
                .mul(Mat4::translation(0.0, 0.02, -0.48))
                .mul(Mat4::scale(0.34, 0.27, 0.52)),
            [0.24, 0.11, 0.04, 1.0],
        );
        self.write(
            8,
            vp,
            fly_model
                .mul(Mat4::translation(0.0, 0.0, 0.88))
                .mul(Mat4::scale(0.25, 0.23, 0.25)),
            [0.04, 0.025, 0.015, 1.0],
        );
        self.write(
            9,
            vp,
            fly_model
                .mul(Mat4::translation(-0.43, 0.16, 0.02))
                .mul(Mat4::rotation_z(-0.12))
                .mul(Mat4::scale(0.75, 0.035, 0.48)),
            [0.42, 0.45, 0.38, 0.65],
        );
        self.write(
            10,
            vp,
            fly_model
                .mul(Mat4::translation(0.43, 0.16, 0.02))
                .mul(Mat4::rotation_z(0.12))
                .mul(Mat4::scale(0.75, 0.035, 0.48)),
            [0.42, 0.45, 0.38, 0.65],
        );
        self.write(
            11,
            vp,
            Mat4::translation(food.position[0], food.position[1], food.position[2])
                .mul(Mat4::scale(0.6, 0.6, 0.6)),
            [0.95, 0.72, 0.10, 1.0],
        );

        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arena"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.015,
                            g: 0.020,
                            b: 0.028,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
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
            pass.set_vertex_buffer(0, self.cube_v.slice(..));
            pass.set_index_buffer(self.cube_i.slice(..), wgpu::IndexFormat::Uint16);
            for slot in 0..11u32 {
                pass.set_bind_group(0, &self.bg, &[slot * STRIDE as u32]);
                pass.draw_indexed(0..CUBE_I.len() as u32, 0, 0..1);
            }
            pass.set_vertex_buffer(0, self.food_v.slice(..));
            pass.set_index_buffer(self.food_i.slice(..), wgpu::IndexFormat::Uint16);
            pass.set_bind_group(0, &self.bg, &[11 * STRIDE as u32]);
            pass.draw_indexed(0..FOOD_I.len() as u32, 0, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
