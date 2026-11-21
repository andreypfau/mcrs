use bevy_ecs::message::Message;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

use crate::attribute_target::AttributeTarget;

#[derive(Message, Clone, Debug, PartialEq)]
pub struct RowAttributeEdit {
    pub target: AttributeTarget,
    pub attribute: Attribute,
    pub edit: InstanceEdit,
}
