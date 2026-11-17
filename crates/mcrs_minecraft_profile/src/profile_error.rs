use core::error::Error;
use core::fmt;

use crate::MAX_PROPERTIES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileError<'a> {
    StringTooLong {
        text: &'a str,
        length: usize,
        max: usize,
    },
    DisallowedNameCharacters(&'a str),
    TooManyProperties(usize),
}

impl fmt::Display for ProfileError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringTooLong { text, length, max } => write!(
                f,
                "String \"{text}\" is too long: {length}, expected range [0-{max}]"
            ),
            Self::DisallowedNameCharacters(name) => {
                write!(f, "Player name contained disallowed characters: '{name}'")
            }
            Self::TooManyProperties(count) => write!(
                f,
                "List is too long: {count}, expected range [0-{MAX_PROPERTIES}]"
            ),
        }
    }
}

impl Error for ProfileError<'_> {}
