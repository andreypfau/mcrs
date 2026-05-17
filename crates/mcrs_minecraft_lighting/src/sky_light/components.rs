use crate::common::components::{Wavefront, WORKSPACE_QUEUE_BASELINE_CAPACITY};
use crate::storage::LightStorage;
use bevy_ecs::prelude::Component;
use smallvec::SmallVec;

#[derive(Component, Clone, Debug, Default)]
pub struct SkyLight(pub LightStorage);

#[derive(Component, Clone, Debug, Default)]
pub struct SkyEgress(pub SmallVec<[Wavefront; 16]>);

#[derive(Component, Clone, Debug, Default)]
pub struct SkyIncoming(pub SmallVec<[Wavefront; 16]>);

/// Sky-light counterpart of `BlockPendingEgress`; same overflow semantics.
#[derive(Component, Clone, Debug, Default)]
pub struct SkyPendingEgress(pub SmallVec<[Wavefront; 16]>);

#[derive(Component, Debug)]
pub struct SkyLightWorkspace {
    pub increase_queue: Vec<u64>,
    pub decrease_queue: Vec<u64>,
}

impl Default for SkyLightWorkspace {
    fn default() -> Self {
        Self {
            increase_queue: Vec::with_capacity(WORKSPACE_QUEUE_BASELINE_CAPACITY),
            decrease_queue: Vec::with_capacity(WORKSPACE_QUEUE_BASELINE_CAPACITY),
        }
    }
}

/// Per-channel pending-BFS marker for the sky-light engine.
/// Inserted by enqueue / seed / pull / distribute when a chunk's
/// sky-light state needs another BFS pass; consumed by
/// `propagate_increase_sky_system` at quiescence and by
/// `clear_sky_bfs_pending_safety_net` as a post-converge sweep.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SkyBfsPending;

/// Inserted on a chunk when sky-light propagation has not yet been seeded for
/// the chunk's initial state. Consumed by `seed_sky_initial` per tick under
/// `LightingSet::Enqueue`. Inserted only when the chunk's parent dimension
/// carries `HasSkyLight`.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SkyNeedsInitialSeed;

/// Marks a chunk whose sky light was seeded as the topmost chunk
/// of its column. Invalidated when a new chunk spawns above this one.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SkyLightSeededAsTopmost;

/// Self-cleaning queue for retopping decrease waves between `seed_sky_initial`
/// and `invalidate_previous_topmost`. The producer (`seed_sky_initial`) inserts
/// this on the previous topmost chunk when a new chunk becomes the topmost of
/// its column. The consumer (`invalidate_previous_topmost`) runs the per-cell
/// decrease-queue push body and removes the marker. Visibility `pub(crate)`
/// because no downstream consumer is known; the `apply_deferred` barrier
/// between `LightingSet::Enqueue` substages makes the marker visible to the
/// consumer in the same tick.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub(crate) struct NeedsRetop;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_light_workspace_default_is_empty_with_baseline_capacity() {
        let ws = SkyLightWorkspace::default();
        assert!(ws.increase_queue.is_empty());
        assert!(ws.decrease_queue.is_empty());
        assert_eq!(ws.increase_queue.capacity(), WORKSPACE_QUEUE_BASELINE_CAPACITY);
        assert_eq!(ws.decrease_queue.capacity(), WORKSPACE_QUEUE_BASELINE_CAPACITY);
    }

    #[test]
    fn sky_pending_egress_default_is_empty() {
        let e = SkyPendingEgress::default();
        assert!(e.0.is_empty());
        let _: SmallVec<[Wavefront; 16]> = e.0;
    }

    #[test]
    fn sky_light_marker_compile_test() {
        let _sky_bfs = SkyBfsPending;
        let _m5 = SkyNeedsInitialSeed;
        let _m6 = NeedsRetop;
    }

    #[test]
    fn sky_light_seeded_as_topmost_marker_compile_test() {
        let _m = SkyLightSeededAsTopmost;
    }
}
