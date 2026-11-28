use bevy_ecs::component::Component;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_replication::SpawnLook;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct Appearance {
    pub kind: &'static EntityTypeData,
    pub look: SpawnLook,
}
