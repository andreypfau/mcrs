use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_entity_key::EntityId;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickedUp {
    pub item: EntityId,
    pub collector: Entity,
    pub count: u8,
}
