use bevy_ecs::entity::Entity;

use crate::SlotRef;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GateQuery {
    pub slot: SlotRef,
    pub actor: Entity,
    pub stack: Entity,
}
