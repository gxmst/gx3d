use glam::Vec3;
use std::f32::consts::PI;

pub struct IblSet {
    pub irradiance: wgpu::Texture,
    pub irradiance_view: wgpu::TextureView,
    pub prefilter: wgpu::Texture,
    pub prefilter_view: wgpu::TextureView,
    pub brdf_lut: wgpu::Texture,
    pub brdf_lut_view: wgpu::TextureView,
}

const ENV_SIZE: u32 = 64;
const IRR_SIZE: u32 = 16;
const PREFILTER_SIZE: u32 = 64;
const BRDF_SIZE: u32 = 128;

/// Convert a cubemap face index and normalized UV to a world direction.
/// Face order follows the WebGPU cubemap convention: +X, -X, +Y, -Y, +Z, -Z.
fn cube_dir(face: u32, u: f32, v: f32) -> Vec3 {
    let uu = u * 2.0 - 1.0;
    let vv = v * 2.0 - 1.0;
    match face {
        0 => Vec3::new(1.0, -vv, -uu).normalize(),
        1 => Vec3::new(-1.0, -vv, uu).normalize(),
        2 => Vec3::new(uu, 1.0, vv).normalize(),
        3 => Vec3::new(uu, -1.0, -vv).normalize(),
        4 => Vec3::new(uu, -vv, 1.0).normalize(),
        5 => Vec3::new(-uu, -vv, -1.0).normalize(),
        _ => Vec3::Y,
    }
}

fn sky_color(dir: Vec3) -> Vec3 {
    let sun_dir = Vec3::new(0.3, 0.8, -0.5).normalize();
    let horizon = Vec3::new(1.0, 0.55, 0.25);
    let zenith = Vec3::new(0.18, 0.34, 0.62);
    let ground = Vec3::new(0.02, 0.02, 0.03);

    let t = dir.y.clamp(-1.0, 1.0);
    let base = if t > 0.0 {
        horizon.lerp(zenith, t)
    } else {
        ground.lerp(horizon, t + 1.0)
    };

    let sun_dot = dir.dot(sun_dir).max(0.0);
    let sun = Vec3::new(1.0, 0.9, 0.7) * sun_dot.powf(128.0) * 4.0;
    base + sun
}

fn pack_color(color: Vec3) -> [u8; 4] {
    let c = color.max(Vec3::ZERO).min(Vec3::splat(1.0));
    [
        (c.x * 255.0) as u8,
        (c.y * 255.0) as u8,
        (c.z * 255.0) as u8,
        255,
    ]
}

/// Sample a direction from an in-memory RGBA8 cubemap.
fn sample_cube(data: &[Vec3], size: u32, dir: Vec3) -> Vec3 {
    let ax = dir.x.abs();
    let ay = dir.y.abs();
    let az = dir.z.abs();

    let (face, uu, vv) = if ax >= ay && ax >= az {
        if dir.x > 0.0 {
            (0, -dir.z / ax, -dir.y / ax)
        } else {
            (1, dir.z / ax, -dir.y / ax)
        }
    } else if ay >= ax && ay >= az {
        if dir.y > 0.0 {
            (2, dir.x / ay, dir.z / ay)
        } else {
            (3, dir.x / ay, -dir.z / ay)
        }
    } else if dir.z > 0.0 {
        (4, dir.x / az, -dir.y / az)
    } else {
        (5, -dir.x / az, -dir.y / az)
    };

    let u = ((uu + 1.0) * 0.5).clamp(0.0, 1.0);
    let v = ((vv + 1.0) * 0.5).clamp(0.0, 1.0);

    let fx = u * (size as f32 - 1.0);
    let fy = v * (size as f32 - 1.0);
    let x0 = fx.floor() as u32;
    let y0 = fy.floor() as u32;
    let x1 = (x0 + 1).min(size - 1);
    let y1 = (y0 + 1).min(size - 1);
    let sx = fx - x0 as f32;
    let sy = fy - y0 as f32;

    let face_offset = face * (size * size) as usize;
    let idx = |x: u32, y: u32| face_offset + (y * size + x) as usize;

    let c00 = data[idx(x0, y0)];
    let c10 = data[idx(x1, y0)];
    let c01 = data[idx(x0, y1)];
    let c11 = data[idx(x1, y1)];

    let c0 = c00.lerp(c10, sx);
    let c1 = c01.lerp(c11, sx);
    c0.lerp(c1, sy)
}

fn hemisphere_directions(count: u32) -> impl Iterator<Item = Vec3> {
    let phi_step = PI * (3.0 - 5.0_f32.sqrt());
    let z_step = 2.0 / count as f32;
    (0..count).map(move |i| {
        let z = 1.0 - (i as f32 + 0.5) * z_step;
        let r = (1.0 - z * z).sqrt();
        let phi = i as f32 * phi_step;
        Vec3::new(r * phi.cos(), r * phi.sin(), z)
    })
}

fn build_env_cubemap() -> Vec<Vec3> {
    let mut data = Vec::with_capacity((6 * ENV_SIZE * ENV_SIZE) as usize);
    for face in 0..6 {
        for y in 0..ENV_SIZE {
            for x in 0..ENV_SIZE {
                let u = (x as f32 + 0.5) / ENV_SIZE as f32;
                let v = (y as f32 + 0.5) / ENV_SIZE as f32;
                data.push(sky_color(cube_dir(face, u, v)));
            }
        }
    }
    data
}

fn build_irradiance(env: &[Vec3]) -> Vec<Vec3> {
    let mut data = Vec::with_capacity((6 * IRR_SIZE * IRR_SIZE) as usize);
    let dirs: Vec<Vec3> = hemisphere_directions(256).collect();
    for face in 0..6 {
        for y in 0..IRR_SIZE {
            for x in 0..IRR_SIZE {
                let u = (x as f32 + 0.5) / IRR_SIZE as f32;
                let v = (y as f32 + 0.5) / IRR_SIZE as f32;
                let n = cube_dir(face, u, v);

                let (tangent, bitangent) = if n.dot(Vec3::Y).abs() > 0.999 {
                    let t = n.cross(Vec3::X).normalize();
                    (t, n.cross(t))
                } else {
                    let t = n.cross(Vec3::Y).normalize();
                    (t, n.cross(t))
                };

                let mut acc = Vec3::ZERO;
                let mut weight: f32 = 0.0;
                for s in &dirs {
                    let world = tangent * s.x + bitangent * s.y + n * s.z;
                    let w = world.dot(n).max(0.0);
                    acc += sample_cube(env, ENV_SIZE, world) * w;
                    weight += w;
                }
                data.push(if weight > 0.0 {
                    acc / weight
                } else {
                    Vec3::ZERO
                });
            }
        }
    }
    data
}

fn build_prefilter(env: &[Vec3], size: u32, roughness: f32) -> Vec<Vec3> {
    let mut data = Vec::with_capacity((6 * size * size) as usize);
    let sample_count = (16.0 + roughness * 80.0) as u32;
    let dirs: Vec<Vec3> = hemisphere_directions(sample_count.max(4)).collect();
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32;
                let v = (y as f32 + 0.5) / size as f32;
                let n = cube_dir(face, u, v);
                let r = n;

                let (tangent, bitangent) = if n.dot(Vec3::Y).abs() > 0.999 {
                    let t = n.cross(Vec3::X).normalize();
                    (t, n.cross(t))
                } else {
                    let t = n.cross(Vec3::Y).normalize();
                    (t, n.cross(t))
                };

                let cone = roughness.clamp(0.0, 1.0) * 0.5 * PI;
                let mut acc = Vec3::ZERO;
                let mut weight: f32 = 0.0;
                for s in &dirs {
                    // Perturb the reflection vector within a cone based on roughness.
                    let angle = cone * s.z.acos();
                    let local = Vec3::new(
                        angle.sin() * s.x.atan2(s.y),
                        angle.sin() * s.y.atan2(s.x),
                        angle.cos(),
                    )
                    .normalize();
                    let sample_dir = tangent * local.x + bitangent * local.y + r * local.z;
                    acc += sample_cube(env, ENV_SIZE, sample_dir);
                    weight += 1.0;
                }
                data.push(acc / weight.max(1.0));
            }
        }
    }
    data
}

fn brdf_fresnel(h_dot_v: f32) -> f32 {
    (1.0 - h_dot_v).powf(5.0)
}

fn brdf_geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let k = (roughness * roughness) * 0.5;
    let g_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let g_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
    g_v * g_l
}

fn build_brdf_lut() -> Vec<[u8; 4]> {
    let mut data = Vec::with_capacity((BRDF_SIZE * BRDF_SIZE) as usize);
    let samples = 128;
    for y in 0..BRDF_SIZE {
        let roughness = ((y as f32 + 0.5) / BRDF_SIZE as f32).clamp(0.0, 1.0);
        for x in 0..BRDF_SIZE {
            let n_dot_v = ((x as f32 + 0.5) / BRDF_SIZE as f32).clamp(0.0, 1.0);
            let v = Vec3::new((1.0 - n_dot_v * n_dot_v).sqrt(), 0.0, n_dot_v);

            let mut scale = 0.0;
            let mut bias = 0.0;
            let mut weight = 0.0;

            for i in 0..samples {
                let phi = i as f32 * 2.0 * PI / samples as f32;
                let cos_theta = (1.0 - i as f32 / samples as f32)
                    .powf(1.0 / (roughness * roughness * 4.0 + 1.0));
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

                let h_local = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);
                let _n = Vec3::Z;
                let l = (2.0 * v.dot(h_local) * h_local - v).normalize();
                let n_dot_l = l.z.max(0.0);
                let n_dot_h = h_local.z.max(0.0);
                let h_dot_v = h_local.dot(v).max(0.0);

                if n_dot_l > 0.0 {
                    let g = brdf_geometry_smith(n_dot_v, n_dot_l, roughness);
                    let g_vis = g * h_dot_v / (n_dot_h * n_dot_v);
                    let fc = brdf_fresnel(h_dot_v);
                    scale += (1.0 - fc) * g_vis;
                    bias += fc * g_vis;
                    weight += 1.0;
                }
            }

            if weight > 0.0 {
                scale /= weight;
                bias /= weight;
            }
            let r = (scale * 255.0) as u8;
            let g = (bias * 255.0) as u8;
            data.push([r, g, 0, 255]);
        }
    }
    data
}

fn write_cube_face(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    face: u32,
    mip_level: u32,
    size: u32,
    data: &[Vec3],
) {
    let face_pixels = (size * size) as usize;
    let face_offset = face as usize * face_pixels;
    let mut bytes = Vec::with_capacity(face_pixels * 4);
    for i in 0..face_pixels {
        bytes.extend_from_slice(&pack_color(data[face_offset + i]));
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: face,
            },
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
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
}

fn write_cube_all_faces(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    mip_level: u32,
    size: u32,
    data: &[Vec3],
) {
    for face in 0..6 {
        write_cube_face(queue, texture, face, mip_level, size, data);
    }
}

/// Generate a procedural sky IBL set. This is a stand-in for HDRI-based IBL.
pub fn generate_procedural(device: &wgpu::Device, queue: &wgpu::Queue) -> IblSet {
    let env = build_env_cubemap();

    let irradiance = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Irradiance Map"),
        size: wgpu::Extent3d {
            width: IRR_SIZE,
            height: IRR_SIZE,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let irradiance_data = build_irradiance(&env);
    write_cube_all_faces(queue, &irradiance, 0, IRR_SIZE, &irradiance_data);

    let prefilter = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Prefilter Map"),
        size: wgpu::Extent3d {
            width: PREFILTER_SIZE,
            height: PREFILTER_SIZE,
            depth_or_array_layers: 6,
        },
        mip_level_count: 5,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for mip in 0..5 {
        let size = (PREFILTER_SIZE >> mip).max(1);
        let roughness = mip as f32 / 4.0;
        let data = build_prefilter(&env, size, roughness);
        write_cube_all_faces(queue, &prefilter, mip, size, &data);
    }

    let brdf_lut = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("BRDF LUT"),
        size: wgpu::Extent3d {
            width: BRDF_SIZE,
            height: BRDF_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let brdf_data = build_brdf_lut();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &brdf_lut,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(&brdf_data),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * BRDF_SIZE),
            rows_per_image: Some(BRDF_SIZE),
        },
        wgpu::Extent3d {
            width: BRDF_SIZE,
            height: BRDF_SIZE,
            depth_or_array_layers: 1,
        },
    );

    let irradiance_view = irradiance.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Irradiance View"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let prefilter_view = prefilter.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Prefilter View"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let brdf_lut_view = brdf_lut.create_view(&wgpu::TextureViewDescriptor::default());

    IblSet {
        irradiance,
        irradiance_view,
        prefilter,
        prefilter_view,
        brdf_lut,
        brdf_lut_view,
    }
}
