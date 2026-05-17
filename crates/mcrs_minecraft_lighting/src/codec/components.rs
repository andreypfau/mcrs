use crate::storage::LightStorage;
use bevy_ecs::prelude::Component;
use smallvec::SmallVec;

#[derive(Component, Clone, Debug, Default)]
pub struct BlockLight(pub LightStorage);

#[derive(Component, Clone, Debug, Default)]
pub struct SkyLight(pub LightStorage);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wavefront(pub u32);

impl Wavefront {
    pub const fn new(face: u8, cell_x: u8, cell_z: u8, level: u8) -> Self {
        debug_assert!(face < 8);
        debug_assert!(cell_x < 16);
        debug_assert!(cell_z < 16);
        debug_assert!(level < 16);
        let packed = (face as u32 & 0b111)
            | ((cell_x as u32 & 0b1111) << 3)
            | ((cell_z as u32 & 0b1111) << 7)
            | ((level as u32 & 0b1111) << 11);
        Wavefront(packed)
    }

    pub const fn face(self) -> u8 {
        (self.0 & 0b111) as u8
    }

    pub const fn cell_x(self) -> u8 {
        ((self.0 >> 3) & 0b1111) as u8
    }

    pub const fn cell_z(self) -> u8 {
        ((self.0 >> 7) & 0b1111) as u8
    }

    pub const fn level(self) -> u8 {
        ((self.0 >> 11) & 0b1111) as u8
    }
}

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

#[derive(Component, Clone, Debug, Default)]
pub struct SkyEgress(pub SmallVec<[Wavefront; 16]>);

#[derive(Component, Clone, Debug, Default)]
pub struct SkyIncoming(pub SmallVec<[Wavefront; 16]>);

/// Cross-chunk wavefronts that cannot fit in the destination's `*Incoming`
/// buffer yet; flushed by the cross-chunk distribute pass. Hard-capped at
/// `PENDING_EGRESS_CAP` entries; overflow triggers a `NeedsFullReseed` insert
/// on the destination column entity.
#[derive(Component, Clone, Debug, Default)]
pub struct BlockPendingEgress(pub SmallVec<[Wavefront; 16]>);

/// Sky-light counterpart of `BlockPendingEgress`; same overflow semantics.
#[derive(Component, Clone, Debug, Default)]
pub struct SkyPendingEgress(pub SmallVec<[Wavefront; 16]>);

/// Baseline BFS-queue capacity for both workspace queues.
///
/// Measured 2026-05-17 via `examples/memory_profile.rs` on the VD12 warmup
/// fixture (`bench_helpers::build_warmed_vd12_app_in_place`, 15000
/// chunk-sections):
///
/// - Block-light queues:     0 / 15000 chunks ever used them in the warmup
///                           fixture (no torches, no block updates).
/// - Sky-light increase:     13125 / 15000 chunks used it. Of the nonzero
///                           subset: p50 cap = 4096, p95 = 4096, p99/max = 8192
///                           - but this is the one-shot initial-seed flood,
///                           not typical mid-tick load.
/// - Sky-light decrease:     0 / 15000 chunks ever used it in warmup.
///
/// The warmup-peak numbers reflect the heroic initial-seed pass and are not a
/// good baseline for per-tick steady state. The 64-entry baseline absorbs the
/// typical mid-frame `0 -> 4 -> 8 -> 16 -> 32 -> 64` growth chain in one
/// allocation while keeping the per-section memory cost bounded: at
/// 60_000 queues * 64 * 8 B (`Vec<u64>` slot size) the upper bound is
/// ~30 MiB, vs ~1.8 GiB if we pre-allocated to the warmup p99 of ~8 K.
const WORKSPACE_QUEUE_BASELINE_CAPACITY: usize = 64;

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

/// Per-channel pending-BFS marker for the block-light engine.
/// Inserted by enqueue / seed / pull / distribute when a chunk's
/// block-light state needs another BFS pass; consumed by
/// `propagate_increase_block_system` at quiescence and by
/// `clear_block_bfs_pending_safety_net` as a post-converge sweep.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct BlockBfsPending;

/// Per-channel pending-BFS marker for the sky-light engine.
/// Inserted by enqueue / seed / pull / distribute when a chunk's
/// sky-light state needs another BFS pass; consumed by
/// `propagate_increase_sky_system` at quiescence and by
/// `clear_sky_bfs_pending_safety_net` as a post-converge sweep.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SkyBfsPending;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct IsAllAir;

/// Inserted on a chunk when block-light propagation has not yet been seeded for
/// the chunk's initial state. Consumed by `seed_block_emitters` per tick under
/// `LightingSet::Enqueue`. Always inserted regardless of dimension because the
/// block-light bundle is unconditional in `attach_lighting_state`.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct BlockNeedsInitialSeed;

/// Inserted on a chunk when sky-light propagation has not yet been seeded for
/// the chunk's initial state. Consumed by `seed_sky_initial` per tick under
/// `LightingSet::Enqueue`. Inserted only when the chunk's parent dimension
/// carries `HasSkyLight`.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SkyNeedsInitialSeed;

/// Inserted on a `Column` entity when a pending-egress overflow is
/// detected; consumed by the full-column reseed system.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NeedsFullReseed;

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
    fn wavefront_pack_unpack_round_trip() {
        for face in 0u8..8 {
            for cell_x in [0u8, 7, 15] {
                for cell_z in [0u8, 7, 15] {
                    for level in [0u8, 7, 15] {
                        let w = Wavefront::new(face, cell_x, cell_z, level);
                        assert_eq!(w.face(), face, "face mismatch for {face},{cell_x},{cell_z},{level}");
                        assert_eq!(w.cell_x(), cell_x, "cell_x mismatch");
                        assert_eq!(w.cell_z(), cell_z, "cell_z mismatch");
                        assert_eq!(w.level(), level, "level mismatch");
                    }
                }
            }
        }
    }

    #[test]
    fn wavefront_reserved_bits_are_zero() {
        let w = Wavefront::new(7, 15, 15, 15);
        assert_eq!(w.0 >> 15, 0, "reserved bits 15..31 must be zero");
    }

    #[test]
    fn block_light_workspace_default_is_empty_with_baseline_capacity() {
        let ws = BlockLightWorkspace::default();
        assert!(ws.increase_queue.is_empty());
        assert!(ws.decrease_queue.is_empty());
        assert_eq!(ws.increase_queue.capacity(), WORKSPACE_QUEUE_BASELINE_CAPACITY);
        assert_eq!(ws.decrease_queue.capacity(), WORKSPACE_QUEUE_BASELINE_CAPACITY);
    }

    #[test]
    fn sky_light_workspace_default_is_empty_with_baseline_capacity() {
        let ws = SkyLightWorkspace::default();
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
    fn sky_pending_egress_default_is_empty() {
        let e = SkyPendingEgress::default();
        assert!(e.0.is_empty());
        let _: SmallVec<[Wavefront; 16]> = e.0;
    }

    #[test]
    fn light_dirty_marker_compile_test() {
        let _block_bfs = BlockBfsPending;
        let _sky_bfs = SkyBfsPending;
        let _m2 = IsAllAir;
        let _m4 = BlockNeedsInitialSeed;
        let _m5 = SkyNeedsInitialSeed;
        let _m6 = NeedsRetop;
    }

    #[test]
    fn needs_full_reseed_marker_compile_test() {
        let _m = NeedsFullReseed;
    }

    #[test]
    fn sky_light_seeded_as_topmost_marker_compile_test() {
        let _m = SkyLightSeededAsTopmost;
    }

    #[test]
    fn wavefront_default_is_zero() {
        let w = Wavefront::default();
        assert_eq!(w.0, 0);
        assert_eq!(w.face(), 0);
        assert_eq!(w.cell_x(), 0);
        assert_eq!(w.cell_z(), 0);
        assert_eq!(w.level(), 0);
    }
}
