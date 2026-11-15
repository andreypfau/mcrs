use core::error::Error;
use core::fmt;

use mcrs_minecraft_item_stack::SavedError;

/// The failure of a read or a write of the player file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileError {
    /// The player file is not a compound.
    NotACompound,
    /// The key has a tag of the wrong kind.
    BadShape(&'static str),
    /// A stack under the key does not load or does not save.
    Stack {
        key: &'static str,
        cause: SavedError,
    },
}

impl fmt::Display for FileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotACompound => formatter.write_str("the player file is not a compound"),
            Self::BadShape(key) => write!(formatter, "`{key}` has a tag of the wrong kind"),
            Self::Stack { key, cause } => write!(formatter, "a stack of `{key}`: {cause}"),
        }
    }
}

impl Error for FileError {}
