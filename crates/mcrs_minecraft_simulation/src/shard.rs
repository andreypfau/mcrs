use crate::Message;
use crate::access::PartitionKind;
use crate::chain::Chain;
use crate::event_queue::EventQueue;
use crate::output::ChainLedger;
use crate::policy::CollectScratch;
use crate::scalar::{GameTick, ShardId};
use crate::sequence::SequenceClock;
use crate::tick_queue::{Collected, Kinds, TickQueue};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_volume::PaletteBuffer;
use rustc_hash::FxHashSet;

pub struct BlockShard<K: Kinds> {
    pub(crate) clock: SequenceClock,
    pub(crate) block_ticks: TickQueue<K::Block>,
    pub(crate) fluid_ticks: TickQueue<K::Fluid>,
    pub(crate) events: EventQueue<K::Block>,
    pub(crate) scratch: CollectScratch,
    pub(crate) collected_blocks: Vec<Collected<K::Block>>,
    pub(crate) collected_fluids: Vec<Collected<K::Fluid>>,
    pub(crate) ticking: FxHashSet<(BlockPos, K::Block)>,
    pub(crate) palette: PaletteBuffer,
    pub(crate) chain: Chain<K>,
}

impl<K: Kinds> BlockShard<K> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K: Kinds> Default for BlockShard<K> {
    fn default() -> Self {
        Self {
            clock: SequenceClock::new(),
            block_ticks: TickQueue::new(),
            fluid_ticks: TickQueue::new(),
            events: EventQueue::new(),
            scratch: CollectScratch::default(),
            collected_blocks: Vec::new(),
            collected_fluids: Vec::new(),
            ticking: FxHashSet::default(),
            palette: PaletteBuffer::default(),
            chain: Chain::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelConfig {
    pub tick_budget: u32,
    pub event_drain_bound: u32,
    pub chain_limit: ChainLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainLimit {
    Limited(u32),
    Unlimited,
}

impl ChainLimit {
    pub fn from_property(value: i32) -> Self {
        u32::try_from(value).map_or(Self::Unlimited, Self::Limited)
    }
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            tick_budget: 65_536,
            event_drain_bound: 1_000_000,
            chain_limit: ChainLimit::Limited(1_000_000),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Ingress,
    BlockTicks,
    FluidTicks,
    BlockEvents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepAt {
    pub now: GameTick,
    pub phase: Phase,
}

pub struct Env<'a, F, B, P> {
    /// Unique among the partitions whose messages can meet in one inbound batch, and not
    /// reused within a game tick: chains are keyed by it together with their first stamp.
    pub partition: ShardId,
    pub facts: &'a F,
    pub behaviour: &'a B,
    pub policy: &'a P,
    pub kind: PartitionKind,
    pub config: &'a KernelConfig,
}

pub struct PhaseInput<'a, K: Kinds> {
    pub inbound: &'a [Message<K>],
    pub ledgers: &'a [ChainLedger],
}
