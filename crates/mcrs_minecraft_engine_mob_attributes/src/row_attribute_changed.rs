use bevy_ecs::message::Message;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

use crate::attribute_target::AttributeTarget;

#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct RowAttributeChanged {
    pub target: AttributeTarget,
    pub attribute: Attribute,
}
