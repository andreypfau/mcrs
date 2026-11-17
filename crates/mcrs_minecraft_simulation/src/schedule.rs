use mcrs_minecraft_volume::{Denied, Volume};
use mcrs_voxel::BlockPos;

use crate::scalar::TickPriority;
use crate::tick_queue::Kinds;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingTick<K: Kinds> {
    Block {
        pos: BlockPos,
        block: K::Block,
        delay: i32,
        priority: TickPriority,
    },
    Fluid {
        pos: BlockPos,
        fluid: K::Fluid,
        delay: i32,
        priority: TickPriority,
    },
}

impl<K: Kinds> PendingTick<K> {
    pub fn pos(&self) -> BlockPos {
        match self {
            Self::Block { pos, .. } | Self::Fluid { pos, .. } => *pos,
        }
    }
}

pub trait ScheduleTicks<K: Kinds>: Volume {
    fn schedule_tick(&mut self, tick: PendingTick<K>) -> Result<(), Denied>;
}
