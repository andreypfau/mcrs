use alloc::vec::Vec;

use mcrs_minecraft_wire::{Decode, Encode};

use crate::attribute_bases::AttributeBases;
use crate::attribute_id::AttributeId;
use crate::attribute_keys::Attribute;
use crate::attribute_modifier::AttributeModifier;
use crate::instance::Instance;

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct AttributeSnapshot<'a> {
    pub attribute: AttributeId,
    pub base: f64,
    pub modifiers: Vec<AttributeModifier<'a>>,
}

impl<'a> AttributeSnapshot<'a> {
    #[must_use]
    pub fn from_instance(
        bases: AttributeBases,
        attribute: Attribute,
        instance: Option<&'a Instance>,
    ) -> Option<Self> {
        let Some(instance) = instance else {
            return Some(Self {
                attribute: attribute.into(),
                base: bases.base(attribute)?,
                modifiers: Vec::new(),
            });
        };
        Some(Self {
            attribute: attribute.into(),
            base: instance.base,
            modifiers: instance
                .modifiers
                .iter()
                .map(|modifier| AttributeModifier {
                    id: modifier.id(),
                    amount: modifier.amount(),
                    operation: modifier.operation(),
                })
                .collect(),
        })
    }
}
