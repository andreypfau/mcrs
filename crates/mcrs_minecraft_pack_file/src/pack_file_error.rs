use core::fmt;

use crate::json::SyntaxError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackFileError {
    Syntax(SyntaxError),
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
