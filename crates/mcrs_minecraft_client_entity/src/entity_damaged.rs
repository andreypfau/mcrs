use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_entity_key::Cause;

#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct EntityDamaged {
    pub entity: Entity,
    pub damage_type: u32,
    pub cause: Cause,
    pub source_position: Option<DVec3>,
}
