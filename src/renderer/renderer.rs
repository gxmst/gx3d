use crate::asset::{Handle, Material, Mesh, Texture};
use crate::core::{GxError, GxResult};
use glam::Mat4;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

use super::bind_groups::{BindGroupLayouts, GlobalUniforms, MaterialUniforms};
use super::bloom::BloomPass;
use super::camera::Camera;
use super::object_uniforms::ObjectUniformState;
use crate::asset::Vertex;

/// Directional shadow map resolution. `assets/shaders/pbr.wgsl` derives its
/// PCF texel size from the same value via shader-source injection, and the
/// shadow texel snapping in `game::systems::render` reads this constant.
pub const SHADOW_MAP_SIZE: u32 = 2048;

/// Optional per-material texture views passed to material bind group creation.
/// A `None` slot binds the matching default texture and clears the shader's
/// `has_*_map` flag.
#[derive(Default, Clone, Copy)]
pub struct MaterialTextureViews<'a> {
    pub albedo: Option<&'a wgpu::TextureView>,
    pub normal: Option<&'a wgpu::TextureView>,
    pub emissive: Option<&'a wgpu::TextureView>,
    pub metallic_roughness: Option<&'a wgpu::TextureView>,
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
    /// Alpha-blended variant of the PBR pipeline for water/glass: depth test
    /// on, depth write off, culling off. Drawn after all opaque geometry.
    pub transparent_pipeline: wgpu::RenderPipeline,
    pub shader: wgpu::ShaderModule,
    pub default_texture: wgpu::Texture,
    pub default_texture_view: wgpu::TextureView,
    pub default_black_texture: wgpu::Texture,
    pub default_black_texture_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
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
    pub bloom: BloomPass,
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

        // Bloom bright-pass + blur chain, reading from the HDR target.
        let bloom = BloomPass::new(
            &device,
            surface_config.width,
            surface_config.height,
            &hdr_color_view,
        );

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

        // Default white / black textures for missing material map slots.
        let default_texture = create_solid_texture(
            &device,
            &queue,
            "Default Texture",
            wgpu::TextureFormat::Rgba8UnormSrgb,
            4,
            [255, 255, 255, 255],
        );
        let default_texture_view =
            default_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let default_black_texture = create_solid_texture(
            &device,
            &queue,
            "Default Black Texture",
            wgpu::TextureFormat::Rgba8UnormSrgb,
            4,
            [0, 0, 0, 0],
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

        let placeholder_brdf = create_solid_texture(
            &device,
            &queue,
            "Placeholder BRDF",
            wgpu::TextureFormat::Rgba8Unorm,
            1,
            [128, 128, 0, 0],
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
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
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

        let global_bind_group = create_global_bind_group(
            &device,
            &bind_group_layouts.global,
            &global_buffer,
            &default_irradiance_view,
            &default_prefilter_view,
            &default_brdf_view,
            &ibl_sampler,
            &brdf_sampler,
            &shadow_view,
            &shadow_sampler,
        );

        // Shared object uniform buffer using dynamic offsets.
        let object_uniforms =
            RefCell::new(ObjectUniformState::new(&device, &bind_group_layouts.object));

        // SSAO pass (must exist before the post-processor, which samples it).
        let ssao_pass = super::ssao::SsaoPass::new(
            &device,
            &queue,
            surface_config.width,
            surface_config.height,
            &depth_view,
        );

        // Post-processor (ACES tone mapping), reading HDR + bloom + AO.
        let post_processor = super::post_process::PostProcessor::new(
            &device,
            surface_config.format,
            &hdr_color_view,
            &bloom.view_a,
            &ssao_pass.blur_view,
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

        let make_pipeline =
            |label: &str, blend: wgpu::BlendState, depth_write: bool, cull: Option<wgpu::Face>| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
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
                            blend: Some(blend),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: cull,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: Some(depth_write),
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
                })
            };
        let pipeline = make_pipeline(
            "PBR Pipeline",
            wgpu::BlendState::REPLACE,
            true,
            Some(wgpu::Face::Back),
        );
        let transparent_pipeline = make_pipeline(
            "PBR Transparent Pipeline",
            wgpu::BlendState::ALPHA_BLENDING,
            false,
            None,
        );

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
            transparent_pipeline,
            shader,
            default_texture,
            default_texture_view,
            default_black_texture,
            default_black_texture_view,
            sampler,
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
            bloom,
            post_processor,
            ssao_pass,
            overlay,
            object_uniforms,
            material_bind_groups: RefCell::new(HashMap::new()),
        })
    }

    /// Upload an RGBA8 texture with a full CPU-generated mip chain.
    ///
    /// `srgb` must be true for color data (albedo, emissive) and false for
    /// data maps (normals, metallic-roughness): sampling a normal map through
    /// an sRGB view would decode the encoded vectors and skew all lighting.
    pub fn upload_texture(
        &self,
        texture: &Texture,
        srgb: bool,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let format = if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let mip_level_count = mip_level_count(texture.width, texture.height);
        let wgpu_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&texture.name),
            size: wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let mut level_data = texture.data.clone();
        let (mut level_width, mut level_height) = (texture.width, texture.height);
        for mip in 0..mip_level_count {
            if mip > 0 {
                level_data = downsample_rgba8(&level_data, level_width, level_height);
                level_width = (level_width / 2).max(1);
                level_height = (level_height / 2).max(1);
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &wgpu_texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &level_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * level_width),
                    rows_per_image: Some(level_height),
                },
                wgpu::Extent3d {
                    width: level_width,
                    height: level_height,
                    depth_or_array_layers: 1,
                },
            );
        }

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

            // Recreate the bloom chain (its targets are half resolution) and
            // the full-resolution SSAO targets against the new depth buffer.
            self.bloom
                .resize(&self.device, width, height, &self.hdr_color_view);
            self.ssao_pass
                .resize(&self.device, width, height, &self.depth_view);
            // The post-processor samples HDR/bloom/AO; rebind the new targets.
            self.post_processor.rebuild_bind_group(
                &self.device,
                &self.hdr_color_view,
                &self.bloom.view_a,
                &self.ssao_pass.blur_view,
            );
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
                // Like `Outdated`, a lost surface must be reconfigured or every
                // following frame fails and the window stays black permanently.
                log::error!("Surface lost; reconfiguring");
                self.surface.configure(&self.device, &self.surface_config);
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
        self.object_uniforms.borrow_mut().allocate(model)
    }

    /// Upload the whole frame's object uniforms in a single `write_buffer`.
    /// Call once after every draw's `allocate_object_uniform`, before submit.
    pub fn flush_object_uniforms(&self) {
        self.object_uniforms.borrow().flush(&self.queue);
    }

    fn build_material_bind_group(
        &self,
        material: &Material,
        views: MaterialTextureViews<'_>,
    ) -> wgpu::BindGroup {
        let material_uniforms = MaterialUniforms::new(
            material.albedo_factor,
            material.metallic,
            material.roughness,
            views.albedo.is_some(),
            views.normal.is_some(),
            material.emissive_factor,
            views.emissive.is_some(),
            views.metallic_roughness.is_some(),
            material.water,
        );
        let material_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Material Buffer"),
                contents: bytemuck::cast_slice(&[material_uniforms]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let albedo_view = views.albedo.unwrap_or(&self.default_texture_view);
        let normal_view = views.normal.unwrap_or(&self.default_texture_view);
        let emissive_view = views.emissive.unwrap_or(&self.default_black_texture_view);
        let metallic_roughness_view = views
            .metallic_roughness
            .unwrap_or(&self.default_texture_view);

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
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(metallic_roughness_view),
                },
            ],
        })
    }

    pub fn get_or_create_material_bind_group(
        &self,
        handle: Handle<Material>,
        material: &Material,
        views: MaterialTextureViews<'_>,
    ) -> wgpu::BindGroup {
        let mut cache = self.material_bind_groups.borrow_mut();
        if let Some(bind_group) = cache.get(&handle.id) {
            return bind_group.clone();
        }

        let bind_group = self.build_material_bind_group(material, views);
        cache.insert(handle.id, bind_group.clone());
        bind_group
    }

    pub fn set_ibl(&mut self, ibl: super::ibl::IblSet) {
        self.irradiance_view = ibl.irradiance_view;
        self.prefilter_view = ibl.prefilter_view;
        self.brdf_lut_view = ibl.brdf_lut_view;

        self.global_bind_group = create_global_bind_group(
            &self.device,
            &self.bind_group_layouts.global,
            &self.global_buffer,
            &self.irradiance_view,
            &self.prefilter_view,
            &self.brdf_lut_view,
            &self.ibl_sampler,
            &self.brdf_sampler,
            &self.shadow_view,
            &self.shadow_sampler,
        );
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
                    load: wgpu::LoadOp::Load,
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
}

/// Create a `size`×`size` single-color texture (used for material map
/// fallbacks and the placeholder BRDF LUT).
fn create_solid_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    format: wgpu::TextureFormat,
    size: u32,
    rgba: [u8; 4],
) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let pixels: Vec<u8> = rgba
        .iter()
        .copied()
        .cycle()
        .take((size * size * 4) as usize)
        .collect();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * size),
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
    texture
}

fn mip_level_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

/// Box-filter one RGBA8 mip level down to the next. Simple and CPU-side, but
/// only runs once per texture at load time.
fn downsample_rgba8(src: &[u8], src_width: u32, src_height: u32) -> Vec<u8> {
    let dst_width = (src_width / 2).max(1);
    let dst_height = (src_height / 2).max(1);
    let mut dst = vec![0u8; (dst_width * dst_height * 4) as usize];
    for y in 0..dst_height {
        for x in 0..dst_width {
            // Clamp source coordinates so odd dimensions stay in bounds.
            let sx0 = (x * 2).min(src_width - 1);
            let sx1 = (x * 2 + 1).min(src_width - 1);
            let sy0 = (y * 2).min(src_height - 1);
            let sy1 = (y * 2 + 1).min(src_height - 1);
            for channel in 0..4 {
                let sum = src[((sy0 * src_width + sx0) * 4 + channel) as usize] as u32
                    + src[((sy0 * src_width + sx1) * 4 + channel) as usize] as u32
                    + src[((sy1 * src_width + sx0) * 4 + channel) as usize] as u32
                    + src[((sy1 * src_width + sx1) * 4 + channel) as usize] as u32;
                dst[((y * dst_width + x) * 4 + channel) as usize] = (sum / 4) as u8;
            }
        }
    }
    dst
}

/// The global bind group is identical between startup (placeholder IBL) and
/// `set_ibl` (generated IBL); build it in one place.
#[allow(clippy::too_many_arguments)]
fn create_global_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    global_buffer: &wgpu::Buffer,
    irradiance_view: &wgpu::TextureView,
    prefilter_view: &wgpu::TextureView,
    brdf_lut_view: &wgpu::TextureView,
    ibl_sampler: &wgpu::Sampler,
    brdf_sampler: &wgpu::Sampler,
    shadow_view: &wgpu::TextureView,
    shadow_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Global Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: global_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(irradiance_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(prefilter_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(brdf_lut_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(ibl_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(brdf_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(shadow_sampler),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::SHADOW_MAP_SIZE;
    use crate::renderer::bind_groups::MAX_LIGHTS;

    /// pbr.wgsl declares the same constants; a silent mismatch would skew PCF
    /// filtering or drop lights, so keep the shader source in sync.
    #[test]
    fn pbr_shader_constants_match_the_rust_side() {
        let source = include_str!("../../assets/shaders/pbr.wgsl");
        assert!(
            source.contains(&format!("const MAX_LIGHTS: u32 = {MAX_LIGHTS}u;")),
            "pbr.wgsl MAX_LIGHTS diverged from bind_groups::MAX_LIGHTS"
        );
        assert!(
            source.contains(&format!(
                "const SHADOW_MAP_SIZE: f32 = {SHADOW_MAP_SIZE}.0;"
            )),
            "pbr.wgsl SHADOW_MAP_SIZE diverged from renderer::SHADOW_MAP_SIZE"
        );
        assert!(
            source.contains("array<Light, MAX_LIGHTS>"),
            "pbr.wgsl light array must be sized by MAX_LIGHTS"
        );
    }
}
