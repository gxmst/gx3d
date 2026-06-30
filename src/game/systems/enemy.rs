use crate::core::Transform;
use crate::core::{EngineWorld, Resources, Time};
use crate::game::EnemyAI;

pub fn system(world: &mut EngineWorld, resources: &Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds().min(0.05))
        .unwrap_or(0.0);

    for (transform, ai) in world.ecs.query::<(&mut Transform, &mut EnemyAI)>().iter() {
        ai.update(transform, dt);
    }
}
