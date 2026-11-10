use core::error::Error;
use core::fmt;

/// A custom component type with the key of a type that the registry already has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DuplicateKey;

impl fmt::Display for DuplicateKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the registry already has a component type with this key")
    }
}

impl Error for DuplicateKey {}
