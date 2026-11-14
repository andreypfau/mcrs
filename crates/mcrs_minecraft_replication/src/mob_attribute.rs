use mcrs_minecraft_entity::AttributeInstance;
use mcrs_minecraft_entity::attribute_keys::Attribute;

/// The instance of a syncable attribute of a mob after a change in a tick.
#[derive(Clone, Debug, PartialEq)]
pub struct MobAttribute {
    pub attribute: Attribute,
    /// `None` when the instance is that of a new mob of its type: the base of the type and no
    /// modifier.
    pub instance: Option<AttributeInstance>,
}
