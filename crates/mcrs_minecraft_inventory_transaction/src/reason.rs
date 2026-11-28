use core::error::Error;
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    TooManyOps,
    NoSuchSlot,
    SameSlot,
    BadCount,
    EmptySource,
    NotEnough,
    TakeDenied,
    NotEqual,
    PlaceDenied,
    NoRoom,
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooManyOps => "too many operations",
            Self::NoSuchSlot => "no such slot",
            Self::SameSlot => "the two slots are the same slot",
            Self::BadCount => "the count is zero",
            Self::EmptySource => "the source slot is empty",
            Self::NotEnough => "the source slot holds fewer items",
            Self::TakeDenied => "the slot does not give its items",
            Self::NotEqual => "the stacks are not equal",
            Self::PlaceDenied => "the slot does not accept the stack",
            Self::NoRoom => "the slot has no room",
        })
    }
}

impl Error for Reason {}
