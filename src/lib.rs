use std::sync::Arc;

use wgpu::CurrentSurfaceTexture;
use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow::Poll, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

const SHADER: &[u8] = include_bytes!(env!("shader.spv"));

pub fn run() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}

struct Gpu {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
}

#[derive(Default)]
struct App {
    gpu: Option<Gpu>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("rust-gpu triangle")
                    .with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
            )
            .unwrap(),
        );
        self.gpu = Some(pollster::block_on(init(window, el)));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        ..
                    },
                ..
            }
            | WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                gpu.config.width = size.width;
                gpu.config.height = size.height;
                gpu.surface.configure(&gpu.device, &gpu.config);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() {
                    CurrentSurfaceTexture::Success(frame) => frame,
                    CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => {
                        // Try again later
                        if let Some(gpu) = &self.gpu {
                            gpu.window.request_redraw();
                        }
                        return;
                    }
                    CurrentSurfaceTexture::Suboptimal(texture) => {
                        drop(texture);

                        if let Some(gpu) = &self.gpu {
                            gpu.surface.configure(&gpu.device, &gpu.config);
                            gpu.window.request_redraw();
                        }

                        return;
                    }
                    CurrentSurfaceTexture::Outdated => {
                        if let Some(gpu) = &self.gpu {
                            gpu.surface.configure(&gpu.device, &gpu.config);
                            gpu.window.request_redraw();
                        }
                        return;
                    }
                    CurrentSurfaceTexture::Validation => {
                        unreachable!("No error scope registered, so validation errors will panic")
                    }
                    CurrentSurfaceTexture::Lost => {
                        if let Some(gpu) = &mut self.gpu {
                            gpu.surface = gpu.instance.create_surface(gpu.window.clone()).unwrap();
                            gpu.surface.configure(&gpu.device, &gpu.config);
                            gpu.window.request_redraw();
                        }
                        return;
                    }
                };
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = gpu
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
                    rpass.set_pipeline(&gpu.pipeline);
                    rpass.draw(0..3, 0..1);
                }
                gpu.queue.submit(Some(encoder.finish()));
                frame.present();
            }
            _ => {}
        }
    }
}

async fn init(window: Arc<Window>, el: &ActiveEventLoop) -> Gpu {
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

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
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
    }
}
