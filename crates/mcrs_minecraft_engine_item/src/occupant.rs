use bevy_ecs::entity::Entity;

/// An occupied slot and its stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Occupant {
    pub index: u16,
    pub stack: Entity,
}
