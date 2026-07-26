pub mod bind_groups;
pub mod bloom;
pub mod camera;
pub mod debug_lines;
pub mod ibl;
pub mod light;
pub mod object_uniforms;
pub mod overlay;
pub(crate) mod pipeline;
pub mod post_process;
#[allow(clippy::module_inception)]
pub mod renderer;
pub mod sky;
pub mod ssao;

pub mod debug_ui;

pub use camera::*;
pub use debug_lines::*;
pub use ibl::*;
pub use light::*;
pub use overlay::*;
pub use post_process::*;
pub use renderer::*;
pub use sky::*;
pub use ssao::*;

use crate::asset::{Handle, Material, Mesh};

pub struct RenderMesh {
    pub mesh: Handle<Mesh>,
    pub material: Handle<Material>,
}
