use bevy_ecs::message::Message;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

use crate::attribute_target::AttributeTarget;

/// An [`AttributeEdit`](crate::AttributeEdit) changed the instance of `attribute` of `target`.
/// One edit writes at most one; an edit that leaves the instance as it is writes none.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct AttributeChanged {
    pub target: AttributeTarget,
    pub attribute: Attribute,
}
