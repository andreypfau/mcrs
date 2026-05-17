use bevy_ecs::bundle::Bundle;
use crate::{BlockBfsQueues, BlockInbox, BlockLight, BlockOutbox, BlockParkedEgress};
use crate::codec::LightStorage;
use crate::nibble::LightNibbles;

#[derive(Bundle)]
pub struct BlockLightBundle {
    pub light: BlockLight,
    pub outbox: BlockOutbox,
    pub inbox: BlockInbox,
    pub queues: BlockBfsQueues,
    pub parked_egress: BlockParkedEgress,
}

// `LightStorage::set` promotes `Empty -> Uniform(v)` on the first non-zero write,
// which blanket-fills every cell with `v` and breaks per-cell BFS propagation.
// Chunks that participate in block-light propagation start with explicit
// `Dense(zeros)` so per-cell writes stay independent. An idle-time compaction
// pass will revisit empty chunks later.
impl Default for BlockLightBundle {
    fn default() -> Self {
        Self {
            light: BlockLight(LightStorage::Dense(Box::new(LightNibbles::zeros()))),
            outbox: BlockOutbox::default(),
            inbox: BlockInbox::default(),
            queues: BlockBfsQueues::default(),
            parked_egress: BlockParkedEgress::default(),
        }
    }
}