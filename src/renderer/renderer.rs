use crate::asset::{Handle, Material, Mesh, Texture};
use crate::core::{GxError, GxResult};
use glam::Mat4;
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

use super::bind_groups::{BindGroupLayouts, GlobalUniforms, MaterialUniforms, ObjectUniforms};
use super::camera::Camera;
use crate::asset::Vertex;

const INITIAL_OBJECT_CAPACITY: usize = 1024;

pub(crate) struct ObjectUniformState {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    capacity: usize,
    stride: usize,
    next_index: usize,
}

impl ObjectUniformState {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let raw_size = mem::size_of::<ObjectUniforms>();
        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;
        let stride = raw_size.div_ceil(alignment) * alignment;
        let capacity = INITIAL_OBJECT_CAPACITY;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Object Uniform Buffer"),
            size: (capacity * stride) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Object Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: Some(core::num::NonZeroU64::new(raw_size as u64).unwrap()),
                }),
            }],
        });
        Self {
            buffer,
            bind_group,
            capacity,
            stride,
            next_index: 0,
        }
    }

    fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        count: usize,
    ) {
        if count <= self.capacity {
            return;
        }
        let new_capacity = count.next_power_of_two().max(INITIAL_OBJECT_CAPACITY);
        let raw_size = mem::size_of::<ObjectUniforms>();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Object Uniform Buffer"),
            size: (new_capacity * self.stride) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Object Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: Some(core::num::NonZeroU64::new(raw_size as u64).unwrap()),
                }),
            }],
        });
        self.buffer = buffer;
        self.bind_group = bind_group;
        self.capacity = new_capacity;
    }

    fn reset(&mut self) {
        self.next_index = 0;
    }

    fn allocate(&mut self, queue: &wgpu::Queue, model: Mat4) -> u32 {
        let index = self.next_index;
        self.next_index += 1;
        let offset = index * self.stride;
        let uniforms = ObjectUniforms::new(model);
        queue.write_buffer(
            &self.buffer,
            offset as u64,
            bytemuck::cast_slice(&[uniforms]),
        );
        offset as u32
    }
}

pub struct Renderer {
    pub window: Arc<Window>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub bind_group_layouts: BindGroupLayouts,
    pub global_bind_group: wgpu::BindGroup,
    pub global_buffer: wgpu::Buffer,
    pub pipeline: wgpu::RenderPipeline,
    pub shader: wgpu::ShaderModule,
    pub default_texture: wgpu::Texture,
    pub default_texture_view: wgpu::TextureView,
    pub default_black_texture: wgpu::Texture,
    pub default_black_texture_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub texture_cache: HashMap<u64, (wgpu::Texture, wgpu::TextureView)>,
    pub ibl_sampler: wgpu::Sampler,
    pub brdf_sampler: wgpu::Sampler,
    pub irradiance_view: wgpu::TextureView,
    pub prefilter_view: wgpu::TextureView,
    pub brdf_lut_view: wgpu::TextureView,
    pub shadow_map: wgpu::Texture,
    pub shadow_view: wgpu::TextureView,
    pub shadow_sampler: wgpu::Sampler,
    pub shadow_pipeline: wgpu::RenderPipeline,
    pub shadow_global_layout: wgpu::BindGroupLayout,
    pub shadow_buffer: wgpu::Buffer,
    pub shadow_bind_group: wgpu::BindGroup,
    pub hdr_color_texture: wgpu::Texture,
    pub hdr_color_view: wgpu::TextureView,
    pub bloom_texture_a: wgpu::Texture,
    pub bloom_view_a: wgpu::TextureView,
    pub bloom_texture_b: wgpu::Texture,
    pub bloom_view_b: wgpu::TextureView,
    pub bloom_sampler: wgpu::Sampler,
    pub bloom_threshold_pipeline: wgpu::RenderPipeline,
    pub bloom_threshold_layout: wgpu::BindGroupLayout,
    pub bloom_blur_pipeline: wgpu::RenderPipeline,
    pub bloom_blur_layout: wgpu::BindGroupLayout,
    pub bloom_blur_buffer: wgpu::Buffer,
    pub post_processor: super::post_process::PostProcessor,
    pub ssao_pass: super::ssao::SsaoPass,
    pub overlay: RefCell<super::overlay::OverlayRenderer>,
    pub(crate) object_uniforms: RefCell<ObjectUniformState>,
    pub(crate) material_bind_groups: RefCell<HashMap<u64, wgpu::BindGroup>>,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> GxResult<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| GxError::Surface(format!("{:?}", e)))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| GxError::RequestAdapter)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| GxError::RequestDevice(format!("{}", e)))?;

        let size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| GxError::Render("Failed to configure surface".to_string()))?;

        surface.configure(&device, &surface_config);

        // Create depth texture
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // HDR color target for scene rendering before post-processing.
        let hdr_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Color Texture"),
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hdr_color_view = hdr_color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Bloom targets at half resolution.
        let bloom_width = (surface_config.width / 2).max(1);
        let bloom_height = (surface_config.height / 2).max(1);
        let create_bloom_texture = |label| {
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let bloom_texture_a = create_bloom_texture("Bloom A");
        let bloom_view_a = bloom_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_texture_b = create_bloom_texture("Bloom B");
        let bloom_view_b = bloom_texture_b.create_view(&wgpu::TextureViewDescriptor::default());

        let bloom_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bloom_threshold_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bloom_threshold_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Bloom Threshold Pipeline Layout"),
                bind_group_layouts: &[Some(&bloom_threshold_layout)],
                immediate_size: 0,
            });

        let bloom_threshold_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Threshold Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/bloom_threshold.wgsl").into(),
            ),
        });

        let bloom_threshold_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Bloom Threshold Pipeline"),
                layout: Some(&bloom_threshold_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &bloom_threshold_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &bloom_threshold_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
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
            });

        let bloom_blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bloom_blur_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Bloom Blur Pipeline Layout"),
                bind_group_layouts: &[Some(&bloom_blur_layout)],
                immediate_size: 0,
            });

        let bloom_blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/bloom_blur.wgsl").into(),
            ),
        });

        let bloom_blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Bloom Blur Pipeline"),
            layout: Some(&bloom_blur_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bloom_blur_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &bloom_blur_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
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
        });

        let bloom_blur_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bloom Blur Uniforms"),
            contents: bytemuck::cast_slice(&[0.0f32, 0.0f32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layouts
        let bind_group_layouts = BindGroupLayouts::new(&device);

        // Create global uniform buffer
        let global_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Buffer"),
            contents: bytemuck::cast_slice(&[GlobalUniforms::new(
                Mat4::IDENTITY,
                glam::Vec3::ZERO,
                0.0,
            )]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create default white texture
        let default_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Texture"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload white pixels
        let white_pixels = vec![255u8; 4 * 4 * 4];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * 4),
                rows_per_image: Some(4),
            },
            wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
        );

        let default_texture_view =
            default_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create default black texture for missing emission/roughness maps.
        let default_black_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Black Texture"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let black_pixels = vec![0u8; 4 * 4 * 4];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_black_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &black_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * 4),
                rows_per_image: Some(4),
            },
            wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
        );
        let default_black_texture_view =
            default_black_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Placeholder IBL textures (black) until IBL generation is wired up.
        let placeholder_cube = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Placeholder Cube"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let black_pixel = [0u8; 4];
        for face in 0..6 {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &placeholder_cube,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: face,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &black_pixel,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        let default_irradiance_view = placeholder_cube.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Irradiance View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let default_prefilter_view = placeholder_cube.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Prefilter View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let placeholder_brdf = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Placeholder BRDF"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &placeholder_brdf,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[128u8, 128u8, 0u8, 0u8],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let default_brdf_view =
            placeholder_brdf.create_view(&wgpu::TextureViewDescriptor::default());

        // Create samplers
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Default Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        let ibl_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("IBL Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let brdf_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BRDF Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let shadow_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
            size: wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_map.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let shadow_global_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Global Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let shadow_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Buffer"),
            contents: bytemuck::cast_slice(&[super::bind_groups::ShadowUniforms::new(
                Mat4::IDENTITY,
            )]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Bind Group"),
            layout: &shadow_global_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_buffer.as_entire_binding(),
            }],
        });

        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/shadow.wgsl").into(),
            ),
        });

        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&shadow_global_layout),
                    Some(&bind_group_layouts.object),
                ],
                immediate_size: 0,
            });

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Bind Group"),
            layout: &bind_group_layouts.global,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: global_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&default_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&default_prefilter_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&default_brdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&ibl_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&brdf_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });

        // Shared object uniform buffer using dynamic offsets.
        let object_uniforms =
            RefCell::new(ObjectUniformState::new(&device, &bind_group_layouts.object));

        // Post-processor (ACES tone mapping).
        let post_processor =
            super::post_process::PostProcessor::new(&device, surface_config.format);

        // SSAO pass.
        let ssao_pass = super::ssao::SsaoPass::new(
            &device,
            &queue,
            surface_config.width,
            surface_config.height,
        );
        let overlay = RefCell::new(super::overlay::OverlayRenderer::new(
            &device,
            &queue,
            surface_config.width,
            surface_config.height,
            surface_config.format,
        ));

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PBR Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/pbr.wgsl").into()),
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[
                Some(&bind_group_layouts.global),
                Some(&bind_group_layouts.material),
                Some(&bind_group_layouts.object),
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            window,
            device,
            queue,
            surface,
            surface_config,
            depth_texture,
            depth_view,
            bind_group_layouts,
            global_bind_group,
            global_buffer,
            pipeline,
            shader,
            default_texture,
            default_texture_view,
            default_black_texture,
            default_black_texture_view,
            sampler,
            texture_cache: HashMap::new(),
            ibl_sampler,
            brdf_sampler,
            irradiance_view: default_irradiance_view,
            prefilter_view: default_prefilter_view,
            brdf_lut_view: default_brdf_view,
            shadow_map,
            shadow_view,
            shadow_sampler,
            shadow_pipeline,
            shadow_global_layout,
            shadow_buffer,
            shadow_bind_group,
            hdr_color_texture,
            hdr_color_view,
            bloom_texture_a,
            bloom_view_a,
            bloom_texture_b,
            bloom_view_b,
            bloom_sampler,
            bloom_threshold_pipeline,
            bloom_threshold_layout,
            bloom_blur_pipeline,
            bloom_blur_layout,
            bloom_blur_buffer,
            post_processor,
            ssao_pass,
            overlay,
            object_uniforms,
            material_bind_groups: RefCell::new(HashMap::new()),
        })
    }

    pub fn upload_texture(&mut self, texture: &Texture) -> (wgpu::Texture, wgpu::TextureView) {
        let wgpu_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&texture.name),
            size: wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * texture.width),
                rows_per_image: Some(texture.height),
            },
            wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
        );

        let view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());
        (wgpu_texture, view)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);

            // Recreate depth texture
            self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Depth Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.depth_view = self
                .depth_texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            // Recreate HDR color target
            self.hdr_color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("HDR Color Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.hdr_color_view = self
                .hdr_color_texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            // Recreate bloom targets at half resolution.
            let bloom_width = (width / 2).max(1);
            let bloom_height = (height / 2).max(1);
            let create_bloom_texture = |label| {
                self.device.create_texture(&wgpu::TextureDescriptor {
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
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
            };
            self.bloom_texture_a = create_bloom_texture("Bloom A");
            self.bloom_view_a = self
                .bloom_texture_a
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.bloom_texture_b = create_bloom_texture("Bloom B");
            self.bloom_view_b = self
                .bloom_texture_b
                .create_view(&wgpu::TextureViewDescriptor::default());

            // Resize SSAO targets at half resolution.
            self.ssao_pass.resize(&self.device, width, height);
            self.overlay.borrow_mut().resize(width, height, &self.queue);
        }
    }

    pub fn update_global_uniforms(
        &self,
        camera: &Camera,
        time: f32,
        lights: &[super::bind_groups::LightData],
        light_space_matrix: Mat4,
    ) {
        let view_proj = camera.view_projection_matrix();
        let mut uniforms =
            super::bind_groups::GlobalUniforms::new(view_proj, camera.position, time);
        uniforms.set_lights(lights);
        uniforms.set_light_space_matrix(light_space_matrix);
        self.queue
            .write_buffer(&self.global_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    pub fn update_shadow_uniforms(&self, light_view_proj: Mat4) {
        let uniforms = super::bind_groups::ShadowUniforms::new(light_view_proj);
        self.queue
            .write_buffer(&self.shadow_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    pub fn begin_shadow_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    pub fn render_shadow_mesh<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        mesh: &'a Mesh,
        object_dynamic_offset: u32,
    ) {
        if let (Some(vb), Some(ib)) = (&mesh.vertex_buffer, &mesh.index_buffer) {
            let object_bind_group = &self.object_uniforms.borrow().bind_group;
            render_pass.set_pipeline(&self.shadow_pipeline);
            render_pass.set_bind_group(0, &self.shadow_bind_group, &[]);
            render_pass.set_bind_group(1, object_bind_group, &[object_dynamic_offset]);
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    pub fn acquire_surface_texture(&self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => Some(surface_texture),
            wgpu::CurrentSurfaceTexture::Timeout => {
                log::warn!("Surface timeout");
                None
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                log::warn!("Surface occluded");
                None
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                log::warn!("Surface outdated");
                self.surface.configure(&self.device, &self.surface_config);
                None
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                log::error!("Surface lost");
                None
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("Surface validation error");
                None
            }
        }
    }

    pub fn render_mesh<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        mesh: &'a Mesh,
        material_bind_group: &'a wgpu::BindGroup,
        object_dynamic_offset: u32,
    ) {
        if let (Some(vb), Some(ib)) = (&mesh.vertex_buffer, &mesh.index_buffer) {
            let object_bind_group = &self.object_uniforms.borrow().bind_group;
            render_pass.set_bind_group(0, &self.global_bind_group, &[]);
            render_pass.set_bind_group(1, material_bind_group, &[]);
            render_pass.set_bind_group(2, object_bind_group, &[object_dynamic_offset]);
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    pub fn begin_object_uniform_frame(&self) {
        self.object_uniforms.borrow_mut().reset();
    }

    pub fn ensure_object_uniform_capacity(&self, count: usize) {
        self.object_uniforms.borrow_mut().ensure_capacity(
            &self.device,
            &self.bind_group_layouts.object,
            count,
        );
    }

    pub fn allocate_object_uniform(&self, model: Mat4) -> u32 {
        self.object_uniforms
            .borrow_mut()
            .allocate(&self.queue, model)
    }

    fn build_material_bind_group(
        &self,
        material: &Material,
        albedo_texture_view: Option<&wgpu::TextureView>,
        normal_texture_view: Option<&wgpu::TextureView>,
        emissive_texture_view: Option<&wgpu::TextureView>,
    ) -> wgpu::BindGroup {
        let material_uniforms = MaterialUniforms::new(
            material.albedo_factor,
            material.metallic,
            material.roughness,
            albedo_texture_view.is_some(),
            normal_texture_view.is_some(),
            material.emissive_factor,
            emissive_texture_view.is_some(),
        );
        let material_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Material Buffer"),
                contents: bytemuck::cast_slice(&[material_uniforms]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let albedo_view = albedo_texture_view.unwrap_or(&self.default_texture_view);
        let normal_view = normal_texture_view.unwrap_or(&self.default_texture_view);
        let emissive_view = emissive_texture_view.unwrap_or(&self.default_black_texture_view);

        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &self.bind_group_layouts.material,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: material_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(emissive_view),
                },
            ],
        })
    }

    pub fn get_or_create_material_bind_group(
        &self,
        handle: Handle<Material>,
        material: &Material,
        albedo_texture_view: Option<&wgpu::TextureView>,
        normal_texture_view: Option<&wgpu::TextureView>,
        emissive_texture_view: Option<&wgpu::TextureView>,
    ) -> wgpu::BindGroup {
        let mut cache = self.material_bind_groups.borrow_mut();
        if let Some(bind_group) = cache.get(&handle.id) {
            return bind_group.clone();
        }

        let bind_group = self.build_material_bind_group(
            material,
            albedo_texture_view,
            normal_texture_view,
            emissive_texture_view,
        );
        cache.insert(handle.id, bind_group.clone());
        bind_group
    }

    /// Backwards-compatible helper for code that still wants a one-off material bind group.
    pub fn create_material_bind_group(
        &self,
        material: &Material,
        albedo_texture_view: Option<&wgpu::TextureView>,
        normal_texture_view: Option<&wgpu::TextureView>,
    ) -> wgpu::BindGroup {
        self.build_material_bind_group(material, albedo_texture_view, normal_texture_view, None)
    }

    pub fn set_ibl(&mut self, ibl: super::ibl::IblSet) {
        self.irradiance_view = ibl.irradiance_view;
        self.prefilter_view = ibl.prefilter_view;
        self.brdf_lut_view = ibl.brdf_lut_view;

        self.global_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Bind Group"),
            layout: &self.bind_group_layouts.global,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.global_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.prefilter_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.brdf_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.ibl_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.brdf_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&self.shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&self.shadow_sampler),
                },
            ],
        });
    }

    pub fn render_bloom(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        hdr_view: &wgpu::TextureView,
    ) -> &wgpu::TextureView {
        // Threshold pass: HDR -> bloom A
        {
            let threshold_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom Threshold Bind Group"),
                layout: &self.bloom_threshold_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(hdr_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.bloom_sampler),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Threshold Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_view_a,
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
            pass.set_pipeline(&self.bloom_threshold_pipeline);
            pass.set_bind_group(0, &threshold_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // Blur passes: A -> B -> A
        self.dispatch_blur(
            encoder,
            &self.bloom_view_a,
            &self.bloom_view_b,
            glam::Vec2::X,
        );
        self.dispatch_blur(
            encoder,
            &self.bloom_view_b,
            &self.bloom_view_a,
            glam::Vec2::Y,
        );

        &self.bloom_view_a
    }

    fn dispatch_blur(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        direction: glam::Vec2,
    ) {
        self.queue.write_buffer(
            &self.bloom_blur_buffer,
            0,
            bytemuck::cast_slice(&[direction.x, direction.y]),
        );
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Blur Bind Group"),
            layout: &self.bloom_blur_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.bloom_blur_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.bloom_sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Bloom Blur Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
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
        pass.set_pipeline(&self.bloom_blur_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn create_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        color_view: &'a wgpu::TextureView,
        depth_view: &'a wgpu::TextureView,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    pub fn end_frame(&self, output: wgpu::SurfaceTexture) {
        output.present();
    }
}
