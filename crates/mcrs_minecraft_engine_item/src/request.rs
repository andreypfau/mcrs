use bevy_ecs::entity::Entity;
use mcrs_minecraft_inventory_plan::MAX_OPS;
use mcrs_minecraft_inventory_plan::Op;

use crate::RequestId;
use crate::SlotRef;

/// An ordered list of operations on slots that applies in full or not at all. Operations past
/// the largest number reject the request with [`Reason::TooManyOps`](crate::Reason::TooManyOps).
// chisle: the operations are inline, MAX_OPS of them, so a request never allocates; a transaction
// specification that needs a longer request lifts it.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    id: RequestId,
    ops: [Option<Op<SlotRef, Entity>>; MAX_OPS],
    len: usize,
}

impl Request {
    #[must_use]
    pub fn new(id: RequestId) -> Self {
        Self {
            id,
            ops: [None; MAX_OPS],
            len: 0,
        }
    }

    /// Moves exactly `count` items from `from` to `to`.
    #[must_use]
    pub fn split(self, from: SlotRef, to: SlotRef, count: u8) -> Self {
        self.with(Op::Split { from, to, count })
    }

    /// Moves as many items as `to` can accept from `from` to `to`.
    #[must_use]
    pub fn merge(self, from: SlotRef, to: SlotRef) -> Self {
        self.with(Op::Merge { from, to })
    }

    /// Exchanges the stacks of two slots.
    #[must_use]
    pub fn swap(self, left: SlotRef, right: SlotRef) -> Self {
        self.with(Op::Swap { left, right })
    }

    /// Puts the stack entity `stack` with the count `count` in `slot`, or the empty stack for
    /// `None` or the count 0, past every filter and slot limit; the stack that `slot` held ends.
    /// A stack that has no place moves to the slot; the slot gets a copy of each other stack. An
    /// entity that is not a stack sets the empty stack. It makes and destroys items, so only an
    /// edge adds it.
    #[must_use]
    pub fn set(self, slot: SlotRef, stack: Option<Entity>, count: u8) -> Self {
        self.with(Op::Set { slot, stack, count })
    }

    pub(crate) fn id(&self) -> RequestId {
        self.id
    }

    /// The operations in order. A request with more than [`MAX_OPS`] operations gives one more,
    /// so that the planner rejects it.
    pub(crate) fn ops(&self) -> impl Iterator<Item = Op<SlotRef, Entity>> + '_ {
        let past_max = (self.ops.last().copied().flatten()).filter(|_| self.len > MAX_OPS);
        self.ops.iter().flatten().copied().chain(past_max)
    }

    fn with(mut self, op: Op<SlotRef, Entity>) -> Self {
        if let Some(free) = self.ops.get_mut(self.len) {
            *free = Some(op);
        }
        self.len = self.len.saturating_add(1);
        self
    }
}
