use std::sync::Arc;

use wgpu::BindGroupDescriptor;
use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

use crate::gpu::{self, Gpu};

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
                gpu.display_uniform = [size.width, size.height];
                gpu.queue.write_buffer(
                    &gpu.display_buffer,
                    0,
                    bytemuck::cast_slice(&gpu.display_uniform),
                );
                let new_raytracing_view =
                    &gpu::create_storage_texture(&gpu.device, gpu.config.width, gpu.config.height);
                gpu.raytracing_view_bind_group =
                    gpu.device.create_bind_group(&BindGroupDescriptor {
                        label: Some("Raytracing view bind group"),
                        layout: &gpu.raytracing_view_bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(new_raytracing_view),
                        }],
                    });
                gpu.blit_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Blit bind group"),
                    layout: &gpu.blit_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(new_raytracing_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&gpu.sampler),
                        },
                    ],
                });
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                gpu.params.t = gpu.start_timestamp.elapsed().as_secs_f32();
                gpu.queue
                    .write_buffer(&gpu.params_buffer, 0, bytemuck::cast_slice(&[gpu.params]));
                gpu.window.request_redraw();
                gpu.render();
            }
            _ => {}
        }
    }
}
