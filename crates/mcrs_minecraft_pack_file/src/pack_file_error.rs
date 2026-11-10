use core::fmt;

use crate::json::SyntaxError;

/// A file of the entity pack that the loader cannot read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackFileError {
    /// The text is not JSON.
    Syntax(SyntaxError),
    /// The root has no object with this name.
    NoRoot(&'static str),
}

impl fmt::Display for PackFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(error) => error.fmt(formatter),
            Self::NoRoot(name) => write!(formatter, "the root has no object `{name}`"),
        }
    }
}

impl core::error::Error for PackFileError {}
