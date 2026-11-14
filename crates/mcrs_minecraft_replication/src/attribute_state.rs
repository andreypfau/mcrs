use mcrs_minecraft_entity::AttributeInstance;
use mcrs_minecraft_entity::attribute_keys::Attribute;

/// The instance of a syncable attribute of an entity, after a change in a tick or at its spawn.
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeState {
    pub attribute: Attribute,
    /// `None` when the instance is that of a new entity of its type: the base of the type and no
    /// modifier.
    pub instance: Option<AttributeInstance>,
}
