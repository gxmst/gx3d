//! Data-driven scene description.
//!
//! A [`Scene`] is the *content* of a level — its materials, geometry, lights,
//! and enemy layout — expressed as plain data that can be authored in JSON
//! without touching Rust. The engine *plumbing* (procedural mesh generation,
//! the player capsule, weapons, GPU upload, audio) stays in code and is
//! referenced from a scene by name.
//!
//! See [`spawn`](crate::game::scene::spawn) for instantiation into the world,
//! and `assets/scenes/sandbox.json` for the default scene.

mod spawn;

pub use spawn::spawn_scene;

use serde::Deserialize;

/// A complete, data-driven description of a level.
#[derive(Debug, Clone, Deserialize)]
pub struct Scene {
    /// Player spawn / camera configuration.
    #[serde(default)]
    pub player: PlayerDesc,
    /// Meshes referenced by entities, each bound to a procedural generator or
    /// a glTF source by name.
    #[serde(default)]
    pub meshes: Vec<MeshDesc>,
    /// Materials referenced by entities, by name.
    #[serde(default)]
    pub materials: Vec<MaterialDesc>,
    /// Lights in the scene.
    #[serde(default)]
    pub lights: Vec<LightDesc>,
    /// Static and dynamic props that make up the level geometry.
    #[serde(default)]
    pub entities: Vec<EntityDesc>,
    /// Optional enemy layout.
    #[serde(default)]
    pub enemies: Option<EnemyDesc>,
}

/// Player spawn configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PlayerDesc {
    /// World-space spawn position.
    #[serde(default = "default_player_position")]
    pub position: [f32; 3],
    /// Eye height above the capsule base.
    #[serde(default = "default_player_height")]
    pub height: f32,
    /// Movement speed in units/second.
    #[serde(default = "default_move_speed")]
    pub move_speed: f32,
    /// Mouse look sensitivity (radians per pixel).
    #[serde(default = "default_mouse_sensitivity")]
    pub mouse_sensitivity: f32,
}

impl Default for PlayerDesc {
    fn default() -> Self {
        Self {
            position: default_player_position(),
            height: default_player_height(),
            move_speed: default_move_speed(),
            mouse_sensitivity: default_mouse_sensitivity(),
        }
    }
}

/// Which procedural generator (or glTF file) backs a named mesh.
#[derive(Debug, Clone, Deserialize)]
pub struct MeshDesc {
    /// Name used to reference this mesh from entities.
    pub name: String,
    /// Source generator. See [`MeshSource`].
    pub source: MeshSource,
}

/// The origin of a mesh's geometry.
///
/// Tagged by the `source` field, e.g. `{"source": "cube"}` or
/// `{"source": {"sphere": {"segments": 16, "rings": 8}}}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshSource {
    /// Unit cube centered at the origin.
    Cube,
    /// UV sphere.
    Sphere {
        #[serde(default = "default_sphere_segments")]
        segments: u32,
        #[serde(default = "default_sphere_rings")]
        rings: u32,
    },
    /// Capped cylinder along Y.
    Cylinder {
        #[serde(default = "default_cylinder_segments")]
        segments: u32,
    },
    /// Subdivided ground plane.
    Plane {
        #[serde(default = "default_plane_size")]
        size: f32,
        #[serde(default)]
        subdivisions: u32,
    },
    /// The built-in procedural rifle mesh.
    Rifle,
    /// Triangular prism useful for ramps, broken masonry and roof pieces.
    Wedge,
    /// Simple articulated-looking humanoid assembled into one render mesh.
    Humanoid,
    /// A flattened primitive from a glTF/GLB scene. Node transforms are baked
    /// by the loader; `primitive` indexes the imported primitive list.
    Gltf {
        path: String,
        #[serde(default)]
        primitive: usize,
    },
}

/// A PBR material defined inline as constant factors (no textures).
///
/// This is the bread-and-butter of the stylized/minimal art direction: flat
/// color blocks shaped by lighting and post-processing.
#[derive(Debug, Clone, Deserialize)]
pub struct MaterialDesc {
    /// Name used to reference this material from entities.
    pub name: String,
    /// Linear RGB albedo.
    pub color: [f32; 3],
    /// Alpha (defaults to opaque).
    #[serde(default = "default_alpha")]
    pub alpha: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    #[serde(default)]
    pub metallic: f32,
    /// Emissive RGB (HDR; values above 1.0 glow through bloom).
    #[serde(default)]
    pub emissive: [f32; 3],
}

/// A light source.
///
/// Tagged by the `type` field: `directional`, `point`, or `spot`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LightDesc {
    Directional {
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
    },
    Point {
        position: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        range: f32,
    },
    Spot {
        position: [f32; 3],
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    },
}

/// A single placed entity: a mesh + material with an optional physics body.
#[derive(Debug, Clone, Deserialize)]
pub struct EntityDesc {
    /// Optional human-readable name (shown in logs / debugging).
    #[serde(default)]
    pub name: Option<String>,
    /// Name of a mesh declared in [`Scene::meshes`].
    pub mesh: String,
    /// Name of a material declared in [`Scene::materials`].
    pub material: String,
    /// Placement in the world.
    pub transform: TransformDesc,
    /// Optional physics body. Absent means render-only (no collider).
    #[serde(default)]
    pub physics: Option<PhysicsDesc>,
    #[serde(default)]
    pub interaction: Option<InteractionDesc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InteractionDesc {
    Door {
        open_rotation: [f32; 3],
        #[serde(default = "default_door_speed")]
        speed: f32,
    },
}

/// Placement of an entity. Rotation is in **degrees** (Euler XYZ) so scenes
/// stay human-authorable; scale defaults to uniform 1.
#[derive(Debug, Clone, Deserialize)]
pub struct TransformDesc {
    pub position: [f32; 3],
    /// Euler rotation in degrees (X, Y, Z), applied in that order.
    #[serde(default)]
    pub rotation: [f32; 3],
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
}

/// Physics body attached to an entity.
#[derive(Debug, Clone, Deserialize)]
pub struct PhysicsDesc {
    /// Collider shape.
    pub shape: ShapeDesc,
    /// Mass in kg. `0` (or less) makes the body static.
    #[serde(default)]
    pub mass: f32,
    /// Surface material: a preset name or custom friction/restitution.
    #[serde(default)]
    pub surface: SurfaceDesc,
    /// One-shot impulse applied on spawn (e.g. to launch a puck).
    #[serde(default)]
    pub initial_impulse: [f32; 3],
}

/// A collider shape, mirroring [`crate::physics::PhysicsShape`].
///
/// Tagged: `{"shape": {"cuboid": [hx, hy, hz]}}`,
/// `{"shape": {"sphere": 0.5}}`,
/// `{"shape": {"capsule": [radius, half_height]}}`,
/// `{"shape": {"cylinder": [radius, half_height]}}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeDesc {
    Sphere(f32),
    Cuboid([f32; 3]),
    Capsule([f32; 2]),
    Cylinder([f32; 2]),
    Wedge([f32; 3]),
}

/// A physics surface material: either a named preset or custom values.
///
/// Authored as a string preset — `"default"` | `"ice"` | `"rubber"` |
/// `"metal"` — or as a bare object `{"friction": 0.5, "restitution": 0.1}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SurfaceDesc {
    /// One of the named presets.
    Preset(SurfacePreset),
    /// Explicit friction/restitution values.
    Custom { friction: f32, restitution: f32 },
}

impl Default for SurfaceDesc {
    fn default() -> Self {
        SurfaceDesc::Preset(SurfacePreset::Default)
    }
}

/// Named physics surface presets, mirroring [`crate::physics::PhysicsMaterial`].
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfacePreset {
    #[default]
    Default,
    Ice,
    Rubber,
    Metal,
}

/// Enemy spawn layout.
#[derive(Debug, Clone, Deserialize)]
pub struct EnemyDesc {
    /// Mesh name for enemies.
    pub mesh: String,
    /// Material name for enemies.
    pub material: String,
    /// Positions where enemies spawn.
    #[serde(default)]
    pub spawn_points: Vec<[f32; 3]>,
    /// Shared patrol waypoints.
    #[serde(default)]
    pub waypoints: Vec<[f32; 3]>,
}

impl Scene {
    /// Parse a scene from JSON text.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

// --- serde defaults -------------------------------------------------------

fn default_player_position() -> [f32; 3] {
    [0.0, 2.0, 5.0]
}
fn default_player_height() -> f32 {
    1.6
}
fn default_move_speed() -> f32 {
    8.0
}
fn default_mouse_sensitivity() -> f32 {
    0.002
}
fn default_alpha() -> f32 {
    1.0
}
fn default_roughness() -> f32 {
    0.5
}
fn default_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}
fn default_sphere_segments() -> u32 {
    16
}
fn default_sphere_rings() -> u32 {
    8
}
fn default_cylinder_segments() -> u32 {
    24
}
fn default_plane_size() -> f32 {
    1.0
}
fn default_door_speed() -> f32 {
    4.5
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scene shipped in `assets/scenes/sandbox.json` is also embedded in the
    /// binary as the startup fallback. Guard that it always parses and matches
    /// the hand-authored sandbox layout, so a typo in the JSON fails the build
    /// instead of silently falling back at runtime.
    #[test]
    fn embedded_sandbox_scene_parses() {
        let text = include_str!("../../../assets/scenes/sandbox.json");
        let scene = Scene::from_json(text).expect("sandbox.json must parse");
        assert_eq!(scene.meshes.len(), 4, "cube/sphere/cylinder/rifle");
        assert_eq!(scene.materials.len(), 10);
        assert_eq!(scene.lights.len(), 4, "1 directional + 3 point");
        // 1 ground + 4 walls + 5 boxes + ramp + ice lane + 4 balls
        // + light box + heavy box + 2 pucks + 8 targets = 28 entities.
        assert_eq!(scene.entities.len(), 28);
        let enemies = scene.enemies.expect("scene declares enemies");
        assert_eq!(enemies.spawn_points.len(), 3);
        assert_eq!(enemies.waypoints.len(), 4);
    }

    #[test]
    fn surface_preset_and_custom_both_parse() {
        let scene = Scene::from_json(
            r#"{
                "entities": [
                    { "mesh": "m", "material": "x",
                      "transform": { "position": [0,0,0] },
                      "physics": { "shape": { "sphere": 0.5 }, "surface": "ice" } },
                    { "mesh": "m", "material": "x",
                      "transform": { "position": [0,0,0] },
                      "physics": { "shape": { "cuboid": [1,1,1] },
                                   "surface": { "friction": 0.6, "restitution": 0.1 } } }
                ]
            }"#,
        )
        .expect("custom + preset surfaces must parse");
        assert_eq!(scene.entities.len(), 2);
    }

    /// The dust2-style level is the startup scene and is embedded as the
    /// fallback, so guard that it parses and references the engine-plumbing
    /// names (`cube`/`sphere`/`rifle`, `box`/`ice`/...) that setup depends on.
    #[test]
    fn embedded_dust2_scene_parses() {
        let text = include_str!("../../../assets/scenes/dust2.json");
        let scene = Scene::from_json(text).expect("dust2.json must parse");
        assert_eq!(scene.entities.len(), 99);
        let mesh_names: Vec<&str> = scene.meshes.iter().map(|m| m.name.as_str()).collect();
        for required in ["cube", "sphere", "cylinder", "rifle"] {
            assert!(mesh_names.contains(&required), "missing mesh `{required}`");
        }
        let mat_names: Vec<&str> = scene.materials.iter().map(|m| m.name.as_str()).collect();
        for required in ["box", "bouncy_rubber", "ice", "heavy_metal"] {
            assert!(
                mat_names.contains(&required),
                "missing sandbox material `{required}` (runtime G/B/H spawns need it)"
            );
        }
        let enemies = scene.enemies.expect("dust2 declares enemies");
        assert_eq!(enemies.spawn_points.len(), 4);
    }
}
