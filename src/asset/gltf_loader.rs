use super::{AssetManager, Handle, Material, Mesh, Texture, TextureFormat, Vertex};
use crate::core::GxResult;
use glam::{Vec2, Vec3};

pub struct GltfLoader;

pub struct GltfLoadResult {
    pub meshes: Vec<Handle<Mesh>>,
    pub materials: Vec<Handle<Material>>,
    pub textures: Vec<Handle<Texture>>,
}

impl GltfLoader {
    pub fn load(path: &str, asset_manager: &mut AssetManager) -> GxResult<GltfLoadResult> {
        let (document, buffers, images) = gltf::import(path)?;

        let mut mesh_handles = Vec::new();
        let mut material_handles = Vec::new();
        let mut texture_handles = Vec::new();

        // Load textures
        for (i, image) in images.iter().enumerate() {
            let texture = match image.format {
                gltf::image::Format::R8G8B8 => {
                    let rgba_data = Self::rgb_to_rgba(&image.pixels);
                    Texture::new(
                        &format!("{}_tex_{}", path, i),
                        image.width,
                        image.height,
                        rgba_data,
                        TextureFormat::Rgba8,
                    )
                }
                gltf::image::Format::R8G8B8A8 => Texture::new(
                    &format!("{}_tex_{}", path, i),
                    image.width,
                    image.height,
                    image.pixels.clone(),
                    TextureFormat::Rgba8,
                ),
                _ => {
                    log::warn!("Unsupported texture format: {:?}", image.format);
                    continue;
                }
            };
            let handle = asset_manager.textures.insert(texture);
            texture_handles.push(handle);
        }

        // Load materials
        for material in document.materials() {
            let pbr = material.pbr_metallic_roughness();

            let albedo_factor: [f32; 4] = pbr.base_color_factor();
            let metallic = pbr.metallic_factor();
            let roughness = pbr.roughness_factor();

            let albedo_map = pbr.base_color_texture().and_then(|info| {
                let tex_index = info.texture().index();
                texture_handles.get(tex_index).copied()
            });

            let metallic_roughness_map = pbr.metallic_roughness_texture().and_then(|info| {
                let tex_index = info.texture().index();
                texture_handles.get(tex_index).copied()
            });

            let normal_map = material.normal_texture().and_then(|info| {
                let tex_index = info.texture().index();
                texture_handles.get(tex_index).copied()
            });

            let emissive_factor: [f32; 3] = material.emissive_factor();
            let emissive_map = material.emissive_texture().and_then(|info| {
                let tex_index = info.texture().index();
                texture_handles.get(tex_index).copied()
            });

            let mat = Material {
                name: material.name().unwrap_or("Unnamed").to_string(),
                albedo_factor,
                metallic,
                roughness,
                emissive_factor,
                albedo_map,
                normal_map,
                metallic_roughness_map,
                emissive_map,
            };
            let handle = asset_manager.materials.insert(mat);
            material_handles.push(handle);
        }

        // If no materials, add a default one
        if material_handles.is_empty() {
            let default_mat = Material::new("Default");
            material_handles.push(asset_manager.materials.insert(default_mat));
        }

        // Load meshes
        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions: Vec<Vec3> = match reader.read_positions() {
                    Some(iter) => iter.map(Vec3::from).collect(),
                    None => continue,
                };

                let normals: Vec<Vec3> = reader
                    .read_normals()
                    .map(|n| n.map(Vec3::from).collect())
                    .unwrap_or_else(|| vec![Vec3::Y; positions.len()]);

                let uvs: Vec<Vec2> = reader
                    .read_tex_coords(0)
                    .map(|uv| uv.into_f32().map(Vec2::from).collect())
                    .unwrap_or_else(|| vec![Vec2::ZERO; positions.len()]);

                // Read tangents if available
                let tangents: Vec<[f32; 4]> = reader
                    .read_tangents()
                    .map(|t| t.collect())
                    .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);

                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|i| i.into_u32().collect())
                    .unwrap_or_else(|| (0..positions.len() as u32).collect());

                let vertices: Vec<Vertex> = positions
                    .iter()
                    .zip(normals.iter())
                    .zip(uvs.iter())
                    .zip(tangents.iter())
                    .map(|(((pos, normal), uv), tangent)| {
                        let mut v = Vertex::new(*pos, *normal, *uv);
                        v.tangent = *tangent;
                        v
                    })
                    .collect();

                let material_index = primitive.material().index().unwrap_or(0);
                let _material_handle = material_handles
                    .get(material_index)
                    .copied()
                    .unwrap_or(material_handles[0]);

                let mesh_asset = Mesh::new(mesh.name().unwrap_or("Unnamed"), vertices, indices);
                let mesh_handle = asset_manager.meshes.insert(mesh_asset);
                mesh_handles.push(mesh_handle);
            }
        }

        Ok(GltfLoadResult {
            meshes: mesh_handles,
            materials: material_handles,
            textures: texture_handles,
        })
    }

    fn rgb_to_rgba(pixels: &[u8]) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(pixels.len() * 4 / 3);
        for chunk in pixels.chunks(3) {
            if chunk.len() == 3 {
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
                rgba.push(chunk[2]);
                rgba.push(255);
            }
        }
        rgba
    }
}
