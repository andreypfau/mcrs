use bevy_ecs::message::Message;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

use crate::attribute_target::AttributeTarget;

/// Asks to change the instance of `attribute` of `target`. The edits apply in arrival order in
/// [`AttributeEdits`](crate::AttributeEdits). An edit of an attribute outside the attribute set
/// of the target, or of a target that the world does not hold, does nothing (ATR-SRC-001).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct AttributeEdit {
    pub target: AttributeTarget,
    pub attribute: Attribute,
    pub edit: InstanceEdit,
}
