use alloc::borrow::Cow;

use crate::EntityTypeId;
use crate::EntityTypeTagId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EntityTypeSet {
    Tag(EntityTypeTagId),
    EntityTypes(Cow<'static, [EntityTypeId]>),
}

impl EntityTypeSet {
    pub(crate) fn borrow_static(&'static self) -> Self {
        match self {
            Self::Tag(tag) => Self::Tag(*tag),
            Self::EntityTypes(entity_types) => Self::EntityTypes(Cow::Borrowed(entity_types)),
        }
    }
}
