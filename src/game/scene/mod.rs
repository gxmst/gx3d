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

/// One selectable scene on disk.
#[derive(Debug, Clone)]
pub struct SceneEntry {
    /// Display name (file stem).
    pub name: String,
    pub path: std::path::PathBuf,
}

/// All scenes found in `assets/scenes/*.json`, sorted by name. Scanned once
/// at startup; drives the pause menu's scene selector.
#[derive(Debug, Clone, Default)]
pub struct SceneLibrary(pub Vec<SceneEntry>);

impl SceneLibrary {
    pub fn scan() -> Self {
        let mut entries = Vec::new();
        if let Ok(dir) = std::fs::read_dir("assets/scenes") {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        entries.push(SceneEntry {
                            name: stem.to_string(),
                            path,
                        });
                    }
                }
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Self(entries)
    }

    /// Index of the entry matching `path` (by file stem), if any.
    pub fn index_of(&self, path: &std::path::Path) -> Option<usize> {
        let stem = path.file_stem()?.to_str()?;
        self.0.iter().position(|entry| entry.name == stem)
    }
}

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
    /// Optional tornado force field (physics showcase).
    #[serde(default)]
    pub tornado: Option<TornadoDesc>,
    /// Optional animated water surface with buoyancy (physics showcase).
    #[serde(default)]
    pub water: Option<WaterDesc>,
    /// Optional destructible block structures (physics showcase).
    #[serde(default)]
    pub structures: Vec<StructureDesc>,
    /// Optional team-vs-team bot match (CS-style rounds). Presence of this
    /// block switches the scene into match mode: `enemies` is ignored.
    #[serde(default)]
    pub match_mode: Option<MatchDesc>,
}

/// A 5v5-style bot match: the player joins `team_a` at its first spawn.
#[derive(Debug, Clone, Deserialize)]
pub struct MatchDesc {
    /// Mesh used for every bot (usually `humanoid`).
    pub mesh: String,
    /// Material for the player's team (team A) bots.
    pub team_a_material: String,
    /// Material for the opposing team (team B) bots.
    pub team_b_material: String,
    /// Spawns for team A. The FIRST entry is the player spawn; bots fill the
    /// rest. Team size = spawns declared.
    pub team_a_spawns: Vec<[f32; 3]>,
    /// Spawns for team B (all bots).
    pub team_b_spawns: Vec<[f32; 3]>,
    /// Patrol/objective waypoints bots roam between when no enemy is visible.
    /// Shared by both teams; bots pick nearby points to push through the map.
    #[serde(default)]
    pub waypoints: Vec<[f32; 3]>,
    /// Rounds needed to win the match (first to N).
    #[serde(default = "default_rounds_to_win")]
    pub rounds_to_win: u32,
    /// C4 bomb site center; None disables the objective.
    #[serde(default)]
    pub bomb_site: Option<[f32; 3]>,
    /// Bomb site radius.
    #[serde(default = "default_bomb_site_radius")]
    pub bomb_site_radius: f32,
}

/// A vortex force field that lifts and spins dynamic bodies.
#[derive(Debug, Clone, Deserialize)]
pub struct TornadoDesc {
    /// Base position of the funnel axis (ground level).
    pub center: [f32; 3],
    /// Influence radius around the axis.
    #[serde(default = "default_tornado_radius")]
    pub radius: f32,
    /// Influence height above `center`.
    #[serde(default = "default_tornado_height")]
    pub height: f32,
    /// Peak tangential force in newtons applied near the core.
    #[serde(default = "default_tornado_strength")]
    pub strength: f32,
    /// How far the funnel base wanders from `center` over time.
    #[serde(default = "default_tornado_wander")]
    pub wander: f32,
}

/// An animated water plane. Dynamic bodies inside its bounds receive
/// buoyancy from the same wave function that displaces the surface mesh.
#[derive(Debug, Clone, Deserialize)]
pub struct WaterDesc {
    /// Center of the water surface (y = rest water level).
    pub center: [f32; 3],
    /// Edge length of the square surface.
    #[serde(default = "default_water_size")]
    pub size: f32,
    /// Grid subdivisions of the surface mesh per side.
    #[serde(default = "default_water_subdivisions")]
    pub subdivisions: u32,
    /// Wave amplitude in meters.
    #[serde(default = "default_water_amplitude")]
    pub amplitude: f32,
    /// Name of a material declared in [`Scene::materials`].
    pub material: String,
}

/// A grid of blocks spawned frozen-in-place; explosions and impacts unfreeze
/// them so the structure collapses piece by piece.
#[derive(Debug, Clone, Deserialize)]
pub struct StructureDesc {
    /// Center of the structure's base (bottom face).
    pub position: [f32; 3],
    /// Half-extents of one block.
    pub block_half_extents: [f32; 3],
    /// Block count along X, Y (up), Z.
    pub blocks: [u32; 3],
    /// If true only the perimeter walls are built (hollow tower).
    #[serde(default)]
    pub hollow: bool,
    /// Mass per block in kg.
    #[serde(default = "default_structure_block_mass")]
    pub block_mass: f32,
    /// Name of a mesh declared in [`Scene::meshes`] (usually `cube`).
    pub mesh: String,
    /// Name of a material declared in [`Scene::materials`].
    pub material: String,
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
    /// Water shading path: fresnel + animated ripples + foam + transparency.
    #[serde(default)]
    pub water: bool,
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
        /// Local-space centre-to-hinge offset. Zero preserves legacy
        /// centre-pivoted doors.
        #[serde(default)]
        hinge_offset: [f32; 3],
        #[serde(default = "default_door_speed")]
        speed: f32,
    },
    /// Detonates when shot, applying a radial impulse to nearby dynamic bodies.
    Explosive {
        #[serde(default = "default_explosion_radius")]
        radius: f32,
        #[serde(default = "default_explosion_impulse")]
        impulse: f32,
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
    /// Optional route per spawn point. A missing/empty route falls back to the
    /// shared `waypoints` list for backwards compatibility.
    #[serde(default)]
    pub routes: Vec<Vec<[f32; 3]>>,
}

impl Scene {
    /// Parse a scene from JSON text.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Non-fatal authoring diagnostics. Scene loading remains forgiving, but
    /// mistakes are reported with entity context instead of becoming missing
    /// geometry, NaNs, or confusing fallback behavior later in the frame.
    pub fn validation_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if !self.player.position.into_iter().all(f32::is_finite)
            || !self.player.height.is_finite()
            || !self.player.move_speed.is_finite()
            || !self.player.mouse_sensitivity.is_finite()
            || self.player.height <= 0.0
            || self.player.move_speed <= 0.0
            || self.player.mouse_sensitivity <= 0.0
        {
            warnings.push("player configuration contains invalid values".to_string());
        }
        let mut meshes = std::collections::HashMap::new();
        for mesh in &self.meshes {
            if meshes
                .insert(
                    mesh.name.as_str(),
                    matches!(&mesh.source, MeshSource::Gltf { .. }),
                )
                .is_some()
            {
                warnings.push(format!("duplicate mesh name `{}`", mesh.name));
            }
            match &mesh.source {
                MeshSource::Sphere { segments, rings } if *segments < 3 || *rings < 2 => {
                    warnings.push(format!(
                        "mesh `{}` has too few sphere segments/rings",
                        mesh.name
                    ));
                }
                MeshSource::Cylinder { segments } if *segments < 3 => {
                    warnings.push(format!(
                        "mesh `{}` has too few cylinder segments",
                        mesh.name
                    ));
                }
                MeshSource::Plane { size, subdivisions }
                    if !size.is_finite() || *size <= 0.0 || *subdivisions == 0 =>
                {
                    warnings.push(format!("mesh `{}` has invalid plane parameters", mesh.name));
                }
                _ => {}
            }
        }
        let mut materials = std::collections::HashSet::new();
        for material in &self.materials {
            if !materials.insert(material.name.as_str()) {
                warnings.push(format!("duplicate material name `{}`", material.name));
            }
            if !material.color.into_iter().all(f32::is_finite)
                || !material.alpha.is_finite()
                || !material.roughness.is_finite()
                || !material.metallic.is_finite()
                || !material.emissive.into_iter().all(f32::is_finite)
            {
                warnings.push(format!(
                    "material `{}` contains non-finite values",
                    material.name
                ));
            }
        }
        for (index, light) in self.lights.iter().enumerate() {
            let valid = match light {
                LightDesc::Directional {
                    direction,
                    color,
                    intensity,
                } => {
                    direction.iter().copied().all(f32::is_finite)
                        && direction.iter().any(|axis| axis.abs() > f32::EPSILON)
                        && color.iter().copied().all(f32::is_finite)
                        && intensity.is_finite()
                        && *intensity >= 0.0
                }
                LightDesc::Point {
                    position,
                    color,
                    intensity,
                    range,
                } => {
                    position.iter().copied().all(f32::is_finite)
                        && color.iter().copied().all(f32::is_finite)
                        && intensity.is_finite()
                        && *intensity >= 0.0
                        && range.is_finite()
                        && *range > 0.0
                }
                LightDesc::Spot {
                    position,
                    direction,
                    color,
                    intensity,
                    range,
                    inner_angle,
                    outer_angle,
                } => {
                    position.iter().copied().all(f32::is_finite)
                        && direction.iter().copied().all(f32::is_finite)
                        && direction.iter().any(|axis| axis.abs() > f32::EPSILON)
                        && color.iter().copied().all(f32::is_finite)
                        && intensity.is_finite()
                        && *intensity >= 0.0
                        && range.is_finite()
                        && *range > 0.0
                        && inner_angle.is_finite()
                        && outer_angle.is_finite()
                        && *inner_angle >= 0.0
                        && *outer_angle >= *inner_angle
                }
            };
            if !valid {
                warnings.push(format!("light #{index} contains invalid values"));
            }
        }
        let mut entity_names = std::collections::HashSet::new();
        for (index, entity) in self.entities.iter().enumerate() {
            let label = entity.name.as_deref().unwrap_or("unnamed");
            if let Some(name) = entity.name.as_deref() {
                if !entity_names.insert(name) {
                    warnings.push(format!("duplicate entity name `{name}`"));
                }
            }
            let is_gltf = meshes.get(entity.mesh.as_str()).copied();
            if is_gltf.is_none() {
                warnings.push(format!(
                    "entity #{index} `{label}` references missing mesh `{}`",
                    entity.mesh
                ));
            }
            if entity.material == "$gltf" {
                if is_gltf == Some(false) {
                    warnings.push(format!(
                        "entity #{index} `{label}` requests `$gltf` on a procedural mesh"
                    ));
                }
            } else if !materials.contains(entity.material.as_str()) {
                warnings.push(format!(
                    "entity #{index} `{label}` references missing material `{}`",
                    entity.material
                ));
            }
            if !entity.transform.position.into_iter().all(f32::is_finite)
                || !entity.transform.rotation.into_iter().all(f32::is_finite)
                || !entity.transform.scale.into_iter().all(f32::is_finite)
            {
                warnings.push(format!(
                    "entity #{index} `{label}` has a non-finite transform"
                ));
            }
            if entity
                .transform
                .scale
                .into_iter()
                .any(|axis| axis.abs() < 0.0001)
            {
                warnings.push(format!("entity #{index} `{label}` has a zero scale axis"));
            }
            if let Some(physics) = &entity.physics {
                if !physics.mass.is_finite()
                    || !physics.initial_impulse.into_iter().all(f32::is_finite)
                {
                    warnings.push(format!(
                        "entity #{index} `{label}` has invalid physics values"
                    ));
                }
                if let SurfaceDesc::Custom {
                    friction,
                    restitution,
                } = physics.surface
                {
                    if !friction.is_finite()
                        || friction < 0.0
                        || !restitution.is_finite()
                        || !(0.0..=1.0).contains(&restitution)
                    {
                        warnings.push(format!(
                            "entity #{index} `{label}` has invalid surface parameters"
                        ));
                    }
                }
            }
            if let Some(interaction) = &entity.interaction {
                if entity.physics.is_none() {
                    warnings.push(format!(
                        "entity #{index} `{label}` is interactive but has no physics body"
                    ));
                }
                let invalid = match interaction {
                    InteractionDesc::Door {
                        open_rotation,
                        hinge_offset,
                        speed,
                    } => {
                        !open_rotation.iter().copied().all(f32::is_finite)
                            || !hinge_offset.iter().copied().all(f32::is_finite)
                            || !speed.is_finite()
                            || *speed <= 0.0
                    }
                    InteractionDesc::Explosive { radius, impulse } => {
                        !radius.is_finite()
                            || *radius <= 0.0
                            || !impulse.is_finite()
                            || *impulse <= 0.0
                    }
                };
                if invalid {
                    warnings.push(format!(
                        "entity #{index} `{label}` has invalid interaction parameters"
                    ));
                }
            }
        }
        if let Some(enemies) = &self.enemies {
            if !enemies.routes.is_empty() && enemies.routes.len() != enemies.spawn_points.len() {
                warnings.push(format!(
                    "enemy route count ({}) does not match spawn count ({})",
                    enemies.routes.len(),
                    enemies.spawn_points.len()
                ));
            }
            if enemies
                .spawn_points
                .iter()
                .chain(enemies.waypoints.iter())
                .chain(enemies.routes.iter().flatten())
                .any(|point| !point.iter().copied().all(f32::is_finite))
            {
                warnings.push("enemy layout contains non-finite coordinates".to_string());
            }
        }
        warnings
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
fn default_tornado_radius() -> f32 {
    12.0
}
fn default_tornado_height() -> f32 {
    28.0
}
fn default_tornado_strength() -> f32 {
    260.0
}
fn default_tornado_wander() -> f32 {
    6.0
}
fn default_water_size() -> f32 {
    80.0
}
fn default_water_subdivisions() -> u32 {
    96
}
fn default_water_amplitude() -> f32 {
    0.55
}
fn default_structure_block_mass() -> f32 {
    35.0
}
fn default_rounds_to_win() -> u32 {
    5
}
fn default_bomb_site_radius() -> f32 {
    6.0
}
fn default_explosion_radius() -> f32 {
    5.0
}
fn default_explosion_impulse() -> f32 {
    18.0
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
        assert!(
            scene.entities.len() >= 180,
            "the dust2 layout should retain its generated geometry (tools/gen_dust2.py)"
        );
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
        let warnings = scene.validation_warnings();
        assert!(
            warnings.is_empty(),
            "dust2 should be warning-free after authoring validation: {warnings:#?}"
        );
        // dust2 is a 5v5 match scene: match_mode supersedes patrol enemies.
        let match_mode = scene.match_mode.expect("dust2 declares a match block");
        assert_eq!(match_mode.team_a_spawns.len(), 5, "player + 4 teammates");
        assert_eq!(match_mode.team_b_spawns.len(), 5);
        assert!(match_mode.bomb_site.is_some(), "dust2 has a C4 objective");
        assert!(scene.enemies.is_none());
    }

    #[test]
    fn explosive_interaction_uses_safe_defaults() {
        let interaction: InteractionDesc = serde_json::from_str(r#"{"type":"explosive"}"#).unwrap();
        match interaction {
            InteractionDesc::Explosive { radius, impulse } => {
                assert_eq!(radius, 5.0);
                assert_eq!(impulse, 18.0);
            }
            _ => panic!("expected explosive interaction"),
        }
    }

    #[test]
    fn validation_reports_missing_references_without_rejecting_scene() {
        let scene = Scene::from_json(
            r#"{
                "meshes": [{"name":"cube","source":"cube"}],
                "materials": [],
                "entities": [{
                    "name":"bad prop",
                    "mesh":"missing",
                    "material":"missing",
                    "transform":{"position":[0,0,0],"scale":[1,0,1]}
                }]
            }"#,
        )
        .unwrap();
        let warnings = scene.validation_warnings();
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("missing mesh")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("missing material")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("zero scale")));
    }
}
