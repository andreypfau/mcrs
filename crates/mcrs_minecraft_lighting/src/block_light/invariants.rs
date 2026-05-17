#![cfg(any(test, debug_assertions))]
//! Block-light invariant checker. Verifies three invariants for every
//! cell in the 16×16×16 chunk:
//!
//! 1. **Source floor:** the stored level is at least the cell's own emission.
//! 2. **Support floor:** the stored level is at least the maximum inward
//!    contribution from the six cardinal neighbours, where each neighbour
//!    contributes `neighbour_level - max(1, dampening_of_self)` unless the
//!    cell-cell face is fully occluded.
//! 3. **Source-only excess:** when the stored level exceeds the inward
//!    support, the excess equals the cell's own emission — i.e. only
//!    emitter cells may be brighter than the surrounding field supports.

use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_block::palette::BlockPalette;
use crate::codec::LightStorage;
use crate::invariants::{neighbour_contribution, CHUNK_DIM, DIRECTIONS};
pub use crate::invariants::{InvariantViolation, ViolationKind};
use crate::table::BlockStateLightTable;

pub fn check_block_light_invariants(
    table: &BlockStateLightTable,
    palette: &BlockPalette,
    light: &LightStorage,
) -> Result<(), InvariantViolation> {
    for y in 0..CHUNK_DIM {
        for z in 0..CHUNK_DIM {
            for x in 0..CHUNK_DIM {
                let state = palette.get(BlockPos::new(x, y, z));
                let emitted = table.emission_for(state);
                let stored = light.get(x as usize, y as usize, z as usize);
                let cell = BlockPos::new(x, y, z);

                if stored < emitted {
                    return Err(InvariantViolation {
                        cell,
                        stored,
                        emitted,
                        max_support: 0,
                        kind: ViolationKind::SourceFloor,
                    });
                }

                let mut max_inward_support: u8 = 0;
                for d in DIRECTIONS {
                    if let Some(contribution) =
                        neighbour_contribution(d, x, y, z, state, table, palette, light)
                        && contribution > max_inward_support
                    {
                        max_inward_support = contribution;
                    }
                }

                if stored < max_inward_support {
                    return Err(InvariantViolation {
                        cell,
                        stored,
                        emitted,
                        max_support: max_inward_support,
                        kind: ViolationKind::SupportFloor,
                    });
                }

                if stored > max_inward_support && stored != emitted {
                    return Err(InvariantViolation {
                        cell,
                        stored,
                        emitted,
                        max_support: max_inward_support,
                        kind: ViolationKind::SourceExcess,
                    });
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nibble::LightNibbles;
    use crate::table::flag_bits;
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_protocol::BlockStateId;

    const AIR: BlockStateId = BlockStateId(0);
    const TORCH: BlockStateId = BlockStateId(0x1000);

    fn make_test_table() -> BlockStateLightTable {
        let state_count: usize = 0x1001;
        let mut emission = vec![0u8; state_count].into_boxed_slice();
        let mut dampening = vec![0u8; state_count].into_boxed_slice();
        let occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let mut flags = vec![0u8; state_count].into_boxed_slice();

        emission[0] = 0;
        dampening[0] = 0;
        flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

        emission[0x1000] = 14;
        dampening[0x1000] = 0;
        flags[0x1000] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

        BlockStateLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        }
    }

    fn make_palette(emitters: &[(i32, i32, i32, BlockStateId)]) -> BlockPalette {
        let mut p = BlockPalette::default();
        p.fill(AIR);
        for (x, y, z, state) in emitters {
            p.set(BlockPos::new(*x, *y, *z), *state);
        }
        p
    }

    fn seed_light(light: &mut LightStorage, cells: &[((usize, usize, usize), u8)]) {
        for ((x, y, z), val) in cells {
            light.set(*x, *y, *z, *val);
        }
    }

    /// Returns a `LightStorage::Dense` backed by a zeroed `LightNibbles`. The
    /// test helpers use this rather than the `Null` default so that
    /// `LightStorage::set` does not promote the storage to `Uniform(v)` on
    /// the first nonzero write (which would clobber every other cell with
    /// the same value and mask the per-cell behaviour under test).
    fn air_storage() -> LightStorage {
        LightStorage::Dense(Box::new(LightNibbles::zeros()))
    }

    #[test]
    fn invariants_pass_on_valid_uniform_field() {
        let table = make_test_table();
        let palette = make_palette(&[]);
        let light = air_storage();
        assert!(check_block_light_invariants(&table, &palette, &light).is_ok());
    }

    #[test]
    fn invariants_fail_source_floor_below_emission() {
        let table = make_test_table();
        let palette = make_palette(&[(0, 0, 0, TORCH)]);
        let mut light = air_storage();
        seed_light(&mut light, &[((0, 0, 0), 7)]);

        let err = check_block_light_invariants(&table, &palette, &light)
            .expect_err("expected SourceFloor violation");
        assert_eq!(err.kind, ViolationKind::SourceFloor);
        assert_eq!(err.cell, BlockPos::new(0, 0, 0));
        assert_eq!(err.stored, 7);
        assert_eq!(err.emitted, 14);
    }

    #[test]
    fn invariants_fail_support_floor() {
        let table = make_test_table();
        let palette = make_palette(&[(0, 0, 0, TORCH)]);
        let mut light = air_storage();
        seed_light(&mut light, &[((0, 0, 0), 14)]);

        let err = check_block_light_invariants(&table, &palette, &light)
            .expect_err("expected SupportFloor violation");
        assert_eq!(err.kind, ViolationKind::SupportFloor);
        assert_eq!(err.cell, BlockPos::new(1, 0, 0));
        assert_eq!(err.stored, 0);
        assert_eq!(err.max_support, 13);
    }

    #[test]
    fn invariants_fail_source_excess() {
        let table = make_test_table();
        let palette = make_palette(&[]);
        let mut light = air_storage();
        seed_light(&mut light, &[((0, 0, 0), 14)]);

        let err = check_block_light_invariants(&table, &palette, &light)
            .expect_err("expected SourceExcess violation");
        assert_eq!(err.kind, ViolationKind::SourceExcess);
        assert_eq!(err.cell, BlockPos::new(0, 0, 0));
        assert_eq!(err.stored, 14);
        assert_eq!(err.emitted, 0);
    }

    #[test]
    fn invariants_skip_outward_face_support() {
        // L1-attenuated field originating from a torch at (0,0,0):
        //   level(x, y, z) = max(0, 14 - (x + y + z))
        // The boundary cells on the -X, -Y, -Z faces have their outward
        // neighbours in a sibling chunk, so the support-floor check must
        // skip those directions; otherwise (0, 0, 0)'s missing neighbours
        // would read garbage from `BlockPalette::get`'s `& 15` wrap-around.
        let table = make_test_table();
        let palette = make_palette(&[(0, 0, 0, TORCH)]);
        let mut light = air_storage();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let d = (x + y + z) as i32;
                    let level = (14i32 - d).max(0) as u8;
                    light.set(x, y, z, level);
                }
            }
        }
        assert!(
            check_block_light_invariants(&table, &palette, &light).is_ok(),
            "expected Ok(()) for the L1-attenuated field from a (0,0,0) torch"
        );
    }
}
