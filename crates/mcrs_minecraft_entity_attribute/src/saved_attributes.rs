use alloc::vec::Vec;

use serde::{Deserialize, Deserializer, Serialize};

use crate::attribute_bases::AttributeBases;
use crate::attribute_instances::AttributeInstances;
use crate::attribute_set::AttributeSet;
use crate::instance::Instance;
use crate::saved_attribute::SavedAttribute;
use crate::saved_modifier::SavedModifier;
use crate::tolerant::Lenient;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SavedAttributes {
    pub attributes: Vec<SavedAttribute>,
}

impl SavedAttributes {
    #[must_use]
    pub fn from_instances(instances: &AttributeInstances) -> Self {
        let bases = instances.bases();
        let attributes = instances
            .set()
            .instances()
            .filter_map(|instance| saved(bases, instance))
            .collect();
        Self { attributes }
    }

    #[must_use]
    pub fn into_instances(self, bases: AttributeBases) -> AttributeInstances {
        let mut set = AttributeSet::new();
        for compound in self.attributes {
            if set.set_base(bases, compound.id, compound.base).is_err() {
                continue;
            }
            for modifier in compound.modifiers {
                let _in_set = set.set_modifier(bases, compound.id, modifier.into_modifier());
            }
        }
        AttributeInstances::with_set(bases, set)
    }
}

fn saved(bases: AttributeBases, instance: &Instance) -> Option<SavedAttribute> {
    let modifiers: Vec<SavedModifier> = instance
        .modifiers()
        .iter()
        .filter_map(SavedModifier::from_modifier)
        .collect();
    let type_base = bases.base(instance.attribute())?;
    if instance.base().to_bits() == type_base.to_bits() && modifiers.is_empty() {
        return None;
    }
    Some(SavedAttribute {
        id: instance.attribute(),
        base: instance.base(),
        modifiers,
    })
}

impl<'de> Deserialize<'de> for SavedAttributes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Lenient(attributes) = Lenient::<Vec<SavedAttribute>>::deserialize(deserializer)?;
        Ok(Self {
            attributes: attributes.unwrap_or_default(),
        })
    }
}
