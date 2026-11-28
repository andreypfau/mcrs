use mcrs_minecraft_entity_attribute::Instance;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeState {
    pub attribute: Attribute,
    pub instance: Option<Instance>,
}
