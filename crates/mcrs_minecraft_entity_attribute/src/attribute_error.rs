use core::fmt;

/// A change of an attribute instance that the rules refuse (`attributes.md`, section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AttributeError {
    /// The attribute is not in the attribute set of the entity type (ATR-BASE-001).
    NotInSet,
    /// The instance already has a modifier with this identifier (ATR-INS-004, add).
    ModifierPresent,
}

impl fmt::Display for AttributeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotInSet => "the attribute is not in the attribute set of the entity type",
            Self::ModifierPresent => "the instance already has a modifier with this identifier",
        })
    }
}

impl core::error::Error for AttributeError {}
