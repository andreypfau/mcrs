use core::error::Error;
use core::fmt;

/// Why a request does not apply. The variants are in the order in which an operation is examined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    /// The request has more operations than [`MAX_OPS`](crate::MAX_OPS).
    TooManyOps,
    /// A slot of the operation does not exist.
    NoSuchSlot,
    /// The two slots of the operation are the same slot.
    SameSlot,
    /// The count of the operation is 0.
    BadCount,
    /// The source slot is empty; for a swap, the two slots are empty.
    EmptySource,
    /// The count of the operation is larger than the count of the source slot.
    NotEnough,
    /// The take filter refuses for a slot that loses items.
    TakeDenied,
    /// The destination holds a stack that is not equal to the stack that moves.
    NotEqual,
    /// The place filter of a slot refuses the stack that enters.
    PlaceDenied,
    /// The destination cannot hold the count that the operation gives it.
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
