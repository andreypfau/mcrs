use crate::event_queue::BlockEvent;
use crate::tick_queue::Kinds;
use crate::{BlockUpdateFlags, ChainOrigin, Message};
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockWrite {
    pub pos: BlockPos,
    pub old: BlockStateId,
    pub new: BlockStateId,
    pub flags: BlockUpdateFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Destroyed {
    pub pos: BlockPos,
    pub state: BlockStateId,
    pub drops: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainLedger {
    pub origin: ChainOrigin,
    pub offered: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepReport {
    pub unaccepted_ticks: u32,
    pub refused_sets: u32,
    pub unloaded_drops: u32,
    pub events_deferred: u32,
    pub events_left_by_bound: u32,
    pub foreign_reads: u32,
    pub first_chain_overflow: Option<BlockPos>,
    pub chain_overflows: u32,
    /// Random picks whose footprint left the region of their block tile, undone and run again
    /// in the serial step.
    pub random_reruns: u32,
}

impl StepReport {
    /// Adds the counts of `other`, a later part of the same step.
    pub(crate) fn absorb(&mut self, other: Self) {
        let Self {
            unaccepted_ticks,
            refused_sets,
            unloaded_drops,
            events_deferred,
            events_left_by_bound,
            foreign_reads,
            first_chain_overflow,
            chain_overflows,
            random_reruns,
        } = other;
        let add = |count: &mut u32, more: u32| *count = count.saturating_add(more);
        add(&mut self.unaccepted_ticks, unaccepted_ticks);
        add(&mut self.refused_sets, refused_sets);
        add(&mut self.unloaded_drops, unloaded_drops);
        add(&mut self.events_deferred, events_deferred);
        add(&mut self.events_left_by_bound, events_left_by_bound);
        add(&mut self.foreign_reads, foreign_reads);
        add(&mut self.chain_overflows, chain_overflows);
        add(&mut self.random_reruns, random_reruns);
        self.first_chain_overflow = self.first_chain_overflow.or(first_chain_overflow);
    }
}

/// The lengths of a [`StepOutput`] at one point, to drop what came after it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OutputMark {
    pub(crate) writes: usize,
    outbound: usize,
    delivered: usize,
    destroyed: usize,
    chains: usize,
    report: StepReport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepOutput<K: Kinds> {
    pub writes: Vec<BlockWrite>,
    pub outbound: Vec<Message<K>>,
    pub delivered: Vec<BlockEvent<K::Block>>,
    pub destroyed: Vec<Destroyed>,
    pub chains: Vec<ChainLedger>,
    pub report: StepReport,
}

impl<K: Kinds> Default for StepOutput<K> {
    fn default() -> Self {
        Self {
            writes: Vec::new(),
            outbound: Vec::new(),
            delivered: Vec::new(),
            destroyed: Vec::new(),
            chains: Vec::new(),
            report: StepReport::default(),
        }
    }
}

impl<K: Kinds> StepOutput<K> {
    /// Empties the output and keeps the capacity of its lists.
    pub fn clear(&mut self) {
        self.writes.clear();
        self.outbound.clear();
        self.delivered.clear();
        self.destroyed.clear();
        self.chains.clear();
        self.report = StepReport::default();
    }

    pub(crate) fn mark(&self) -> OutputMark {
        OutputMark {
            writes: self.writes.len(),
            outbound: self.outbound.len(),
            delivered: self.delivered.len(),
            destroyed: self.destroyed.len(),
            chains: self.chains.len(),
            report: self.report,
        }
    }

    /// Drops what came after `mark`, the writes included: the caller undoes them in the volume.
    pub(crate) fn truncate(&mut self, mark: OutputMark) {
        self.writes.truncate(mark.writes);
        self.outbound.truncate(mark.outbound);
        self.delivered.truncate(mark.delivered);
        self.destroyed.truncate(mark.destroyed);
        self.chains.truncate(mark.chains);
        self.report = mark.report;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StepError {
    #[error("message for section {section} reached a partition that does not own it")]
    NotOwned { section: SectionPos },
    #[error("a tick or event for section {section} reached a cell, which holds no queues")]
    NoQueues { section: SectionPos },
}
