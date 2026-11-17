use bevy_ecs::entity::Entity;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotRef {
    pub holder: Entity,
    pub index: u16,
}
