use wgpu::util::DeviceExt;

use super::pipeline::create_fullscreen_pipeline;

/// Half-resolution bright-pass + separable blur used by the post-process
/// chain. All bind groups are cached and only rebuilt on resize.
pub struct BloomPass {
    pub texture_a: wgpu::Texture,
    pub view_a: wgpu::TextureView,
    pub texture_b: wgpu::Texture,
    pub view_b: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    threshold_pipeline: wgpu::RenderPipeline,
    threshold_layout: wgpu::BindGroupLayout,
    blur_pipeline: wgpu::RenderPipeline,
    blur_layout: wgpu::BindGroupLayout,
    blur_buffer_x: wgpu::Buffer,
    blur_buffer_y: wgpu::Buffer,
    threshold_bind_group: wgpu::BindGroup,
    blur_bind_group_x: wgpu::BindGroup,
    blur_bind_group_y: wgpu::BindGroup,
}

impl BloomPass {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        hdr_view: &wgpu::TextureView,
    ) -> Self {
        let (texture_a, view_a, texture_b, view_b) = create_targets(device, width, height);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let threshold_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Threshold Layout"),
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

        let threshold_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Bloom Threshold Pipeline Layout"),
                bind_group_layouts: &[Some(&threshold_layout)],
                immediate_size: 0,
            });

        let threshold_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Threshold Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/bloom_threshold.wgsl").into(),
            ),
        });

        let threshold_pipeline = create_fullscreen_pipeline(
            device,
            "Bloom Threshold Pipeline",
            &threshold_pipeline_layout,
            &threshold_shader,
            wgpu::TextureFormat::Rgba16Float,
        );

        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Blur Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Blur Pipeline Layout"),
            bind_group_layouts: &[Some(&blur_layout)],
            immediate_size: 0,
        });

        let blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/bloom_blur.wgsl").into(),
            ),
        });

        let blur_pipeline = create_fullscreen_pipeline(
            device,
            "Bloom Blur Pipeline",
            &blur_pipeline_layout,
            &blur_shader,
            wgpu::TextureFormat::Rgba16Float,
        );

        let create_blur_uniform = |label, direction: glam::Vec2| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&[direction.x, direction.y]),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };
        // Queue writes are applied before the encoded passes at submit time,
        // so each separable pass needs its own immutable direction buffer.
        let blur_buffer_x = create_blur_uniform("Bloom Blur X Uniforms", glam::Vec2::X);
        let blur_buffer_y = create_blur_uniform("Bloom Blur Y Uniforms", glam::Vec2::Y);

        let (threshold_bind_group, blur_bind_group_x, blur_bind_group_y) = build_bind_groups(
            device,
            &threshold_layout,
            &blur_layout,
            &sampler,
            hdr_view,
            &view_a,
            &view_b,
            &blur_buffer_x,
            &blur_buffer_y,
        );

        Self {
            texture_a,
            view_a,
            texture_b,
            view_b,
            sampler,
            threshold_pipeline,
            threshold_layout,
            blur_pipeline,
            blur_layout,
            blur_buffer_x,
            blur_buffer_y,
            threshold_bind_group,
            blur_bind_group_x,
            blur_bind_group_y,
        }
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        hdr_view: &wgpu::TextureView,
    ) {
        let (texture_a, view_a, texture_b, view_b) = create_targets(device, width, height);
        self.texture_a = texture_a;
        self.view_a = view_a;
        self.texture_b = texture_b;
        self.view_b = view_b;

        let (threshold_bind_group, blur_bind_group_x, blur_bind_group_y) = build_bind_groups(
            device,
            &self.threshold_layout,
            &self.blur_layout,
            &self.sampler,
            hdr_view,
            &self.view_a,
            &self.view_b,
            &self.blur_buffer_x,
            &self.blur_buffer_y,
        );
        self.threshold_bind_group = threshold_bind_group;
        self.blur_bind_group_x = blur_bind_group_x;
        self.blur_bind_group_y = blur_bind_group_y;
    }

    /// Threshold HDR -> A, blur A -> B (horizontal), blur B -> A (vertical).
    /// Returns the final blurred bloom target.
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder) -> &wgpu::TextureView {
        self.run_fullscreen_pass(
            encoder,
            "Bloom Threshold Pass",
            &self.threshold_pipeline,
            &self.threshold_bind_group,
            &self.view_a,
        );
        self.run_fullscreen_pass(
            encoder,
            "Bloom Blur Pass",
            &self.blur_pipeline,
            &self.blur_bind_group_x,
            &self.view_b,
        );
        self.run_fullscreen_pass(
            encoder,
            "Bloom Blur Pass",
            &self.blur_pipeline,
            &self.blur_bind_group_y,
            &self.view_a,
        );
        &self.view_a
    }

    fn run_fullscreen_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &str,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        target: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Bloom renders at half resolution; the blur widens naturally and the cost
/// stays low on large displays.
fn create_targets(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Texture,
    wgpu::TextureView,
) {
    let bloom_width = (width / 2).max(1);
    let bloom_height = (height / 2).max(1);
    let create = |label| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: bloom_width,
                height: bloom_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    };
    let texture_a = create("Bloom A");
    let view_a = texture_a.create_view(&wgpu::TextureViewDescriptor::default());
    let texture_b = create("Bloom B");
    let view_b = texture_b.create_view(&wgpu::TextureViewDescriptor::default());
    (texture_a, view_a, texture_b, view_b)
}

#[allow(clippy::too_many_arguments)]
fn build_bind_groups(
    device: &wgpu::Device,
    threshold_layout: &wgpu::BindGroupLayout,
    blur_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    hdr_view: &wgpu::TextureView,
    view_a: &wgpu::TextureView,
    view_b: &wgpu::TextureView,
    blur_buffer_x: &wgpu::Buffer,
    blur_buffer_y: &wgpu::Buffer,
) -> (wgpu::BindGroup, wgpu::BindGroup, wgpu::BindGroup) {
    let threshold = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bloom Threshold Bind Group"),
        layout: threshold_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(hdr_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    let blur = |label, buffer: &wgpu::Buffer, input: &wgpu::TextureView| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: blur_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    };
    let blur_x = blur("Bloom Blur X Bind Group", blur_buffer_x, view_a);
    let blur_y = blur("Bloom Blur Y Bind Group", blur_buffer_y, view_b);
    (threshold, blur_x, blur_y)
}
