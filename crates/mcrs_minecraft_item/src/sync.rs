use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

/// Top-level cells and dropped items whose subtree changed this tick.
#[derive(Resource, Default, Debug)]
pub struct DirtyStacks {
    pub cells: Vec<(Entity, u16)>,
    pub roots: Vec<Entity>,
}
