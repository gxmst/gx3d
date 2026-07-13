use crate::asset::{AssetManager, Handle, Mesh};
use crate::core::{EngineWorld, Resources, Time};
use crate::game::WeaponModel;
use crate::input::InputState;
use crate::renderer::bind_groups::{LightData, MAX_LIGHTS};
use crate::renderer::{
    Camera, HudMode, HudState, Light, LightType, OverlayRuntimeInfo, RenderMesh, Renderer,
};
use glam::{Mat4, Vec2, Vec3};
use winit::keyboard::KeyCode;

use super::{HitMarkerTimer, MenuState, MuzzleFlashTimer, SceneLights, WeaponFeedbackAssets};

struct DrawItem {
    mesh: Handle<Mesh>,
    material_bind_group: wgpu::BindGroup,
    model: Mat4,
    dynamic_offset: u32,
}

struct RapierLineCollector(Vec<crate::renderer::DebugLine>);

impl rapier3d::prelude::DebugRenderBackend for RapierLineCollector {
    fn draw_line(
        &mut self,
        _object: rapier3d::prelude::DebugRenderObject,
        a: rapier3d::prelude::Vector,
        b: rapier3d::prelude::Vector,
        color: rapier3d::prelude::DebugColor,
    ) {
        self.0.push(crate::renderer::DebugLine {
            start: glam::Vec3::new(a.x, a.y, a.z),
            end: glam::Vec3::new(b.x, b.y, b.z),
            color: [color[0], color[1], color[2], 0.9],
        });
    }
}

pub fn system(world: &mut EngineWorld, resources: &Resources) {
    let renderer = resources.expect::<Renderer>();
    let asset_manager = resources.expect::<AssetManager>();
    let camera = resources.expect::<Camera>();
    let time = resources.expect::<Time>();
    let scene_lights = resources.expect::<SceneLights>();
    let weapon_model = resources.expect::<WeaponModel>();
    let weapon = resources.expect::<crate::game::Weapon>();
    let menu_open = resources
        .get::<MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false);
    let show_help = resources
        .get::<InputState>()
        .map(|input| input.is_key_pressed(KeyCode::KeyF) && !menu_open)
        .unwrap_or(false);
    let day_phase = (time.elapsed_seconds() / 360.0) * std::f32::consts::TAU + 0.75;
    let sun_to = glam::Vec3::new(day_phase.cos(), day_phase.sin(), 0.28).normalize();
    let daylight = (sun_to.y * 1.8 + 0.15).clamp(0.04, 1.0);

    // The shader accepts eight lights. Always retain directional lighting and
    // fill the remaining slots with local lights nearest the active camera.
    let light_data = select_light_data(&scene_lights.0, camera.position, sun_to, daylight);

    // Resolve and clone cached material bind groups up front.
    let mut draws: Vec<DrawItem> = Vec::new();
    let world_draw_count;
    {
        let material_cache = renderer.material_bind_groups.borrow();
        for (transform, render_mesh) in world
            .ecs
            .query::<(&crate::core::Transform, &RenderMesh)>()
            .iter()
        {
            let material_bind_group = material_cache
                .get(&render_mesh.material.id)
                .expect("Material bind group not cached; did setup run?")
                .clone();
            draws.push(DrawItem {
                mesh: render_mesh.mesh,
                material_bind_group,
                model: transform.matrix(),
                dynamic_offset: 0,
            });
        }
        world_draw_count = draws.len();
        let material_bind_group = material_cache
            .get(&weapon_model.material.id)
            .expect("Weapon material bind group not cached")
            .clone();
        draws.push(DrawItem {
            mesh: weapon_model.mesh,
            material_bind_group,
            model: weapon_model.world_matrix(&camera),
            dynamic_offset: 0,
        });
        if let (Some(timer), Some(feedback_assets)) = (
            resources.get::<MuzzleFlashTimer>(),
            resources.get::<WeaponFeedbackAssets>(),
        ) {
            if timer.0 > 0.0 {
                let material_bind_group = material_cache
                    .get(&feedback_assets.muzzle_flash_material.id)
                    .expect("Muzzle flash material bind group not cached")
                    .clone();
                let flash_scale = 0.08 + (timer.0 / 0.06).clamp(0.0, 1.0) * 0.08;
                draws.push(DrawItem {
                    mesh: feedback_assets.muzzle_flash_mesh,
                    material_bind_group,
                    model: weapon_model.muzzle_world_matrix(&camera, flash_scale),
                    dynamic_offset: 0,
                });
            }
        }
    }

    render_frame(
        renderer,
        asset_manager,
        camera,
        time,
        scene_lights,
        weapon,
        menu_open,
        show_help,
        weapon_model.aim_blend > 0.5,
        sun_to,
        daylight,
        light_data,
        draws,
        world_draw_count,
        resources,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_frame(
    renderer: std::cell::Ref<'_, Renderer>,
    asset_manager: std::cell::Ref<'_, AssetManager>,
    camera: std::cell::Ref<'_, Camera>,
    time: std::cell::Ref<'_, Time>,
    scene_lights: std::cell::Ref<'_, SceneLights>,
    weapon: std::cell::Ref<'_, crate::game::Weapon>,
    menu_open: bool,
    show_help: bool,
    aiming: bool,
    sun_to: glam::Vec3,
    daylight: f32,
    light_data: Vec<LightData>,
    mut draws: Vec<DrawItem>,
    world_draw_count: usize,
    resources: &Resources,
) {
    renderer.begin_object_uniform_frame();
    renderer.ensure_object_uniform_capacity(draws.len());
    for item in &mut draws {
        item.dynamic_offset = renderer.allocate_object_uniform(item.model);
    }

    // Compute shadow matrix from the first directional light.
    let directional = scene_lights
        .0
        .iter()
        .find(|l| l.light_type == LightType::Directional);
    let light_space_matrix = directional
        .map(|_| compute_light_space_matrix(-sun_to, camera.position, camera.forward()))
        .unwrap_or(Mat4::IDENTITY);

    // Shadow pass: render world objects only.
    {
        renderer.update_shadow_uniforms(light_space_matrix);
        let mut encoder = renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Encoder"),
            });
        let mut shadow_pass = renderer.begin_shadow_pass(&mut encoder);
        for item in &draws[..world_draw_count] {
            let mesh = asset_manager
                .meshes
                .get(item.mesh)
                .expect("Mesh missing during shadow pass");
            renderer.render_shadow_mesh(&mut shadow_pass, mesh, item.dynamic_offset);
        }
        drop(shadow_pass);
        renderer.queue.submit(std::iter::once(encoder.finish()));
    }

    renderer.update_global_uniforms(
        &camera,
        time.elapsed_seconds(),
        &light_data,
        light_space_matrix,
    );

    // Draw main pass to HDR target.
    let mut encoder = renderer
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Main Encoder"),
        });
    resources.expect::<crate::renderer::SkyRenderer>().render(
        &renderer.queue,
        &mut encoder,
        &renderer.hdr_color_view,
        camera.view_projection_matrix(),
        sun_to,
        time.elapsed_seconds(),
        daylight,
    );
    let mut render_pass =
        renderer.create_render_pass(&mut encoder, &renderer.hdr_color_view, &renderer.depth_view);
    render_pass.set_pipeline(&renderer.pipeline);

    for item in &draws {
        let mesh = asset_manager
            .meshes
            .get(item.mesh)
            .expect("Mesh missing during render");
        renderer.render_mesh(
            &mut render_pass,
            mesh,
            &item.material_bind_group,
            item.dynamic_offset,
        );
    }

    drop(render_pass);

    // SSAO pass: uses the depth buffer from the main pass.
    renderer.ssao_pass.update_params(
        &renderer.queue,
        camera.projection_matrix(),
        Vec2::new(
            renderer.surface_config.width as f32,
            renderer.surface_config.height as f32,
        ),
    );
    let ao_view = renderer
        .ssao_pass
        .render(&renderer.device, &mut encoder, &renderer.depth_view);

    // Bloom pass.
    let bloom_view = renderer.render_bloom(&mut encoder, &renderer.hdr_color_view);

    // Post-process HDR (+ bloom + AO) to swapchain.
    let Some(output) = renderer.acquire_surface_texture() else {
        renderer.queue.submit(std::iter::once(encoder.finish()));
        return;
    };
    let output_view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    renderer.post_processor.render(
        &renderer.device,
        &mut encoder,
        &renderer.hdr_color_view,
        bloom_view,
        ao_view,
        &output_view,
    );
    draw_physics_debug(resources, &renderer, &camera, &mut encoder, &output_view);

    let view_mode = resources
        .get::<super::ViewModeState>()
        .map(|state| state.mode)
        .unwrap_or(super::ViewMode::Fps);
    let physics_debug = resources
        .get::<super::PhysicsDebugState>()
        .map(|state| *state)
        .unwrap_or_default();
    let (holding_object, hold_distance) = resources
        .get::<crate::physics::PhysicsSandbox>()
        .map(|sandbox| (sandbox.held_body.is_some(), sandbox.hold_distance))
        .unwrap_or((false, 0.0));
    let dynamic_body_count = resources
        .get::<crate::physics::PhysicsWorld>()
        .map(|physics| {
            physics
                .rigid_body_set
                .iter()
                .filter(|(_, body)| body.is_dynamic())
                .count()
        })
        .unwrap_or(0);
    let runtime = OverlayRuntimeInfo {
        mode: match view_mode {
            super::ViewMode::Fps => HudMode::Fps,
            super::ViewMode::God => HudMode::God,
        },
        fps: time.fps(),
        time_scale: time.time_scale,
        paused: time.is_paused(),
        debug_colliders: physics_debug.colliders,
        debug_velocities: physics_debug.velocities,
        debug_contacts: physics_debug.contacts,
        camera_position: camera.position.into(),
        light_count: scene_lights.0.len(),
        dynamic_body_count,
        holding_object,
        hold_distance,
    };

    if !menu_open {
        if let Some(focus) = resources.get::<crate::game::InteractionFocus>() {
            let hud = HudState {
                focus: &focus,
                weapon_name: &weapon.name,
                current_ammo: weapon.current_ammo,
                max_ammo: weapon.max_ammo,
                is_reloading: weapon.is_reloading,
                reload_timer: weapon.reload_timer,
                reload_duration: weapon.reload_duration,
                aiming,
                show_help,
                hit_marker_seconds: resources
                    .get::<HitMarkerTimer>()
                    .map(|timer| timer.0)
                    .unwrap_or(0.0),
                runtime,
            };
            renderer.overlay.borrow_mut().draw_hud(
                &renderer.device,
                &renderer.queue,
                &mut encoder,
                &output_view,
                &hud,
                renderer.surface_config.width,
                renderer.surface_config.height,
            );
        }
    }
    if menu_open {
        if let Some(menu) = resources.get::<MenuState>() {
            renderer.overlay.borrow_mut().draw_pause_menu(
                &renderer.device,
                &renderer.queue,
                &mut encoder,
                &output_view,
                &menu.0,
                runtime,
                renderer.surface_config.width,
                renderer.surface_config.height,
            );
        }
    }

    renderer.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    let mode_label = match view_mode {
        super::ViewMode::Fps => "FPS",
        super::ViewMode::God => "GOD",
    };
    let time_label = if time.is_paused() {
        "PAUSED".to_string()
    } else {
        format!("{:.2}x", time.time_scale)
    };

    if menu_open {
        renderer.window.set_title("GxEngine — 实验台设置");
    } else if show_help {
        renderer.window.set_title("GxEngine — 操作指南");
    } else {
        renderer.window.set_title(&format!(
            "GxEngine — {} · {} · {:.0} FPS",
            mode_label,
            time_label,
            time.fps(),
        ));
    }
}

fn draw_physics_debug(
    resources: &Resources,
    renderer: &Renderer,
    camera: &Camera,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
) {
    let state = resources
        .get::<super::PhysicsDebugState>()
        .map(|state| *state)
        .unwrap_or_default();
    if !(state.colliders || state.velocities || state.contacts) {
        return;
    }

    let physics = resources.expect::<crate::physics::PhysicsWorld>();
    let mut collector = RapierLineCollector(Vec::new());
    if state.colliders || state.contacts {
        use rapier3d::prelude::{DebugRenderMode, DebugRenderPipeline, DebugRenderStyle};
        let mut mode = DebugRenderMode::empty();
        if state.colliders {
            mode |= DebugRenderMode::COLLIDER_SHAPES;
        }
        if state.contacts {
            mode |= DebugRenderMode::CONTACTS;
        }
        DebugRenderPipeline::new(DebugRenderStyle::default(), mode).render(
            &mut collector,
            &physics.rigid_body_set,
            &physics.collider_set,
            &physics.impulse_joint_set,
            &physics.multibody_joint_set,
            &physics.narrow_phase,
        );
    }
    if state.velocities {
        for (_, body) in physics.rigid_body_set.iter() {
            if !body.is_dynamic() {
                continue;
            }
            let p = body.translation();
            let v = body.linvel();
            let start = glam::Vec3::new(p.x, p.y, p.z);
            let velocity = glam::Vec3::new(v.x, v.y, v.z);
            if velocity.length_squared() > 0.0025 {
                collector.0.push(crate::renderer::DebugLine {
                    start,
                    end: start + velocity * 0.18,
                    color: if body.is_sleeping() {
                        [0.45, 0.5, 0.55, 0.9]
                    } else {
                        [0.2, 0.9, 1.0, 0.95]
                    },
                });
            }
        }
    }
    resources
        .expect::<crate::renderer::DebugLineRenderer>()
        .render(
            &renderer.device,
            &renderer.queue,
            encoder,
            target,
            &renderer.depth_view,
            camera.view_projection_matrix(),
            &collector.0,
        );
}

fn select_light_data(
    lights: &[Light],
    camera_position: Vec3,
    sun_to: Vec3,
    daylight: f32,
) -> Vec<LightData> {
    let mut selected = Vec::with_capacity(MAX_LIGHTS);
    selected.extend(
        lights
            .iter()
            .filter(|light| light.light_type == LightType::Directional)
            .take(MAX_LIGHTS),
    );

    if selected.len() < MAX_LIGHTS {
        let mut local_lights: Vec<_> = lights
            .iter()
            .enumerate()
            .filter(|(_, light)| light.light_type != LightType::Directional)
            .collect();
        local_lights.sort_by(|(left_index, left), (right_index, right)| {
            let left_distance = finite_distance_squared(left.position, camera_position);
            let right_distance = finite_distance_squared(right.position, camera_position);
            left_distance
                .total_cmp(&right_distance)
                .then_with(|| left_index.cmp(right_index))
        });
        selected.extend(
            local_lights
                .into_iter()
                .map(|(_, light)| light)
                .take(MAX_LIGHTS - selected.len()),
        );
    }

    selected
        .into_iter()
        .map(|light| light_to_gpu(light, sun_to, daylight))
        .collect()
}

fn finite_distance_squared(position: Vec3, camera_position: Vec3) -> f32 {
    let distance = position.distance_squared(camera_position);
    if distance.is_finite() {
        distance
    } else {
        f32::INFINITY
    }
}

fn light_to_gpu(light: &Light, sun_to: Vec3, daylight: f32) -> LightData {
    let light_type = match light.light_type {
        LightType::Directional => 0,
        LightType::Point => 1,
        LightType::Spot => 2,
    };
    LightData {
        position: light.position.into(),
        light_type,
        color: light.color,
        intensity: if light.light_type == LightType::Directional {
            light.intensity * daylight
        } else {
            light.intensity
        },
        direction: if light.light_type == LightType::Directional {
            (-sun_to).into()
        } else {
            Vec3::ZERO.into()
        },
        range: light.range,
    }
}

fn compute_light_space_matrix(
    light_dir: Vec3,
    camera_position: Vec3,
    camera_forward: Vec3,
) -> Mat4 {
    const SHADOW_HALF_EXTENT: f32 = 45.0;
    const SHADOW_MAP_RESOLUTION: f32 = 2048.0;
    const SHADOW_FOCUS_AHEAD: f32 = 16.0;

    let dir = if light_dir.length_squared() > 1e-6 && light_dir.is_finite() {
        light_dir.normalize()
    } else {
        Vec3::new(-0.4, -1.0, -0.2).normalize()
    };
    let horizontal_forward = Vec3::new(camera_forward.x, 0.0, camera_forward.z).normalize_or_zero();
    let target = Vec3::new(camera_position.x, 0.0, camera_position.z)
        + horizontal_forward * SHADOW_FOCUS_AHEAD;
    let up = if dir.dot(Vec3::Y).abs() > 0.98 {
        Vec3::Z
    } else {
        Vec3::Y
    };

    // Snap the light-space center to shadow texels. This prevents the entire
    // shadow map from shimmering when the camera moves by a few millimeters.
    let orientation = Mat4::look_to_rh(Vec3::ZERO, dir, up);
    let mut target_light_space = orientation.transform_point3(target);
    let texel_size = SHADOW_HALF_EXTENT * 2.0 / SHADOW_MAP_RESOLUTION;
    target_light_space.x = (target_light_space.x / texel_size).round() * texel_size;
    target_light_space.y = (target_light_space.y / texel_size).round() * texel_size;
    let snapped_target = orientation.inverse().transform_point3(target_light_space);

    let eye = snapped_target - dir * 120.0;
    let view = Mat4::look_to_rh(eye, dir, up);
    let ortho = Mat4::orthographic_rh(
        -SHADOW_HALF_EXTENT,
        SHADOW_HALF_EXTENT,
        -SHADOW_HALF_EXTENT,
        SHADOW_HALF_EXTENT,
        -180.0,
        180.0,
    );
    ortho * view
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_selection_keeps_directional_and_nearest_locals() {
        let mut lights = vec![Light::directional(Vec3::new(1.0, -1.0, 0.2), [1.0; 3], 2.0)];
        for distance in (1..=12).rev() {
            lights.push(Light::point(
                Vec3::new(distance as f32, 0.0, 0.0),
                [1.0; 3],
                1.0,
                10.0,
            ));
        }

        let selected = select_light_data(&lights, Vec3::ZERO, Vec3::Y, 0.5);
        assert_eq!(selected.len(), MAX_LIGHTS);
        assert_eq!(selected[0].light_type, 0);
        assert!((selected[0].intensity - 1.0).abs() < 1e-6);
        let local_x: Vec<_> = selected[1..]
            .iter()
            .map(|light| light.position[0] as i32)
            .collect();
        assert_eq!(local_x, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn shadow_matrix_follows_distant_camera_and_stays_finite() {
        let matrix = compute_light_space_matrix(
            Vec3::new(-0.6, -1.0, 0.3),
            Vec3::new(240.0, 4.0, -190.0),
            Vec3::new(0.0, 0.0, -1.0),
        );
        assert!(matrix.to_cols_array().into_iter().all(f32::is_finite));
        let camera_ground = matrix.transform_point3(Vec3::new(240.0, 0.0, -190.0));
        assert!(camera_ground.x.abs() <= 1.0);
        assert!(camera_ground.y.abs() <= 1.0);
    }

    #[test]
    fn shadow_texel_snapping_ignores_sub_texel_camera_jitter() {
        let direction = Vec3::new(-0.6, -1.0, 0.3);
        let forward = Vec3::new(0.0, 0.0, -1.0);
        let first = compute_light_space_matrix(direction, Vec3::new(10.0, 2.0, 20.0), forward);
        let second = compute_light_space_matrix(direction, Vec3::new(10.001, 2.0, 20.001), forward);
        let largest_delta = first
            .to_cols_array()
            .into_iter()
            .zip(second.to_cols_array())
            .map(|(left, right)| (left - right).abs())
            .fold(0.0, f32::max);
        assert!(largest_delta < 1e-5, "matrix drifted by {largest_delta}");
    }
}
