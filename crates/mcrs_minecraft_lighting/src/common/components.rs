use bevy_ecs::prelude::Component;

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

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct IsAllAir;

/// Inserted on a `Column` entity when a pending-egress overflow is
/// detected; consumed by the full-column reseed system.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NeedsFullReseed;

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
pub(crate) const WORKSPACE_QUEUE_BASELINE_CAPACITY: usize = 64;

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
    fn needs_full_reseed_marker_compile_test() {
        let _m = NeedsFullReseed;
    }

    #[test]
    fn is_all_air_marker_compile_test() {
        let _m = IsAllAir;
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
