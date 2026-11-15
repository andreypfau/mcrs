use core::mem;

use crate::attribute_bases::AttributeBases;
use crate::attribute_keys::Attribute;
use crate::attribute_mask::AttributeMask;
use crate::attribute_set::AttributeSet;
use crate::instance_change::InstanceChange;
use crate::instance_edit::InstanceEdit;

/// The attribute instances of one entity with the bases of its type (`attributes.md`, section 3),
/// and the attributes that an edit changed since the last [`take_dirty`](Self::take_dirty).
/// Equality compares the bases and the instances only.
#[derive(Clone, Debug)]
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

    /// The attributes changed since the last call, which this call clears.
    pub fn take_dirty(&mut self) -> AttributeMask {
        mem::take(&mut self.dirty)
    }
}

impl PartialEq for AttributeInstances {
    fn eq(&self, other: &Self) -> bool {
        self.bases == other.bases && self.set == other.set
    }
}
