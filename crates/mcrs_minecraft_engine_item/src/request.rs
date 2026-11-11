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
    ops: [Option<Op<SlotRef>>; MAX_OPS],
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

    pub(crate) fn id(&self) -> RequestId {
        self.id
    }

    /// The operations in order. A request with more than [`MAX_OPS`] operations gives one more,
    /// so that the planner rejects it.
    pub(crate) fn ops(&self) -> impl Iterator<Item = Op<SlotRef>> + '_ {
        let past_max = (self.ops.last().copied().flatten()).filter(|_| self.len > MAX_OPS);
        self.ops.iter().flatten().copied().chain(past_max)
    }

    fn with(mut self, op: Op<SlotRef>) -> Self {
        if let Some(free) = self.ops.get_mut(self.len) {
            *free = Some(op);
        }
        self.len = self.len.saturating_add(1);
        self
    }
}
