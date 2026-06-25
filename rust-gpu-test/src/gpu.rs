use std::sync::Arc;

use wgpu::{
    BindGroup, BindGroupLayoutDescriptor, CurrentSurfaceTexture, ShaderStages, util::DeviceExt,
};
use winit::{event_loop::ActiveEventLoop, window::Window};

use crate::params::Params;

pub(crate) struct Gpu {
    pub(crate) instance: wgpu::Instance,
    pub(crate) window: Arc<Window>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) blit_pipeline: wgpu::RenderPipeline,
    pub(crate) blit_bind_group: BindGroup,
    pub(crate) compute_pipeline: wgpu::ComputePipeline,
    pub(crate) raytracing_view_bind_group: BindGroup,
    pub(crate) display_uniform: [u32; 2],
    pub(crate) display_buffer: wgpu::Buffer,
    pub(crate) display_bind_group: BindGroup,
    pub(crate) params: Params,
    pub(crate) params_buffer: wgpu::Buffer,
    pub(crate) params_bind_group: wgpu::BindGroup,
    pub(crate) start_timestamp: std::time::Instant,
}
impl Gpu {
    pub(crate) fn render(&mut self) {
        self.params.t = self.start_timestamp.elapsed().as_secs_f32();
        self.queue
            .write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[self.params]));
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
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, Some(&self.raytracing_view_bind_group), &[]);
            compute_pass.set_bind_group(1, Some(&self.display_bind_group), &[]);
            compute_pass.dispatch_workgroups(
                self.display_uniform[0] / 16,
                self.display_uniform[1] / 16,
                1,
            );
        }
        {
            let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
            blit_pass.set_pipeline(&self.blit_pipeline);
            blit_pass.set_bind_group(0, &self.blit_bind_group, &[]);
            blit_pass.draw(0..3, 0..1);
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
            required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: Default::default(),
            experimental_features: Default::default(),
        })
        .await
        .expect("no device");

    let module = device.create_shader_module(wgpu::include_spirv!(env!("SHADER_SPV_PATH")));

    let size = window.inner_size();
    let mut config = surface
        .get_default_config(&adapter, size.width.max(1), size.height.max(1))
        .expect("surface unsupported");
    config.present_mode = wgpu::PresentMode::AutoVsync;
    surface.configure(&device, &config);

    // Begin: target
    let hdr_format = wgpu::TextureFormat::Rgba16Float;

    let raytracing_target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Raytracing target"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: hdr_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[hdr_format],
    });

    let raytracing_view = raytracing_target.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Raytracing view"),
        format: Some(hdr_format),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: None,
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: None,
        base_array_layer: 0,
        array_layer_count: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Raytracing sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    // End: target

    // Begin: display
    let display_uniform = [size.width, size.height];

    let display_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Display Buffer"),
        contents: bytemuck::cast_slice(&display_uniform),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let display_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Display bind group layout"),
        });

    let display_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &display_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: display_buffer.as_entire_binding(),
        }],
        label: Some("Display bind group"),
    });
    // End: display

    // Begin: params
    let params = Params { t: 0.0 };

    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Params Buffer"),
        contents: bytemuck::cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let params_bind_group_layout =
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
            label: Some("params_bind_group_layout"),
        });

    let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &params_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: params_buffer.as_entire_binding(),
        }],
        label: Some("params_bind_group"),
    });
    // End: params

    // Begin: compute
    let raytracing_view_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::ReadWrite,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
            label: Some("Raytracing view bind group layout"),
        });

    let raytracing_view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Raytracing view bind group"),
        layout: &raytracing_view_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&raytracing_view),
        }],
    });

    let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute pipeline layout"),
        bind_group_layouts: &[
            Some(&raytracing_view_bind_group_layout),
            Some(&display_bind_group_layout),
        ],
        immediate_size: 0,
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute pipeline"),
        layout: Some(&compute_pipeline_layout),
        module: &module,
        entry_point: Some("main_cs"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    // End: compute

    // Begin: blit
    let blit_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Blit bind group layout descriptor"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    });
    let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Blit bind group"),
        layout: &blit_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&raytracing_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Blit pipeline layout"),
        bind_group_layouts: &[Some(&blit_bind_group_layout)],
        immediate_size: 0,
    });
    let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Blit pipeline"),
        layout: Some(&blit_pipeline_layout),
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

    // End: blit

    Gpu {
        instance,
        window,
        surface,
        device,
        queue,
        config,
        blit_pipeline,
        blit_bind_group,
        raytracing_view_bind_group,
        compute_pipeline,
        display_uniform,
        display_buffer,
        display_bind_group,
        params,
        params_buffer,
        params_bind_group,
        start_timestamp: std::time::Instant::now(),
    }
}
