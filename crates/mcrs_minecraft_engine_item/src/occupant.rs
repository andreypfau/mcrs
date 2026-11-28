use bevy_ecs::entity::Entity;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Occupant {
    pub index: u16,
    pub stack: Entity,
}
