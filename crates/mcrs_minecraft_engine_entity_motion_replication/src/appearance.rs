use bevy_ecs::component::Component;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_replication::SpawnLook;

/// What the spawn event of a moving entity shows besides its motion. The kind plugin owns it; an
/// entity without it publishes no spawn.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Appearance {
    pub kind: &'static EntityTypeData,
    pub look: SpawnLook,
}
