use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerHolders {
    pub cursor: Entity,
    pub craft_grid: Entity,
    pub craft_result: Entity,
}
