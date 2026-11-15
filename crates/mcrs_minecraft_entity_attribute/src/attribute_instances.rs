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

/// The attribute instances of one entity with the bases of its type (`attributes.md`, section 3),
/// and the attributes that an edit changed since the last [`take_dirty`](Self::take_dirty).
/// Equality compares the bases and the instances only.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct AttributeInstances {
    bases: AttributeBases,
    set: AttributeSet,
    dirty: AttributeMask,
}

impl AttributeInstances {
    /// The instances of a new entity of a type with the bases `bases`.
    #[must_use]
    pub const fn new(bases: AttributeBases) -> Self {
        Self::with_set(bases, AttributeSet::new())
    }

    /// The instances of `set` over `bases`, with no attribute changed: a consumer of
    /// [`take_dirty`](Self::take_dirty) treats a new or replaced value as wholly changed.
    #[must_use]
    pub const fn with_set(bases: AttributeBases, set: AttributeSet) -> Self {
        Self {
            bases,
            set,
            dirty: AttributeMask::EMPTY,
        }
    }

    /// The bases of the type of the entity.
    #[must_use]
    pub fn bases(&self) -> AttributeBases {
        self.bases
    }

    /// The instances that differ from those of a new entity of the type.
    #[must_use]
    pub fn set(&self) -> &AttributeSet {
        &self.set
    }

    /// The value of ATR-VAL-001, or `None` when the attribute is not in the set of the type.
    #[must_use]
    pub fn value(&self, attribute: Attribute) -> Option<f64> {
        self.set.value(self.bases, attribute)
    }

    /// The base value, or `None` when the attribute is not in the set of the type.
    #[must_use]
    pub fn base(&self, attribute: Attribute) -> Option<f64> {
        self.set.base(self.bases, attribute)
    }

    /// Applies `edit` to the instance of `attribute` and marks the attribute changed when the
    /// edit changed the instance.
    pub fn apply(&mut self, attribute: Attribute, edit: &InstanceEdit) -> InstanceChange {
        let change = edit.apply(&mut self.set, self.bases, attribute);
        if change == InstanceChange::Changed {
            self.dirty = self.dirty.with(attribute);
        }
        change
    }

    /// Removes every modifier and keeps every base, and marks each attribute that lost one.
    /// Returns [`InstanceChange::Changed`] when at least one modifier was removed.
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

    /// The attributes changed since the last call, which this call clears.
    pub fn take_dirty(&mut self) -> AttributeMask {
        mem::take(&mut self.dirty)
    }
}

#[cfg(feature = "wire")]
impl AttributeInstances {
    /// The entry of the instance of `attribute` (ATR-NET-002), or `None` when the attribute is
    /// not in the set of the type.
    #[must_use]
    pub fn snapshot(&self, attribute: Attribute) -> Option<AttributeSnapshot<'_>> {
        AttributeSnapshot::from_instance(self.bases, attribute, self.set.instance(attribute))
    }

    /// Takes the base and the modifiers of `snapshot` as transient modifiers (ATR-CLI-002) and
    /// marks the attribute changed when its instance changed. An attribute outside the set of
    /// the type and a second modifier of one identifier change nothing (ATR-CLI-003); an id
    /// outside the registry is an error and changes nothing (ATR-CLI-006).
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

/// The instance of `base` and `modifiers` equals the one that `snapshot` gives.
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
