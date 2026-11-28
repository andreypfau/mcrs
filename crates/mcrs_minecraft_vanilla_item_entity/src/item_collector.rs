use bevy_ecs::component::Component;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_physics::Aabb;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ItemCollector {
    pub id: EntityId,
    pub uuid: u128,
    pub bounds: Aabb,
    pub vehicle: Option<Aabb>,
    pub mode: Collect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collect {
    Survival,
    Creative,
}
