use bevy_ecs::component::Component;
use mcrs_minecraft_entity_key::EntityId;

use crate::grid_slot::GridSlot;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[require(GridSlot)]
pub struct MotionId(pub EntityId);
