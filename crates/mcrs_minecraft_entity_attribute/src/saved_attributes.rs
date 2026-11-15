use alloc::vec::Vec;

use serde::{Deserialize, Deserializer, Serialize};

use crate::attribute_bases::AttributeBases;
use crate::attribute_instances::AttributeInstances;
use crate::attribute_set::AttributeSet;
use crate::instance::Instance;
use crate::saved_attribute::SavedAttribute;
use crate::saved_modifier::SavedModifier;
use crate::tolerant::Lenient;

/// The list `attributes` of the save of a living entity (`attributes.md`, section 8). A load
/// leaves out each bad compound and each bad modifier and keeps the others, and reads a value
/// that is not a list as an empty list (ATR-SAV-002). Only a key that `persistence.md` does not
/// list, in a compound or in a modifier, fails the load with an error that names the key.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SavedAttributes {
    /// The compounds in the order of the list.
    pub attributes: Vec<SavedAttribute>,
}

impl SavedAttributes {
    /// A compound for each instance whose base value differs from the base of the type or that
    /// has a permanent modifier, in the order of the numeric ids; no transient modifier
    /// (ATR-SAV-001).
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

    /// The instances of an entity of the type of `bases` that loads this list (ATR-SAV-002): a
    /// compound sets the base value and then each modifier as a permanent one, and a compound
    /// whose attribute is not in the set has no effect.
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
