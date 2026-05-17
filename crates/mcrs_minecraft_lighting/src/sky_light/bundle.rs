use bevy_ecs::bundle::Bundle;
use crate::{SkyBfsQueues, SkyInbox, SkyLight, SkyOutbox, SkyParkedEgress};
use crate::codec::LightStorage;
use crate::nibble::LightNibbles;

#[derive(Bundle)]
pub struct SkyLightBundle {
    pub light: SkyLight,
    pub outbox: SkyOutbox,
    pub inbox: SkyInbox,
    pub queues: SkyBfsQueues,
    pub parked_egress: SkyParkedEgress,
}

// Sky-light propagation shares the same Empty->Uniform-on-first-write hazard
// described above for BlockLightBundle. Without explicit `Dense(zeros)` the
// first top-face seed at level 15 promotes storage to `Uniform(15)`, which
// then reports 15 for every cell and short-circuits per-cell BFS
// attenuation through partial-air chunks (e.g. one with a water cell).
// The column-walker fast path in `propagate_increase_sky_system` writes
// `Uniform(15)` directly when the chunk is all-air, so this initial Dense
// state only matters for the BFS path.
impl Default for SkyLightBundle {
    fn default() -> Self {
        Self {
            light: SkyLight(LightStorage::Dense(Box::new(LightNibbles::zeros()))),
            outbox: SkyOutbox::default(),
            inbox: SkyInbox::default(),
            queues: SkyBfsQueues::default(),
            parked_egress: SkyParkedEgress::default(),
        }
    }
}