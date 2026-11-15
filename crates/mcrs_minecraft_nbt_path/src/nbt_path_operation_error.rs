use core::fmt;

/// A get-or-create or a set of an NBT path that fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbtPathOperationError {
    /// A node before the last one gives no tag.
    NoTag,
    /// The number of nodes plus the depth of the value is 512 or more.
    TooDeep,
}

impl fmt::Display for NbtPathOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NbtPathOperationError::NoTag => "a path node finds no tag",
            NbtPathOperationError::TooDeep => "the value is too deep for the path",
        })
    }
}

impl core::error::Error for NbtPathOperationError {}
