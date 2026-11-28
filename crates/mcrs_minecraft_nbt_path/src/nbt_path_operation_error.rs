use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbtPathOperationError {
    NoTag,
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
