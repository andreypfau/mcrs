use core::mem;

use crate::attribute_bases::AttributeBases;
use crate::attribute_keys::Attribute;
use crate::attribute_mask::AttributeMask;
use crate::attribute_set::AttributeSet;
#[cfg(feature = "wire")]
use crate::attribute_snapshot::AttributeSnapshot;
use crate::instance_change::InstanceChange;
use crate::instance_edit::InstanceEdit;
#[cfg(feature = "wire")]
use crate::instance_modifier::InstanceModifier;
#[cfg(feature = "wire")]
use crate::modifier_kind::ModifierKind;
#[cfg(feature = "wire")]
use crate::unknown_attribute::UnknownAttribute;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct AttributeInstances {
    bases: AttributeBases,
    set: AttributeSet,
    dirty: AttributeMask,
}

impl AttributeInstances {
    #[must_use]
    pub const fn new(bases: AttributeBases) -> Self {
        Self::with_set(bases, AttributeSet::new())
    }

    #[must_use]
    pub const fn with_set(bases: AttributeBases, set: AttributeSet) -> Self {
        Self {
            bases,
            set,
            dirty: AttributeMask::EMPTY,
        }
    }

    #[must_use]
    pub fn bases(&self) -> AttributeBases {
        self.bases
    }

    #[must_use]
    pub fn set(&self) -> &AttributeSet {
        &self.set
    }

    #[must_use]
    pub fn value(&self, attribute: Attribute) -> Option<f64> {
        self.set.value(self.bases, attribute)
    }

    #[must_use]
    pub fn base(&self, attribute: Attribute) -> Option<f64> {
        self.set.base(self.bases, attribute)
    }

    pub fn apply(&mut self, attribute: Attribute, edit: &InstanceEdit) -> InstanceChange {
        let change = edit.apply(&mut self.set, self.bases, attribute);
        if change == InstanceChange::Changed {
            self.dirty = self.dirty.with(attribute);
        }
        change
    }

    pub fn clear_modifiers(&mut self) -> InstanceChange {
        let mut change = InstanceChange::Unchanged;
        for attribute in self.bases.attributes() {
            if self.set.clear_modifiers(self.bases, attribute) == Ok(InstanceChange::Changed) {
                self.dirty = self.dirty.with(attribute);
                change = InstanceChange::Changed;
            }
        }
        change
    }

    pub fn take_dirty(&mut self) -> AttributeMask {
        mem::take(&mut self.dirty)
    }
}

#[cfg(feature = "wire")]
impl AttributeInstances {
    #[must_use]
    pub fn snapshot(&self, attribute: Attribute) -> Option<AttributeSnapshot<'_>> {
        AttributeSnapshot::from_instance(self.bases, attribute, self.set.instance(attribute))
    }

    pub fn receive(
        &mut self,
        snapshot: &AttributeSnapshot<'_>,
    ) -> Result<InstanceChange, UnknownAttribute> {
        let attribute = snapshot
            .attribute
            .resolve()
            .ok_or(UnknownAttribute(snapshot.attribute))?;
        let Some(type_base) = self.bases.base(attribute) else {
            return Ok(InstanceChange::Unchanged);
        };
        let instance = self.set.instance(attribute);
        let (base, modifiers) = instance.map_or((type_base, &[][..]), |instance| {
            (instance.base, instance.modifiers.as_slice())
        });
        if holds(base, modifiers, snapshot) {
            return Ok(InstanceChange::Unchanged);
        }
        let _in_set = self.set.set_base(self.bases, attribute, snapshot.base);
        let _in_set = self.set.clear_modifiers(self.bases, attribute);
        for modifier in &snapshot.modifiers {
            let _ignored_duplicate =
                self.set
                    .add_modifier(self.bases, attribute, InstanceModifier::transient(modifier));
        }
        self.dirty = self.dirty.with(attribute);
        Ok(InstanceChange::Changed)
    }
}

#[cfg(feature = "wire")]
fn holds(base: f64, modifiers: &[InstanceModifier], snapshot: &AttributeSnapshot<'_>) -> bool {
    if base.to_bits() != snapshot.base.to_bits() {
        return false;
    }
    let mut held = modifiers.iter();
    for (index, modifier) in snapshot.modifiers.iter().enumerate() {
        let earlier = snapshot.modifiers.get(..index).unwrap_or_default();
        if earlier.iter().any(|first| first.id == modifier.id) {
            continue;
        }
        let Some(present) = held.next() else {
            return false;
        };
        let same = present.id() == modifier.id
            && present.amount().to_bits() == modifier.amount.to_bits()
            && present.operation() == modifier.operation
            && present.kind() == ModifierKind::Transient;
        if !same {
            return false;
        }
    }
    held.next().is_none()
}

impl PartialEq for AttributeInstances {
    fn eq(&self, other: &Self) -> bool {
        self.bases == other.bases && self.set == other.set
    }
}
