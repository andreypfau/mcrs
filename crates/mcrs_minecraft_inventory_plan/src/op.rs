/// One operation of a request. `I` names a slot, the cursor slot included. `S` is the stack that
/// a [`Op::Set`] puts in its slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op<I, S> {
    /// Moves exactly `count` items from `from` to `to`.
    Split { from: I, to: I, count: u8 },
    /// Moves as many items as `to` can accept from `from` to `to`.
    Merge { from: I, to: I },
    /// Exchanges the stacks of two slots.
    Swap { left: I, right: I },
    /// Removes exactly `count` items from `from` as one dropped stack.
    Drop { from: I, count: u8 },
    /// Puts `stack` with the count `count` in `slot`, or the empty stack for `None` or the count
    /// 0. It reads no filter and no slot limit, and the stack that `slot` held goes to no other
    /// slot. It makes and destroys items, so only an edge adds it.
    Set {
        slot: I,
        stack: Option<S>,
        count: u8,
    },
}
