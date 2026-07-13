use crate::asset::{AssetManager, Handle, Mesh};
use crate::core::{EngineWorld, Resources, Time};
use crate::game::WeaponModel;
use crate::input::InputState;
use crate::renderer::bind_groups::LightData;
use crate::renderer::{Camera, LightType, RenderMesh, Renderer};
use glam::{Mat4, Vec2};
use winit::keyboard::KeyCode;

use super::{MenuState, MuzzleFlashTimer, SceneLights, WeaponFeedbackAssets};

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

    // Convert scene lights to GPU format.
    let light_data: Vec<LightData> = scene_lights
        .0
        .iter()
        .map(|l| {
            let lt = match l.light_type {
                LightType::Directional => 0u32,
                LightType::Point => 1,
                LightType::Spot => 2,
            };
            let dir = if l.light_type == LightType::Directional {
                -sun_to
            } else {
                glam::Vec3::ZERO
            };
            LightData {
                position: l.position.into(),
                light_type: lt,
                color: l.color,
                intensity: if l.light_type == LightType::Directional {
                    l.intensity * daylight
                } else {
                    l.intensity
                },
                direction: dir.into(),
                range: l.range,
            }
        })
        .collect();

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
        .map(|_| compute_light_space_matrix(-sun_to))
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
    if !menu_open {
        if let Some(focus) = resources.get::<crate::game::InteractionFocus>() {
            renderer.overlay.borrow_mut().draw_hud(
                &renderer.device,
                &renderer.queue,
                &mut encoder,
                &output_view,
                &focus,
                aiming,
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
                renderer.surface_config.width,
                renderer.surface_config.height,
            );
        }
    }

    renderer.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    let view_mode = resources
        .get::<super::ViewModeState>()
        .map(|state| state.mode)
        .unwrap_or(super::ViewMode::Fps);
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
        renderer
            .window
            .set_title("GxEngine | 设置菜单 | Esc 返回游戏");
    } else if show_help {
        renderer.window.set_title(
            "GxEngine Controls | V FPS/God | P Pause | . Step | [/] Speed | F3 Colliders | F4 Velocity | F5 Contacts | WASD Move | E Grab | T Push | X Freeze | Delete Remove | Esc Menu",
        );
    } else {
        renderer.window.set_title(&format!(
            "GxEngine | {} | {} | {:.1} FPS | Ammo: {} | Lights: {} | Pos: ({:.1}, {:.1}, {:.1}) | Hold F: Controls",
            mode_label,
            time_label,
            time.fps(),
            weapon.current_ammo,
            scene_lights.0.len(),
            camera.position.x,
            camera.position.y,
            camera.position.z,
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

fn compute_light_space_matrix(light_dir: glam::Vec3) -> Mat4 {
    let dir = light_dir.normalize();
    let eye = -dir * 25.0;
    let target = glam::Vec3::ZERO;
    let up = if dir.abs_diff_eq(glam::Vec3::Y, 1e-4) {
        glam::Vec3::Z
    } else {
        glam::Vec3::Y
    };
    let view = Mat4::look_at_rh(eye, target, up);
    let ortho = Mat4::orthographic_rh(-20.0, 20.0, -20.0, 20.0, -30.0, 30.0);
    ortho * view
}
