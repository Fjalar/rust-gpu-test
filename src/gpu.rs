use std::sync::Arc;

use wgpu::{CurrentSurfaceTexture, util::DeviceExt};
use winit::{event_loop::ActiveEventLoop, window::Window};

const SHADER: &[u8] = include_bytes!(env!("shader.spv"));

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) t: f32,
}

pub(crate) struct Gpu {
    pub(crate) instance: wgpu::Instance,
    pub(crate) window: Arc<Window>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) camera_buffer: wgpu::Buffer,
    pub(crate) camera_bind_group: wgpu::BindGroup,
    pub(crate) camera_uniform: CameraUniform,
    pub(crate) start_timestamp: std::time::Instant,
}
impl Gpu {
    pub(crate) fn render(&mut self) {
        self.camera_uniform.t = self.start_timestamp.elapsed().as_secs_f32();
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
        let frame = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) => frame,
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => {
                // Try again later
                self.window.request_redraw();
                return;
            }
            CurrentSurfaceTexture::Suboptimal(texture) => {
                drop(texture);

                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();

                return;
            }
            CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return;
            }
            CurrentSurfaceTexture::Validation => {
                unreachable!("No error scope registered, so validation errors will panic")
            }
            CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone()).unwrap();
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        frame.present();
    }
}

pub(crate) async fn init(window: Arc<Window>, el: &ActiveEventLoop) -> Gpu {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
        Box::new(el.owned_display_handle()),
    ));
    let surface = instance.create_surface(window.clone()).unwrap();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .expect("no adapter");
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: Default::default(),
            experimental_features: Default::default(),
        })
        .await
        .expect("no device");

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader"),
        source: wgpu::util::make_spirv(SHADER),
    });

    let size = window.inner_size();
    let mut config = surface
        .get_default_config(&adapter, size.width.max(1), size.height.max(1))
        .expect("surface unsupported");
    config.present_mode = wgpu::PresentMode::AutoVsync;
    surface.configure(&device, &config);

    let camera_uniform = CameraUniform {
        x: size.width,
        y: size.height,
        t: 0.0,
    };

    let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Camera Buffer"),
        contents: bytemuck::cast_slice(&[camera_uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let camera_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("camera_bind_group_layout"),
        });

    let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &camera_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }],
        label: Some("camera_bind_group"),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&camera_bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("main_vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("main_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });

    Gpu {
        instance,
        window,
        surface,
        device,
        queue,
        config,
        pipeline,
        camera_bind_group,
        camera_buffer,
        camera_uniform,
        start_timestamp: std::time::Instant::now(),
    }
}
