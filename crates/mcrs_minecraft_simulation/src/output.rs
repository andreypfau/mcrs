use crate::event_queue::BlockEvent;
use crate::tick_queue::Kinds;
use crate::{BlockUpdateFlags, Message};
use mcrs_minecraft_core::{BlockPos, BlockStateId, SectionPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockWrite {
    pub pos: BlockPos,
    pub old: BlockStateId,
    pub new: BlockStateId,
    pub flags: BlockUpdateFlags,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepReport {
    pub unaccepted_ticks: u32,
    pub refused_sets: u32,
    pub unloaded_drops: u32,
    pub events_deferred: u32,
    pub events_left_by_bound: u32,
    pub foreign_reads: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepOutput<K: Kinds> {
    pub writes: Vec<BlockWrite>,
    pub marked: Vec<BlockPos>,
    pub outbound: Vec<Message<K>>,
    pub delivered: Vec<BlockEvent<K::Block>>,
    pub report: StepReport,
}

impl<K: Kinds> Default for StepOutput<K> {
    fn default() -> Self {
        Self {
            writes: Vec::new(),
            marked: Vec::new(),
            outbound: Vec::new(),
            delivered: Vec::new(),
            report: StepReport::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StepError {
    #[error("message for section {section} reached a partition that does not own it")]
    NotOwned { section: SectionPos },
}
