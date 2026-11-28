use core::error::Error;
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClickError {
    NoSuchEntry,
    NoSuchSlot,
    TooManyOps,
}

impl fmt::Display for ClickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NoSuchEntry => "the menu has no such entry",
            Self::NoSuchSlot => "the player has no such slot",
            Self::TooManyOps => "the click gives too many operations",
        })
    }
}

impl Error for ClickError {}
