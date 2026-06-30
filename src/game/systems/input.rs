use crate::core::{EngineWorld, Resources};

pub fn system(_world: &mut EngineWorld, resources: &Resources) {
    if let Some(mut input) = resources.get_mut::<crate::input::InputState>() {
        input.update();
    }
}
