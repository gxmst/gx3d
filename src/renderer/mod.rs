pub mod bind_groups;
pub mod camera;
pub mod ibl;
pub mod light;
pub mod overlay;
pub mod pipeline;
pub mod post_process;
pub mod renderer;
pub mod ssao;

pub use camera::*;
pub use ibl::*;
pub use light::*;
pub use overlay::*;
pub use post_process::*;
pub use renderer::*;
pub use ssao::*;

use crate::asset::{Handle, Material, Mesh};
use glam::Mat4;

pub struct RenderMesh {
    pub mesh: Handle<Mesh>,
    pub material: Handle<Material>,
}

pub struct RenderData {
    pub view_proj: Mat4,
    pub camera_pos: glam::Vec3,
    pub time: f32,
    pub ambient_color: [f32; 4],
    pub meshes: Vec<(Mat4, Handle<Mesh>, Handle<Material>)>,
    pub lights: Vec<Light>,
}
