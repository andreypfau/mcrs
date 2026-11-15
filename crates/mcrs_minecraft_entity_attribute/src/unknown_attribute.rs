use core::fmt;

use crate::attribute_id::AttributeId;

/// A numeric id that is not in the registry `minecraft:attribute` (ATR-CLI-006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownAttribute(pub AttributeId);

impl fmt::Display for UnknownAttribute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown attribute id {}", self.0)
    }
}

impl core::error::Error for UnknownAttribute {}
