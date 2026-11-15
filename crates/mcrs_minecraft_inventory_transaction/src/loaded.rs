use crate::Filter;
use crate::Planned;

/// The inputs of one slot, read once for each request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loaded<T> {
    /// The stack of the slot, `None` for the empty stack.
    pub stack: Option<Planned<T>>,
    /// The largest count that the slot can hold.
    pub limit: u8,
    /// Whether the player can remove items from the slot.
    pub take: Filter,
}
