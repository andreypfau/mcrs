#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod access;
mod event_queue;
mod message;
mod output;
mod policy;
mod scalar;
mod section;
mod sequence;
mod stream;
mod tick_queue;

pub use access::{PartitionKind, SectionProperties, SimulatedSections, SimulationVolume};
pub use event_queue::{BlockEvent, DrainEnd, Drained, EventQueue, QueuedEvent};
pub use message::{Message, MessageBody};
pub use output::BlockWrite;
pub use policy::{CollectScratch, OrderingPolicy, SemanticOrder, VanillaOrder};
pub use scalar::{GameTick, ShardId, TickPriority};
pub use sequence::{SequenceClock, SubTick, TickKey};
pub use stream::{StreamKey, StreamSeed};
pub use tick_queue::{
    Collected, Kinds, Queued, SavedTick, ScheduledTick, SectionOccupied, SectionTicks,
    SectionTicksRef, TickKind, TickQueue,
};

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct BlockUpdateFlags: u32 {
        const NEIGHBORS = 1;
        const CLIENTS   = 2;
        const INVISIBLE = 4;
        const IMMEDIATE = 8;
        const KNOWN_SHAPE = 16;
        const SUPPRESS_DROPS = 32;
        const MOVE_BY_PISTON = 64;
        const SKIP_SHAPE_UPDATE_ON_WIRE = 128;
        const SKIP_BLOCK_ENTITY_SIDEEFFECTS = 256;
        const SKIP_ON_PLACE = 512;
        const NONE = BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDEEFFECTS.bits() | BlockUpdateFlags::INVISIBLE.bits();
        const ALL = BlockUpdateFlags::NEIGHBORS.bits() | BlockUpdateFlags::CLIENTS.bits();
        const ALL_IMMEDIATE = BlockUpdateFlags::ALL.bits() | BlockUpdateFlags::IMMEDIATE.bits();
    }
}

#[cfg(test)]
mod tests {
    use crate::tick_queue::{QueuedTick, SectionQueue};
    use crate::{BlockWrite, Collected, Kinds, Message, QueuedEvent};

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    struct Narrow;

    impl Kinds for Narrow {
        type Block = u16;
        type Fluid = u16;
    }

    #[test]
    fn multiplied_types_stay_within_their_pinned_sizes() {
        assert!(size_of::<QueuedTick<u16>>() <= 24);
        assert!(size_of::<Collected<u16>>() <= 24);
        assert!(size_of::<QueuedEvent<u16>>() <= 32);
        assert!(size_of::<SectionQueue<u16>>() <= 80);
        assert!(size_of::<Message<Narrow>>() <= 48);
        assert!(size_of::<BlockWrite>() <= 24);
    }
}
