use bevy_ecs::entity::Entity;

use crate::SlotRef;

/// The inputs of a gate (ITX-LIM-006): the slot, the actor that made the request, and the stack
/// that leaves or enters the slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GateQuery {
    pub slot: SlotRef,
    pub actor: Entity,
    pub stack: Entity,
}
