/// One operation of a request. `I` names a slot, the cursor slot included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op<I> {
    /// Moves exactly `count` items from `from` to `to`.
    Split { from: I, to: I, count: u8 },
    /// Moves as many items as `to` can accept from `from` to `to`.
    Merge { from: I, to: I },
    /// Exchanges the stacks of two slots.
    Swap { left: I, right: I },
    /// Removes exactly `count` items from `from` as one dropped stack.
    Drop { from: I, count: u8 },
}
