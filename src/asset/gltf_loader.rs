use super::{AssetManager, Handle, Material, Mesh, Texture, TextureFormat, Vertex};
use crate::core::GxResult;
use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};

pub struct GltfLoader;

pub struct GltfLoadResult {
    pub meshes: Vec<Handle<Mesh>>,
    /// Primitive material matching each entry in `meshes`.
    pub mesh_materials: Vec<Handle<Material>>,
    pub materials: Vec<Handle<Material>>,
    pub textures: Vec<Handle<Texture>>,
}

impl GltfLoader {
    pub fn load(path: &str, asset_manager: &mut AssetManager) -> GxResult<GltfLoadResult> {
        let (document, buffers, images) = gltf::import(path)?;

        let mut mesh_handles = Vec::new();
        let mut material_handles = Vec::new();
        let mut mesh_materials = Vec::new();
        // Keep image indices stable even when an unsupported image is skipped.
        // glTF textures reference image sources, and texture/image indices are
        // not guaranteed to be identical.
        let mut image_handles = Vec::with_capacity(images.len());

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
                    image_handles.push(None);
                    continue;
                }
            };
            let handle = asset_manager.textures.insert(texture);
            image_handles.push(Some(handle));
        }

        // Load materials
        for material in document.materials() {
            let pbr = material.pbr_metallic_roughness();

            let albedo_factor: [f32; 4] = pbr.base_color_factor();
            let metallic = pbr.metallic_factor();
            let roughness = pbr.roughness_factor();

            let albedo_map = pbr.base_color_texture().and_then(|info| {
                let image_index = info.texture().source().index();
                image_handles.get(image_index).copied().flatten()
            });

            let metallic_roughness_map = pbr.metallic_roughness_texture().and_then(|info| {
                let image_index = info.texture().source().index();
                image_handles.get(image_index).copied().flatten()
            });

            let normal_map = material.normal_texture().and_then(|info| {
                let image_index = info.texture().source().index();
                image_handles.get(image_index).copied().flatten()
            });

            let emissive_factor: [f32; 3] = material.emissive_factor();
            let emissive_map = material.emissive_texture().and_then(|info| {
                let image_index = info.texture().source().index();
                image_handles.get(image_index).copied().flatten()
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
                water: false,
            };
            let handle = asset_manager.materials.insert(mat);
            material_handles.push(handle);
        }

        // A primitive without an explicit material always uses the glTF
        // default material, even when other materials exist in the file.
        let default_material = asset_manager.materials.insert(Material::new("Default"));

        // Walk the scene graph instead of iterating raw meshes. This preserves
        // nested node transforms and correctly imports instanced primitives.
        if let Some(scene) = document
            .default_scene()
            .or_else(|| document.scenes().next())
        {
            for node in scene.nodes() {
                Self::load_node(
                    node,
                    Mat4::IDENTITY,
                    &buffers,
                    &material_handles,
                    default_material,
                    asset_manager,
                    &mut mesh_handles,
                    &mut mesh_materials,
                );
            }
        }

        Ok(GltfLoadResult {
            meshes: mesh_handles,
            mesh_materials,
            materials: material_handles,
            textures: image_handles.into_iter().flatten().collect(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn load_node(
        node: gltf::Node<'_>,
        parent_transform: Mat4,
        buffers: &[gltf::buffer::Data],
        material_handles: &[Handle<Material>],
        default_material: Handle<Material>,
        asset_manager: &mut AssetManager,
        mesh_handles: &mut Vec<Handle<Mesh>>,
        mesh_materials: &mut Vec<Handle<Material>>,
    ) {
        let local = Mat4::from_cols_array_2d(&node.transform().matrix());
        let world_transform = parent_transform * local;
        let linear_transform = Mat3::from_mat4(world_transform);
        let determinant = linear_transform.determinant();
        let normal_transform = if determinant.is_finite() && determinant.abs() > 1.0e-8 {
            linear_transform.inverse().transpose()
        } else {
            Mat3::IDENTITY
        };

        if let Some(mesh) = node.mesh() {
            for (primitive_index, primitive) in mesh.primitives().enumerate() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions: Vec<Vec3> = match reader.read_positions() {
                    Some(iter) => iter
                        .map(|p| world_transform.transform_point3(Vec3::from(p)))
                        .collect(),
                    None => continue,
                };

                let normals: Vec<Vec3> = reader
                    .read_normals()
                    .map(|n| {
                        n.map(|value| {
                            normal_transform
                                .mul_vec3(Vec3::from(value))
                                .normalize_or_zero()
                        })
                        .collect()
                    })
                    .unwrap_or_else(|| vec![Vec3::Y; positions.len()]);

                let uvs: Vec<Vec2> = reader
                    .read_tex_coords(0)
                    .map(|uv| uv.into_f32().map(Vec2::from).collect())
                    .unwrap_or_else(|| vec![Vec2::ZERO; positions.len()]);

                // Read tangents if available
                let tangents: Vec<[f32; 4]> = reader
                    .read_tangents()
                    .map(|t| {
                        t.enumerate()
                            .map(|(index, value)| {
                                let normal = normals.get(index).copied().unwrap_or(Vec3::Y);
                                let transformed = linear_transform
                                    .mul_vec3(Vec3::new(value[0], value[1], value[2]))
                                    .normalize_or_zero();
                                let direction = (transformed - normal * transformed.dot(normal))
                                    .normalize_or_zero();
                                let handedness = if determinant < 0.0 {
                                    -value[3]
                                } else {
                                    value[3]
                                };
                                Vec4::new(direction.x, direction.y, direction.z, handedness)
                                    .to_array()
                            })
                            .collect()
                    })
                    .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);

                let mut indices: Vec<u32> = reader
                    .read_indices()
                    .map(|i| i.into_u32().collect())
                    .unwrap_or_else(|| (0..positions.len() as u32).collect());
                if determinant < 0.0 {
                    for triangle in indices.chunks_exact_mut(3) {
                        triangle.swap(1, 2);
                    }
                }

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

                let material_handle = primitive
                    .material()
                    .index()
                    .and_then(|index| material_handles.get(index).copied())
                    .unwrap_or(default_material);

                let mesh_name = format!(
                    "{}:{}",
                    node.name().or_else(|| mesh.name()).unwrap_or("Unnamed"),
                    primitive_index
                );
                let mesh_asset = Mesh::new(&mesh_name, vertices, indices);
                let mesh_handle = asset_manager.meshes.insert(mesh_asset);
                mesh_handles.push(mesh_handle);
                mesh_materials.push(material_handle);
            }
        }

        for child in node.children() {
            Self::load_node(
                child,
                world_transform,
                buffers,
                material_handles,
                default_material,
                asset_manager,
                mesh_handles,
                mesh_materials,
            );
        }
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

#[cfg(test)]
mod tests {
    use super::{AssetManager, GltfLoader};

    #[test]
    fn bundled_glb_has_one_material_mapping_per_imported_primitive() {
        let mut assets = AssetManager::new();
        let result = GltfLoader::load("assets/models/damaged_helmet.glb", &mut assets).unwrap();
        assert!(!result.meshes.is_empty());
        assert_eq!(result.meshes.len(), result.mesh_materials.len());
    }
}
