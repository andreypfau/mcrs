use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct AttributeChanged {
    pub target: Entity,
    pub attribute: Attribute,
}
