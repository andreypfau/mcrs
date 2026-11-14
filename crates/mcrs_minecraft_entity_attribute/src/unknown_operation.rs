use core::fmt;

/// A wire id or a name that is not one of a [`ModifierOperation`](crate::ModifierOperation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownOperation;

impl fmt::Display for UnknownOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown attribute modifier operation")
    }
}

impl core::error::Error for UnknownOperation {}
