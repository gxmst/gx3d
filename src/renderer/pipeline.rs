use crate::asset::Handle;
use crate::asset::Material;

pub struct RenderCommand {
    pub model_matrix: glam::Mat4,
    pub mesh_handle: Handle<crate::asset::Mesh>,
    pub material_handle: Handle<Material>,
}
