use mcrs_minecraft_entity::{AttributeBases, AttributeSet, InstanceChange, InstanceModifier};
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_resource::ResourceLocationBuf;

/// A change of one attribute instance (ATR-INS-002 and ATR-INS-004).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum InstanceEdit {
    /// Stores the base value as given.
    SetBase(f64),
    /// The call "set": leaves an equal modifier in place and puts any other at the end of the
    /// sequence in place of the modifier of its identifier.
    Put(InstanceModifier),
    /// Removes the modifier of this identifier.
    Remove(ResourceLocationBuf),
}

impl InstanceEdit {
    /// Applies the edit to the instance of `attribute` in `set`, over the bases of its type. An
    /// edit of an attribute outside the set of the type leaves the instances as they are
    /// (ATR-SRC-001).
    pub fn apply(
        &self,
        set: &mut AttributeSet,
        bases: AttributeBases,
        attribute: Attribute,
    ) -> InstanceChange {
        let result = match self {
            Self::SetBase(base) => set.set_base(bases, attribute, *base),
            Self::Put(modifier) => set.set_modifier(bases, attribute, modifier.clone()),
            Self::Remove(id) => set.remove_modifier(bases, attribute, id.as_location()),
        };
        result.unwrap_or(InstanceChange::Unchanged)
    }
}
