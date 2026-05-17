use crate::common::components::{Wavefront, WORKSPACE_QUEUE_BASELINE_CAPACITY};
use crate::storage::LightStorage;
use bevy_ecs::prelude::Component;
use smallvec::SmallVec;

#[derive(Component, Clone, Debug, Default)]
pub struct BlockLight(pub LightStorage);

// Inline capacity bumped 8 -> 16 on 2026-05-17 based on
// `examples/memory_profile.rs` data taken at the same time. The example
// reports per-section SmallVec occupancy across the warmup fixture; the
// pre-bump snapshot was:
//
// | Type                  | spilled% | len>8% | len>16% |
// | --------------------- | -------- | ------ | ------- |
// | BlockEgress           |    0.00% |  0.00% |   0.00% |
// | BlockIncoming         |    0.00% |  0.00% |   0.00% |
// | BlockPendingEgress    |    0.00% |  0.00% |   0.00% |
// | SkyEgress             |    0.00% |  0.00% |   0.00% |
// | SkyIncoming           |   87.50% |  0.00% |   0.00% |
// | SkyPendingEgress      |    4.17% |  4.17% |   4.17% |
//
// SkyIncoming clears the >50% spill threshold by a wide margin: ~87% of
// sections had `len > 8` at some point during warmup (forcing a heap spill
// that persists for the lifetime of the SmallVec, since we never call
// `shrink_to_fit`). The post-bump snapshot then shows SkyIncoming still at
// ~87% spilled - meaning the per-section peak is in fact above 16 for
// most chunks, not in the 9..=16 sweet spot. The bump therefore does NOT
// eliminate the SkyIncoming heap allocations.
//
// We keep the bump anyway because:
//   1. The plan-decision threshold (>50% spill -> bump to 16) is the
//      pre-registered rule, and SkyIncoming clearly satisfies it. Reverting
//      based on a post-hoc "the bump didn't help enough" reading would be
//      a moving-goalpost change.
//   2. Workloads with smaller SkyIncoming peaks (single torch, isolated
//      pit dig) are likely in the 9..=16 sweet spot, and the bump moves
//      them from "heap" to "inline" exactly as intended.
//   3. The cost is bounded: per-section inline grows from 32 B to 64 B,
//      and across the six types and the VD12 warmup fixture this totals
//      ~3 MiB of additional `wavefront_buffers` (55.3 MiB -> 56.1 MiB
//      in the measurement; the rest of the delta is SmallVec headers).
//
// The trait `WavefrontFanOut::{egress_inner_mut, pending_inner_mut}` in
// `distribute.rs` requires uniform inline size across egress/pending
// implementations, so the six types move together rather than per-type.

#[derive(Component, Clone, Debug, Default)]
pub struct BlockEgress(pub SmallVec<[Wavefront; 16]>);

#[derive(Component, Clone, Debug, Default)]
pub struct BlockIncoming(pub SmallVec<[Wavefront; 16]>);

/// Cross-chunk wavefronts that cannot fit in the destination's `*Incoming`
/// buffer yet; flushed by the cross-chunk distribute pass. Hard-capped at
/// `PENDING_EGRESS_CAP` entries; overflow triggers a `NeedsFullReseed` insert
/// on the destination column entity.
#[derive(Component, Clone, Debug, Default)]
pub struct BlockPendingEgress(pub SmallVec<[Wavefront; 16]>);

#[derive(Component, Debug)]
pub struct BlockLightWorkspace {
    pub increase_queue: Vec<u64>,
    pub decrease_queue: Vec<u64>,
}

impl Default for BlockLightWorkspace {
    fn default() -> Self {
        Self {
            increase_queue: Vec::with_capacity(WORKSPACE_QUEUE_BASELINE_CAPACITY),
            decrease_queue: Vec::with_capacity(WORKSPACE_QUEUE_BASELINE_CAPACITY),
        }
    }
}

/// Per-channel pending-BFS marker for the block-light engine.
/// Inserted by enqueue / seed / pull / distribute when a chunk's
/// block-light state needs another BFS pass; consumed by
/// `propagate_increase_block_system` at quiescence and by
/// `clear_block_bfs_pending_safety_net` as a post-converge sweep.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct BlockBfsPending;

/// Inserted on a chunk when block-light propagation has not yet been seeded for
/// the chunk's initial state. Consumed by `seed_block_emitters` per tick under
/// `LightingSet::Enqueue`. Always inserted regardless of dimension because the
/// block-light bundle is unconditional in `attach_lighting_state`.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct BlockNeedsInitialSeed;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_light_workspace_default_is_empty_with_baseline_capacity() {
        let ws = BlockLightWorkspace::default();
        assert!(ws.increase_queue.is_empty());
        assert!(ws.decrease_queue.is_empty());
        assert_eq!(ws.increase_queue.capacity(), WORKSPACE_QUEUE_BASELINE_CAPACITY);
        assert_eq!(ws.decrease_queue.capacity(), WORKSPACE_QUEUE_BASELINE_CAPACITY);
    }

    #[test]
    fn block_egress_default_is_empty() {
        let e = BlockEgress::default();
        assert!(e.0.is_empty());
        // SmallVec inline capacity is exactly 16; see the type doc above
        // for the measurement that motivates the 16-entry choice.
        let _: SmallVec<[Wavefront; 16]> = e.0;
    }

    #[test]
    fn block_pending_egress_default_is_empty() {
        let e = BlockPendingEgress::default();
        assert!(e.0.is_empty());
        let _: SmallVec<[Wavefront; 16]> = e.0;
    }

    #[test]
    fn block_light_marker_compile_test() {
        let _block_bfs = BlockBfsPending;
        let _m4 = BlockNeedsInitialSeed;
    }
}
