use crate::BlockUpdateFlags;
use crate::event_queue::BlockEvent;
use crate::scalar::{GameTick, ShardId, TickPriority};
use crate::sequence::SubTick;
use crate::tick_queue::Kinds;
use mcrs_minecraft_core::Direction;
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

/// Stamps of different partitions collide, so a chain is told apart by the partition that
/// started it as well.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChainOrigin {
    pub stamp: SubTick,
    pub partition: ShardId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message<K: Kinds> {
    pub stamp: SubTick,
    pub origin: ChainOrigin,
    pub body: MessageBody<K>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateRequest<K: Kinds> {
    Around {
        source: BlockPos,
        block: K::Block,
        skip: Option<Direction>,
    },
    Single {
        target: BlockPos,
        block: K::Block,
    },
    WithState {
        target: BlockPos,
        state: BlockStateId,
        block: K::Block,
        moved_by_piston: bool,
    },
    Shape {
        target: BlockPos,
        from: Direction,
        neighbour_state: BlockStateId,
        flags: BlockUpdateFlags,
        limit: u16,
    },
}

impl<K: Kinds> UpdateRequest<K> {
    pub fn pos(&self) -> BlockPos {
        match self {
            Self::Around { source, .. } => *source,
            Self::Single { target, .. }
            | Self::WithState { target, .. }
            | Self::Shape { target, .. } => *target,
        }
    }
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
    Update(UpdateRequest<K>),
}

impl<K: Kinds> Message<K> {
    pub fn section(&self) -> SectionPos {
        let pos = match &self.body {
            MessageBody::Set { pos, .. }
            | MessageBody::BlockTick { pos, .. }
            | MessageBody::FluidTick { pos, .. } => *pos,
            MessageBody::Event(event) => event.pos,
            MessageBody::Update(request) => request.pos(),
        };
        SectionPos::from(pos)
    }
}
