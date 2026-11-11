use core::error::Error;
use core::fmt;

/// Why a custom item component type was not registered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegisterError {
    /// A registered type has the same key.
    DuplicateKey,
    /// The world has 128 rows of item components, the size of a mask.
    MaskFull,
    /// The type is not stored in tables.
    UnsupportedStorage,
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DuplicateKey => "an item component type with this key is registered",
            Self::MaskFull => "the world has the maximum number of item component types",
            Self::UnsupportedStorage => "an item component type must be stored in tables",
        })
    }
}

impl Error for RegisterError {}
