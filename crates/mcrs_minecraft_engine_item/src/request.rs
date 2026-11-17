use bevy_ecs::entity::Entity;
use mcrs_minecraft_inventory_transaction::MAX_OPS;
use mcrs_minecraft_inventory_transaction::Op;

use crate::RequestId;
use crate::RequestKind;
use crate::SlotRef;

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    id: RequestId,
    kind: RequestKind,
    ops: [Option<Op<SlotRef, Entity>>; MAX_OPS],
    len: usize,
}

impl Request {
    #[must_use]
    pub fn new(id: RequestId) -> Self {
        Self::of_kind(id, RequestKind::Action)
    }

    #[must_use]
    pub fn close(id: RequestId) -> Self {
        Self::of_kind(id, RequestKind::Close)
    }

    fn of_kind(id: RequestId, kind: RequestKind) -> Self {
        Self {
            id,
            kind,
            ops: [None; MAX_OPS],
            len: 0,
        }
    }

    #[must_use]
    pub fn split(self, from: SlotRef, to: SlotRef, count: u8) -> Self {
        self.with(Op::Split { from, to, count })
    }

    #[must_use]
    pub fn merge(self, from: SlotRef, to: SlotRef) -> Self {
        self.with(Op::Merge { from, to })
    }

    #[must_use]
    pub fn swap(self, left: SlotRef, right: SlotRef) -> Self {
        self.with(Op::Swap { left, right })
    }

    #[must_use]
    pub fn drop(self, from: SlotRef, count: u8) -> Self {
        self.with(Op::Drop { from, count })
    }

    #[must_use]
    pub fn set(self, slot: SlotRef, stack: Option<Entity>, count: u8) -> Self {
        self.with(Op::Set { slot, stack, count })
    }

    #[must_use]
    pub fn id(&self) -> RequestId {
        self.id
    }

    #[must_use]
    pub fn kind(&self) -> RequestKind {
        self.kind
    }

    pub fn ops(&self) -> impl Iterator<Item = Op<SlotRef, Entity>> + '_ {
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
