use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::Event;
use mcrs_engine::world::block::BlockPos;
use mcrs_protocol::BlockStateId;

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Explosion;

/// The radius of the [Explosion] to be created by detonating an [Explosive](crate::entity::explosive::Explosive).
#[derive(Component, Default, Debug)]
pub struct ExplosionRadius(pub f32);

#[derive(Event, Debug, Eq, PartialEq)]
pub struct BlockExplodedEvent {
    pub dimension: Entity,
    pub chunk: Entity,
    pub block_pos: BlockPos,
    pub block_state_id: BlockStateId,
    pub detonator: Option<Entity>,
}

impl std::hash::Hash for BlockExplodedEvent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.dimension.hash(state);
        self.block_pos.hash(state);
    }
}
