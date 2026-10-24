use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_volume::{Denied, Volume};

use crate::scalar::TickPriority;
use crate::tick_queue::Kinds;

/// A tick a write asked for before any queue holds it. Its delay counts from when a queue
/// takes it.
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

/// A volume that keeps the ticks its writes ask for, in the order they were asked, for
/// the queue that later runs its sections.
pub trait ScheduleTicks<K: Kinds>: Volume {
    /// Refused, like a write, where the volume may not write.
    fn schedule_tick(&mut self, tick: PendingTick<K>) -> Result<(), Denied>;
}
