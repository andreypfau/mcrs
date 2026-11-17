use core::error::Error;
use core::fmt;

use mcrs_minecraft_core::codec::StringTooLong;

use crate::MAX_PROPERTIES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileError<'a> {
    StringTooLong(StringTooLong<'a>),
    DisallowedNameCharacters(&'a str),
    TooManyProperties(usize),
    UngroupedProperties,
}

impl fmt::Display for ProfileError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringTooLong(error) => error.fmt(f),
            Self::DisallowedNameCharacters(name) => {
                write!(f, "Player name contained disallowed characters: '{name}'")
            }
            Self::TooManyProperties(count) => write!(
                f,
                "List is too long: {count}, expected range [0-{MAX_PROPERTIES}]"
            ),
            Self::UngroupedProperties => {
                f.write_str("Properties with one name must be next to each other")
            }
        }
    }
}

impl Error for ProfileError<'_> {}

impl<'a> From<StringTooLong<'a>> for ProfileError<'a> {
    fn from(error: StringTooLong<'a>) -> Self {
        Self::StringTooLong(error)
    }
}
