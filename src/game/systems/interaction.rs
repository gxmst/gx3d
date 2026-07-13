use crate::core::{EngineWorld, Name, Resources, Time, Transform};
use crate::game::{InteractionFocus, ToggleDoor};
use crate::physics::{PhysicsBody, PhysicsWorld, Ray};
use crate::renderer::Camera;
use winit::keyboard::KeyCode;

pub fn system(world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<super::MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false)
    {
        return;
    }
    let (origin, direction) = {
        let camera = resources.expect::<Camera>();
        (camera.position, camera.forward())
    };
    let hit = {
        let physics = resources.expect::<PhysicsWorld>();
        let player_body = resources.expect::<super::PlayerBody>().0;
        physics.cast_ray_excluding_body(&Ray::new(origin, direction, 10.0), player_body)
    };
    let interact_pressed = resources
        .expect::<crate::input::InputState>()
        .is_key_just_pressed(KeyCode::KeyE);

    let dt = resources.expect::<Time>().real_delta_seconds().min(0.05);
    let mut next_focus = InteractionFocus::default();
    if let Some(hit) = hit {
        if let Some(entity) = hit.entity {
            next_focus.entity = Some(entity);
            next_focus.distance = hit.distance;
            next_focus.title = world
                .ecs
                .get::<&Name>(entity)
                .map(|name| name.0.to_string())
                .unwrap_or_else(|_| "场景物体".to_string());
            if let Ok(mut door) = world.ecs.get::<&mut ToggleDoor>(entity) {
                next_focus.prompt = if door.open { "E 关闭" } else { "E 打开" }.to_string();
                if interact_pressed {
                    door.open = !door.open;
                }
            } else if let Ok(body) = world.ecs.get::<&PhysicsBody>(entity) {
                if !body.is_static {
                    next_focus.prompt = "E 抓取  ·  T 推动  ·  X 冻结  ·  Delete 删除".to_string();
                }
            }
            if !next_focus.prompt.is_empty() {
                next_focus.hold_seconds = 0.18;
            }
        }
    }
    {
        let mut focus = resources.expect_mut::<InteractionFocus>();
        if next_focus.prompt.is_empty() && focus.hold_seconds > 0.0 {
            focus.hold_seconds = (focus.hold_seconds - dt).max(0.0);
        } else {
            *focus = next_focus;
        }
    }
    let mut updates = Vec::new();
    for (entity, transform, door, body) in world
        .ecs
        .query::<(
            hecs::Entity,
            &mut Transform,
            &mut ToggleDoor,
            Option<&PhysicsBody>,
        )>()
        .iter()
    {
        let target = if door.open { 1.0 } else { 0.0 };
        door.progress += (target - door.progress) * (dt * door.speed).clamp(0.0, 1.0);
        transform.rotation = door
            .closed_rotation
            .slerp(door.open_rotation, door.progress);
        if let Some(body) = body {
            updates.push((body.rigid_body_handle, transform.rotation));
        }
        let _ = entity;
    }
    if !updates.is_empty() {
        let mut physics = resources.expect_mut::<PhysicsWorld>();
        for (body, rotation) in &updates {
            physics.set_body_rotation(*body, *rotation);
        }
        let bodies: Vec<_> = updates.into_iter().map(|(body, _)| body).collect();
        physics.refresh_body_colliders(&bodies);
    }
}
