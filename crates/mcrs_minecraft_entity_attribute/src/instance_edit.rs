use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::attribute_bases::AttributeBases;
use crate::attribute_keys::Attribute;
use crate::attribute_set::AttributeSet;
use crate::instance_change::InstanceChange;
use crate::instance_modifier::InstanceModifier;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum InstanceEdit {
    SetBase(f64),
    Put(InstanceModifier),
    Remove(ResourceLocationBuf),
}

impl InstanceEdit {
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
