use crate::attribute_keys::Attribute;
use crate::instance_modifier::InstanceModifier;

/// The instance of one attribute on one entity: a base value and a sequence of modifiers
/// (`attributes.md`, ATR-INS-001).
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeInstance {
    pub(crate) attribute: Attribute,
    pub(crate) base: f64,
    pub(crate) modifiers: Vec<InstanceModifier>,
}

impl AttributeInstance {
    #[must_use]
    pub fn attribute(&self) -> Attribute {
        self.attribute
    }

    /// The base value as it was set, outside the range when it was set so.
    #[must_use]
    pub fn base(&self) -> f64 {
        self.base
    }

    /// The modifiers in the order of the sequence (ATR-INS-004).
    #[must_use]
    pub fn modifiers(&self) -> &[InstanceModifier] {
        &self.modifiers
    }

    pub(crate) fn new_of_type(attribute: Attribute, type_base: f64) -> Self {
        Self {
            attribute,
            base: type_base,
            modifiers: Vec::new(),
        }
    }

    /// The instance is the one of a new entity of the type (ATR-BASE-003).
    pub(crate) fn is_new_of_type(&self, type_base: f64) -> bool {
        self.base.to_bits() == type_base.to_bits() && self.modifiers.is_empty()
    }
}
