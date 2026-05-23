use std::sync::Arc;

use wgpu::CurrentSurfaceTexture;
use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

use crate::gpu::Gpu;

#[derive(Default)]
pub(crate) struct App {
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
                    .with_title("rust-gpu-test")
                    .with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
            )
            .unwrap(),
        );
        self.gpu = Some(pollster::block_on(crate::gpu::init(window, el)));
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
