use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

#[derive(Message, Clone, Debug, PartialEq)]
pub struct AttributeEdit {
    pub target: Entity,
    pub attribute: Attribute,
    pub edit: InstanceEdit,
}
