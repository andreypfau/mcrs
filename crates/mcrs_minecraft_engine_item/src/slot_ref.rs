use bevy_ecs::entity::Entity;

/// One slot of a holder: the entity with [`Slots`](crate::Slots) and the index of the slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotRef {
    pub holder: Entity,
    pub index: u16,
}
