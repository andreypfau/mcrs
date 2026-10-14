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

mod event_queue;
mod policy;
mod scalar;
mod section;
mod sequence;
mod stream;
mod tick_queue;

pub use event_queue::{BlockEvent, DrainEnd, Drained, EventQueue, QueuedEvent};
pub use policy::{CollectScratch, OrderingPolicy, SemanticOrder, VanillaOrder};
pub use scalar::{GameTick, ShardId, TickPriority};
pub use sequence::{SequenceClock, SubTick, TickKey};
pub use stream::{StreamKey, StreamSeed};
pub use tick_queue::{
    Collected, Queued, SavedTick, ScheduledTick, SectionOccupied, SectionTicks, SectionTicksRef,
    TickKind, TickQueue,
};

#[cfg(test)]
mod tests {
    use crate::tick_queue::{QueuedTick, SectionQueue};
    use crate::{Collected, QueuedEvent};

    #[test]
    fn multiplied_types_stay_within_their_pinned_sizes() {
        assert!(size_of::<QueuedTick<u16>>() <= 24);
        assert!(size_of::<Collected<u16>>() <= 24);
        assert!(size_of::<QueuedEvent<u16>>() <= 32);
        assert!(size_of::<SectionQueue<u16>>() <= 80);
    }
}
