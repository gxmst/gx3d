use hecs::World;

pub struct EngineWorld {
    pub ecs: World,
}

impl EngineWorld {
    pub fn new() -> Self {
        Self { ecs: World::new() }
    }

    pub fn spawn(&mut self) -> hecs::Entity {
        self.ecs.spawn(())
    }

    pub fn despawn(&mut self, entity: hecs::Entity) -> bool {
        self.ecs.despawn(entity).is_ok()
    }

    pub fn add_component<C: hecs::Component>(&mut self, entity: hecs::Entity, component: C) {
        if let Err(e) = self.ecs.insert_one(entity, component) {
            log::warn!("Failed to add component to entity {:?}: {}", entity, e);
        }
    }

    pub fn remove_component<C: hecs::Component>(
        &mut self,
        entity: hecs::Entity,
    ) -> Result<C, hecs::ComponentError> {
        self.ecs.remove_one::<C>(entity)
    }

    pub fn get_component<C: hecs::Component + Clone>(&self, entity: hecs::Entity) -> Option<C> {
        self.ecs.get::<&C>(entity).ok().map(|c| (*c).clone())
    }

    pub fn has_component<C: hecs::Component>(&self, entity: hecs::Entity) -> bool {
        self.ecs.get::<&C>(entity).is_ok()
    }

    pub fn query<Q: hecs::Query>(&self) -> hecs::QueryBorrow<'_, Q> {
        self.ecs.query::<Q>()
    }

    pub fn query_mut<Q: hecs::Query>(&mut self) -> hecs::QueryBorrow<'_, Q> {
        self.ecs.query::<Q>()
    }
}

impl Default for EngineWorld {
    fn default() -> Self {
        Self::new()
    }
}
