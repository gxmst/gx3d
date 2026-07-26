use crate::core::{EngineWorld, Name, Resources, Time, Transform};
use crate::game::{Explosive, InteractionFocus, ToggleDoor};
use crate::physics::{PhysicsBody, PhysicsWorld};
use winit::keyboard::KeyCode;

pub fn system(world: &mut EngineWorld, resources: &Resources) {
    if super::menu_open(resources) {
        return;
    }
    // The buy menu captures digit/interaction input; spectators cannot act.
    let buy_open = resources
        .get::<super::buy_menu::BuyState>()
        .map(|buy| buy.open)
        .unwrap_or(false);
    let spectating = resources
        .get::<super::match_mode::MatchState>()
        .map(|state| state.player_spectating)
        .unwrap_or(false);
    if buy_open || spectating {
        return;
    }
    let hit = super::crosshair_raycast(resources, 10.0);
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
            if world.ecs.get::<&Explosive>(entity).is_ok() {
                next_focus.prompt = "E 抓取  ·  危险：射击可引爆并推动附近物体".to_string();
            } else if let Ok(mut door) = world.ecs.get::<&mut ToggleDoor>(entity) {
                next_focus.prompt = if door.open { "E 关闭" } else { "E 打开" }.to_string();
                if interact_pressed {
                    door.open = !door.open;
                }
            } else if let Ok(body) = world.ecs.get::<&PhysicsBody>(entity) {
                let physics = resources.expect::<PhysicsWorld>();
                if physics.is_sandbox_manipulable(&body) {
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
}

pub fn fixed_update(world: &mut EngineWorld, resources: &Resources) {
    let dt = resources.expect::<Time>().fixed_timestep;
    let mut updates = Vec::new();
    let physics = resources.expect::<PhysicsWorld>();
    for (transform, door, body) in world
        .ecs
        .query::<(&mut Transform, &mut ToggleDoor, Option<&PhysicsBody>)>()
        .iter()
    {
        let target = if door.open { 1.0 } else { 0.0 };
        let next_progress =
            door.progress + (target - door.progress) * (dt * door.speed).clamp(0.0, 1.0);
        let (next_position, next_rotation) = door.pose_at(next_progress);
        let blocked = (next_progress - door.progress).abs() > f32::EPSILON
            && body.is_some_and(|body| {
                physics.movable_body_blocks_pose(
                    body.rigid_body_handle,
                    next_position,
                    next_rotation,
                )
            });
        if blocked {
            // Opening pauses until the sweep area is clear. Closing reverses
            // automatically instead of trapping the player or forcing a
            // dynamic prop through the frame.
            if next_progress < door.progress {
                door.open = true;
            }
            continue;
        }

        door.progress = next_progress;
        transform.position = next_position;
        transform.rotation = next_rotation;
        if let Some(body) = body {
            updates.push((
                body.rigid_body_handle,
                transform.position,
                transform.rotation,
            ));
        }
    }
    drop(physics);
    if !updates.is_empty() {
        let mut physics = resources.expect_mut::<PhysicsWorld>();
        for (body, position, rotation) in updates {
            if let Some(rigid_body) = physics.rigid_body_set.get_mut(body) {
                if rigid_body.is_kinematic() {
                    rigid_body.set_next_kinematic_translation(position);
                    rigid_body.set_next_kinematic_rotation(rotation);
                } else {
                    rigid_body.set_translation(position, true);
                    rigid_body.set_rotation(rotation, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixed_update;
    use crate::core::{EngineWorld, Resources, Time, Transform};
    use crate::game::ToggleDoor;
    use crate::physics::{PhysicsBody, PhysicsShape, PhysicsWorld};
    use glam::{Quat, Vec3};

    #[test]
    fn closing_door_reopens_when_a_movable_body_blocks_it() {
        let mut world = EngineWorld::new();
        let resources = Resources::new();
        let mut physics = PhysicsWorld::new(Vec3::ZERO);
        let door = ToggleDoor {
            closed_position: Vec3::ZERO,
            closed_rotation: Quat::IDENTITY,
            open_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            hinge_offset: -Vec3::X,
            open: false,
            progress: 1.0,
            speed: 60.0,
        };
        let (open_position, open_rotation) = door.pose_at(1.0);
        let (door_body, door_collider) = physics.add_kinematic_body(
            open_position,
            PhysicsShape::Cuboid {
                half_extents: Vec3::new(1.0, 1.0, 0.1),
            }
            .to_rapier_collider(),
        );
        physics.set_body_rotation(door_body, open_rotation);
        physics.add_dynamic_body(
            Vec3::ZERO,
            PhysicsShape::Sphere { radius: 0.4 }.to_rapier_collider(),
            1.0,
        );
        physics.step();

        let entity = world.spawn();
        world.add_component(
            entity,
            Transform::new(open_position, open_rotation, Vec3::ONE),
        );
        world.add_component(entity, door);
        world.add_component(entity, PhysicsBody::new(door_body, door_collider, true));
        resources.insert(Time::new());
        resources.insert(physics);

        fixed_update(&mut world, &resources);

        let door = world.get_component::<ToggleDoor>(entity).unwrap();
        assert!(door.open);
        assert_eq!(door.progress, 1.0);
    }

    #[test]
    fn opening_door_pauses_when_a_kinematic_actor_blocks_it() {
        let mut world = EngineWorld::new();
        let resources = Resources::new();
        let mut physics = PhysicsWorld::new(Vec3::ZERO);
        let door = ToggleDoor {
            closed_position: Vec3::ZERO,
            closed_rotation: Quat::IDENTITY,
            open_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            hinge_offset: -Vec3::X,
            open: true,
            progress: 0.0,
            speed: 60.0,
        };
        let (open_position, _) = door.pose_at(1.0);
        let (door_body, door_collider) = physics.add_kinematic_body(
            Vec3::ZERO,
            PhysicsShape::Cuboid {
                half_extents: Vec3::new(1.0, 1.0, 0.1),
            }
            .to_rapier_collider(),
        );
        physics.add_kinematic_body(
            open_position,
            PhysicsShape::Capsule {
                radius: 0.35,
                half_height: 0.65,
            }
            .to_rapier_collider(),
        );
        physics.step();

        let entity = world.spawn();
        world.add_component(
            entity,
            Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE),
        );
        world.add_component(entity, door);
        world.add_component(entity, PhysicsBody::new(door_body, door_collider, true));
        resources.insert(Time::new());
        resources.insert(physics);

        fixed_update(&mut world, &resources);

        let door = world.get_component::<ToggleDoor>(entity).unwrap();
        assert!(door.open);
        assert_eq!(door.progress, 0.0);
    }
}
