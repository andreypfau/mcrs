use crate::BlockUpdateFlags;
use crate::event_queue::BlockEvent;
use crate::scalar::{GameTick, TickPriority};
use crate::sequence::SubTick;
use crate::tick_queue::Kinds;
use mcrs_minecraft_core::{BlockPos, BlockStateId, SectionPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message<K: Kinds> {
    pub stamp: SubTick,
    pub origin: SubTick,
    pub body: MessageBody<K>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageBody<K: Kinds> {
    Set {
        pos: BlockPos,
        state: BlockStateId,
        flags: BlockUpdateFlags,
        limit: u16,
    },
    BlockTick {
        pos: BlockPos,
        block: K::Block,
        due: GameTick,
        priority: TickPriority,
    },
    FluidTick {
        pos: BlockPos,
        fluid: K::Fluid,
        due: GameTick,
        priority: TickPriority,
    },
    Event(BlockEvent<K::Block>),
}

impl<K: Kinds> Message<K> {
    pub fn section(&self) -> SectionPos {
        let pos = match &self.body {
            MessageBody::Set { pos, .. }
            | MessageBody::BlockTick { pos, .. }
            | MessageBody::FluidTick { pos, .. } => *pos,
            MessageBody::Event(event) => event.pos,
        };
        SectionPos::from(pos)
    }
}
