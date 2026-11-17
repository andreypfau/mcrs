use alloc::vec::Vec;

use crate::attribute_keys::Attribute;
use crate::instance_modifier::InstanceModifier;

#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub(crate) attribute: Attribute,
    pub(crate) base: f64,
    pub(crate) modifiers: Vec<InstanceModifier>,
}

impl Instance {
    #[must_use]
    pub fn attribute(&self) -> Attribute {
        self.attribute
    }

    #[must_use]
    pub fn base(&self) -> f64 {
        self.base
    }

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

    pub(crate) fn is_new_of_type(&self, type_base: f64) -> bool {
        self.base.to_bits() == type_base.to_bits() && self.modifiers.is_empty()
    }
}
