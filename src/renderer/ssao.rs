use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2};
use wgpu::util::DeviceExt;

const SSAO_KERNEL_SIZE: usize = 64;
const SSAO_NOISE_SIZE: usize = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SsaoParams {
    pub projection: [[f32; 4]; 4],
    pub inv_projection: [[f32; 4]; 4],
    pub noise_scale: [f32; 2],
    pub radius: f32,
    pub bias: f32,
    pub strength: f32,
    pub _padding: [f32; 3],
}

impl SsaoParams {
    pub fn new(projection: Mat4, screen_size: Vec2, radius: f32, bias: f32, strength: f32) -> Self {
        let inv_projection = projection.inverse();
        Self {
            projection: projection.to_cols_array_2d(),
            inv_projection: inv_projection.to_cols_array_2d(),
            noise_scale: [
                screen_size.x / SSAO_NOISE_SIZE as f32,
                screen_size.y / SSAO_NOISE_SIZE as f32,
            ],
            radius,
            bias,
            strength,
            _padding: [0.0; 3],
        }
    }
}

pub struct SsaoPass {
    pub params_buffer: wgpu::Buffer,
    pub kernel_buffer: wgpu::Buffer,
    pub noise_texture: wgpu::Texture,
    pub noise_view: wgpu::TextureView,
    pub noise_sampler: wgpu::Sampler,
    pub ssao_texture: wgpu::Texture,
    pub ssao_view: wgpu::TextureView,
    pub ssao_sampler: wgpu::Sampler,
    pub blur_texture: wgpu::Texture,
    pub blur_view: wgpu::TextureView,
    pub pipeline: wgpu::RenderPipeline,
    pub blur_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub blur_bind_group_layout: wgpu::BindGroupLayout,
}

impl SsaoPass {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) -> Self {
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SSAO Params Buffer"),
            contents: bytemuck::cast_slice(&[SsaoParams::new(
                Mat4::IDENTITY,
                Vec2::new(width as f32, height as f32),
                0.5,
                0.025,
                1.0,
            )]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Full-resolution SSAO avoids soft halos and edge swimming at 1440p
        // and ultrawide resolutions. The blur pass still suppresses noise.
        let ssao_width = width.max(1);
        let ssao_height = height.max(1);

        let (kernel, noise) = generate_kernel_and_noise();

        let kernel_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SSAO Kernel Buffer"),
            contents: bytemuck::cast_slice(&kernel),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let noise_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSAO Noise Texture"),
            size: wgpu::Extent3d {
                width: SSAO_NOISE_SIZE as u32,
                height: SSAO_NOISE_SIZE as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue_write_noise(queue, &noise_texture, &noise);
        let noise_view = noise_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSAO Noise Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        let create_r8_texture = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: ssao_width,
                    height: ssao_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };

        let ssao_texture = create_r8_texture("SSAO Texture");
        let ssao_view = ssao_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ssao_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSAO Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let blur_texture = create_r8_texture("SSAO Blur Texture");
        let blur_view = blur_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSAO Bind Group Layout"),
            entries: &[
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Kernel
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Depth sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // Noise texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Noise sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SSAO Blur Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSAO Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSAO Blur Pipeline Layout"),
            bind_group_layouts: &[Some(&blur_bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSAO Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/ssao.wgsl").into()),
        });

        let blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSAO Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/ssao_blur.wgsl").into(),
            ),
        });

        let create_fullscreen_pipeline =
            |label: &str,
             layout: &wgpu::PipelineLayout,
             shader: &wgpu::ShaderModule,
             target_format: wgpu::TextureFormat| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: shader,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: shader,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: target_format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview_mask: None,
                    cache: None,
                })
            };

        let pipeline = create_fullscreen_pipeline(
            "SSAO Pipeline",
            &pipeline_layout,
            &shader,
            wgpu::TextureFormat::R8Unorm,
        );
        let blur_pipeline = create_fullscreen_pipeline(
            "SSAO Blur Pipeline",
            &blur_pipeline_layout,
            &blur_shader,
            wgpu::TextureFormat::R8Unorm,
        );

        Self {
            params_buffer,
            kernel_buffer,
            noise_texture,
            noise_view,
            noise_sampler,
            ssao_texture,
            ssao_view,
            ssao_sampler,
            blur_texture,
            blur_view,
            pipeline,
            blur_pipeline,
            bind_group_layout,
            blur_bind_group_layout,
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, projection: Mat4, screen_size: Vec2) {
        let params = SsaoParams::new(projection, screen_size, 0.5, 0.025, 1.0);
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let ssao_width = width.max(1);
        let ssao_height = height.max(1);

        let create_r8_texture = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: ssao_width,
                    height: ssao_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };

        self.ssao_texture = create_r8_texture("SSAO Texture");
        self.ssao_view = self
            .ssao_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.blur_texture = create_r8_texture("SSAO Blur Texture");
        self.blur_view = self
            .blur_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
    ) -> &wgpu::TextureView {
        // SSAO generation pass.
        let depth_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSAO Depth Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSAO Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.kernel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&depth_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.noise_sampler),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SSAO Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ssao_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // Simple 4-tap box blur to reduce noise.
        let blur_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSAO Blur Bind Group"),
            layout: &self.blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.ssao_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.ssao_sampler),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SSAO Blur Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.blur_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, &blur_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        &self.blur_view
    }
}

fn generate_kernel_and_noise() -> (
    [[f32; 4]; SSAO_KERNEL_SIZE],
    [[f32; 4]; SSAO_NOISE_SIZE * SSAO_NOISE_SIZE],
) {
    let mut kernel = [[0.0; 4]; SSAO_KERNEL_SIZE];
    for (i, entry) in kernel.iter_mut().enumerate() {
        let scale = i as f32 / SSAO_KERNEL_SIZE as f32;
        let r = lerp(0.1, 1.0, scale * scale);
        let theta = hash_f(i as u32, 0) * std::f32::consts::TAU;
        let phi = hash_f(i as u32, 1) * std::f32::consts::FRAC_PI_2;
        let x = r * phi.sin() * theta.cos();
        let y = r * phi.sin() * theta.sin();
        let z = r * phi.cos();
        *entry = [x, y, z, 0.0];
    }

    let mut noise = [[0.0; 4]; SSAO_NOISE_SIZE * SSAO_NOISE_SIZE];
    for (i, entry) in noise.iter_mut().enumerate() {
        let angle = hash_f(i as u32, 2) * std::f32::consts::TAU;
        *entry = [angle.cos(), angle.sin(), 0.0, 0.0];
    }

    (kernel, noise)
}

fn queue_write_noise(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    noise: &[[f32; 4]; SSAO_NOISE_SIZE * SSAO_NOISE_SIZE],
) {
    let data: Vec<u8> = noise
        .iter()
        .flat_map(|v| v.iter().copied())
        .flat_map(f32::to_ne_bytes)
        .collect();

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SSAO_NOISE_SIZE as u32 * std::mem::size_of::<[f32; 4]>() as u32),
            rows_per_image: Some(SSAO_NOISE_SIZE as u32),
        },
        wgpu::Extent3d {
            width: SSAO_NOISE_SIZE as u32,
            height: SSAO_NOISE_SIZE as u32,
            depth_or_array_layers: 1,
        },
    );
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// Simple deterministic hash for stable test results without a rand dependency.
fn hash_f(seed: u32, offset: u32) -> f32 {
    let mut s = seed.wrapping_add(offset).wrapping_mul(2654435761);
    s ^= s >> 17;
    s = s.wrapping_mul(1597334677);
    s ^= s >> 14;
    (s as f32) / (u32::MAX as f32)
}

#[cfg(test)]
mod tests {
    use super::generate_kernel_and_noise;

    #[test]
    fn ssao_kernel_uses_the_tbn_normal_axis() {
        let (kernel, noise) = generate_kernel_and_noise();
        assert!(kernel.iter().all(|sample| sample[2] >= 0.0));
        assert!(kernel
            .iter()
            .any(|sample| sample[2] > sample[0].abs() && sample[2] > sample[1].abs()));
        assert!(noise.iter().all(|sample| sample[2] == 0.0));
    }
}
