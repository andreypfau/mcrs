use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_entity_key::EntityId;

/// A pickup of `count` items of the item entity with the network id `item`: all of its items
/// (IEN-PCK-004), or the part that found room (IEN-PCK-005).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickedUp {
    pub item: EntityId,
    pub collector: Entity,
    pub count: u8,
}
