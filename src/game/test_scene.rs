use crate::asset::{AssetManager, ProceduralGenerator};
use crate::core::{EngineWorld, Transform};
use crate::renderer::{Light, LightType};
use glam::Vec3;
use hecs::Entity;

pub struct TestScene {
    pub ground: Entity,
    pub walls: Vec<Entity>,
    pub targets: Vec<Entity>,
    pub lights: Vec<Entity>,
}

impl TestScene {
    pub fn create(world: &mut EngineWorld, asset_manager: &mut AssetManager) -> Self {
        // Create meshes
        let _cube_mesh = asset_manager
            .meshes
            .insert(ProceduralGenerator::create_cube());
        let plane_mesh = asset_manager
            .meshes
            .insert(ProceduralGenerator::create_plane(100.0, 10));
        let _sphere_mesh = asset_manager
            .meshes
            .insert(ProceduralGenerator::create_sphere(32, 16));

        let gray_material = asset_manager
            .materials
            .insert(crate::asset::Material::gray());
        let _white_material = asset_manager
            .materials
            .insert(crate::asset::Material::white());
        let _metal_material = asset_manager
            .materials
            .insert(crate::asset::Material::metal([0.8, 0.8, 0.9]));

        // Create ground
        let ground = world.spawn();
        world.add_component(ground, Transform::from_position(Vec3::new(0.0, -0.5, 0.0)));
        world.add_component(
            ground,
            crate::renderer::RenderMesh {
                mesh: plane_mesh,
                material: gray_material,
            },
        );

        // Create walls
        let mut walls = Vec::new();
        for i in 0..5 {
            let wall = world.spawn();
            let wall_cube_mesh = asset_manager
                .meshes
                .insert(ProceduralGenerator::create_cube());
            let wall_material = asset_manager
                .materials
                .insert(crate::asset::Material::white());
            world.add_component(
                wall,
                Transform::new(
                    Vec3::new(i as f32 * 3.0 - 6.0, 1.5, -5.0),
                    glam::Quat::IDENTITY,
                    Vec3::new(2.0, 3.0, 0.2),
                ),
            );
            world.add_component(
                wall,
                crate::renderer::RenderMesh {
                    mesh: wall_cube_mesh,
                    material: wall_material,
                },
            );
            walls.push(wall);
        }

        // Create targets
        let mut targets = Vec::new();
        for i in 0..3 {
            let target = world.spawn();
            let target_cube_mesh = asset_manager
                .meshes
                .insert(ProceduralGenerator::create_cube());
            let target_material = asset_manager
                .materials
                .insert(crate::asset::Material::metal([0.8, 0.8, 0.9]));
            world.add_component(
                target,
                Transform::new(
                    Vec3::new(i as f32 * 4.0 - 4.0, 1.0, -10.0),
                    glam::Quat::IDENTITY,
                    Vec3::new(1.0, 1.0, 0.1),
                ),
            );
            world.add_component(
                target,
                crate::renderer::RenderMesh {
                    mesh: target_cube_mesh,
                    material: target_material,
                },
            );
            targets.push(target);
        }

        // Create lights
        let mut lights = Vec::new();

        // Directional light
        let sun = world.spawn();
        world.add_component(
            sun,
            Light {
                position: Vec3::new(10.0, 20.0, 10.0),
                color: [1.0, 0.95, 0.9],
                intensity: 1.0,
                light_type: LightType::Directional,
                range: f32::MAX,
                inner_angle: 0.0,
                outer_angle: 0.0,
            },
        );
        lights.push(sun);

        // Point lights
        for i in 0..3 {
            let light = world.spawn();
            world.add_component(
                light,
                Light {
                    position: Vec3::new(i as f32 * 5.0 - 5.0, 3.0, -3.0),
                    color: [1.0, 0.8, 0.6],
                    intensity: 50.0,
                    light_type: LightType::Point,
                    range: 10.0,
                    inner_angle: 0.0,
                    outer_angle: 0.0,
                },
            );
            lights.push(light);
        }

        TestScene {
            ground,
            walls,
            targets,
            lights,
        }
    }
}
