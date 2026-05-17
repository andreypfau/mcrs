#![cfg(any(test, debug_assertions))]
//! Sky-light invariant checker. Enforces three invariants:
//! - `TopRowFloor`: when the chunk is the topmost of a sky-having column,
//!   every y=15 air cell (`PROPAGATES_SKYLIGHT_DOWN`) must store the sky
//!   maximum (15).
//! - `SupportFloor`: every cell's stored level is at least the maximum
//!   sky-aware inward contribution from its six cardinal neighbours.
//! - `SourceExcess`: no cell may exceed its inward support, except for the
//!   top-row air cells of a topmost-of-column chunk which receive their
//!   light from the open-sky source above the chunk.
//!
//! Inward contributions are computed against the sky-aware oracle, which
//! mirrors the BFS vertical-drop rule: a downward step from a level-15 cell
//! into a destination with `PROPAGATES_SKYLIGHT_DOWN` propagates 15
//! unattenuated rather than the unified attenuation.
//!
//! The `is_topmost_in_skyhaving_column` flag is passed by the caller rather
//! than derived from ECS state so the checker stays a pure function reachable
//! from both unit tests and a debug-only verification system.

use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;
use crate::codec::LightStorage;
use crate::invariants::{direction_offset, CHUNK_DIM, DIRECTIONS};
use crate::table::{flag_bits, BlockStateLightTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkyViolationKind {
    TopRowFloor,
    SupportFloor,
    SourceExcess,
}

#[derive(Debug, Clone, Copy)]
pub struct SkyInvariantViolation {
    pub cell: BlockPos,
    pub stored: u8,
    pub max_support: u8,
    pub kind: SkyViolationKind,
}

impl std::fmt::Display for SkyInvariantViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SkyInvariantViolation {{ kind: {:?}, cell: {:?}, stored: {}, max_support: {} }}",
            self.kind, self.cell, self.stored, self.max_support
        )
    }
}

/// Sky-aware inward contribution from `neighbour` to `(x,y,z)` along
/// direction `d` (which steps from the cell towards the neighbour). Mirrors
/// the BFS vertical-drop rule: when the neighbour sits directly above self,
/// the neighbour's level is 15, and self has `PROPAGATES_SKYLIGHT_DOWN`, the
/// downward BFS step propagates 15 unattenuated. All other directions fall
/// back to the unified `parent - max(1, dampening_of_self)` attenuation used
/// by block light.
fn sky_neighbour_contribution(
    d: Direction,
    x: i32,
    y: i32,
    z: i32,
    self_state: BlockStateId,
    table: &BlockStateLightTable,
    palette: &BlockPalette,
    light: &LightStorage,
) -> Option<u8> {
    let (dx, dy, dz) = direction_offset(d);
    let nx = x + dx;
    let ny = y + dy;
    let nz = z + dz;
    if !(0..CHUNK_DIM).contains(&nx)
        || !(0..CHUNK_DIM).contains(&ny)
        || !(0..CHUNK_DIM).contains(&nz)
    {
        return None;
    }

    let neighbour_state = palette.get(BlockPos::new(nx, ny, nz));
    let neighbour_level = light.get(nx as usize, ny as usize, nz as usize);

    let combined_flags = table.flags_for(self_state) | table.flags_for(neighbour_state);
    let face_blocks = if combined_flags & flag_bits::IS_CONDITIONALLY_OPAQUE != 0 {
        let from_shape = table
            .occlusion_for(neighbour_state)
            .face_shape(d.opposite());
        let to_shape = table.occlusion_for(self_state).face_shape(d);
        from_shape.face_occludes(to_shape, d.opposite())
    } else {
        false
    };

    if face_blocks {
        return Some(0);
    }

    // The vertical-drop rule fires when the BFS would step downward from the
    // neighbour to self. In this helper's frame `d` walks from self toward
    // the neighbour, so neighbour-above corresponds to `Direction::Up`.
    if d == Direction::Up
        && neighbour_level == 15
        && (table.flags_for(self_state) & flag_bits::PROPAGATES_SKYLIGHT_DOWN) != 0
    {
        return Some(15);
    }

    let dampening = table.dampening_for(self_state).max(1);
    Some(neighbour_level.saturating_sub(dampening))
}

pub fn check_sky_light_invariants(
    table: &BlockStateLightTable,
    palette: &BlockPalette,
    light: &LightStorage,
    is_topmost_in_skyhaving_column: bool,
) -> Result<(), SkyInvariantViolation> {
    for y in 0..CHUNK_DIM {
        for z in 0..CHUNK_DIM {
            for x in 0..CHUNK_DIM {
                let state = palette.get(BlockPos::new(x, y, z));
                let stored = light.get(x as usize, y as usize, z as usize);
                let cell = BlockPos::new(x, y, z);

                if y == 15
                    && is_topmost_in_skyhaving_column
                    && (table.flags_for(state) & flag_bits::PROPAGATES_SKYLIGHT_DOWN) != 0
                    && stored != 15
                {
                    return Err(SkyInvariantViolation {
                        cell,
                        stored,
                        max_support: 15,
                        kind: SkyViolationKind::TopRowFloor,
                    });
                }

                let mut max_inward_support: u8 = 0;
                for d in DIRECTIONS {
                    if let Some(contribution) =
                        sky_neighbour_contribution(d, x, y, z, state, table, palette, light)
                        && contribution > max_inward_support
                    {
                        max_inward_support = contribution;
                    }
                }

                if stored < max_inward_support {
                    return Err(SkyInvariantViolation {
                        cell,
                        stored,
                        max_support: max_inward_support,
                        kind: SkyViolationKind::SupportFloor,
                    });
                }

                let is_top_row_source = y == 15
                    && is_topmost_in_skyhaving_column
                    && (table.flags_for(state) & flag_bits::PROPAGATES_SKYLIGHT_DOWN) != 0;
                if stored > max_inward_support && !is_top_row_source {
                    return Err(SkyInvariantViolation {
                        cell,
                        stored,
                        max_support: max_inward_support,
                        kind: SkyViolationKind::SourceExcess,
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

    const AIR: BlockStateId = BlockStateId(0);

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

    fn air_storage() -> LightStorage {
        LightStorage::Dense(Box::new(LightNibbles::zeros()))
    }

    #[test]
    fn sky_invariants_pass_on_all_air_topmost() {
        let table = make_test_table();
        let palette = make_palette(&[]);
        let light = LightStorage::Uniform(15);
        let result = check_sky_light_invariants(&table, &palette, &light, /* is_topmost */ true);
        assert!(
            result.is_ok(),
            "expected Ok(()) for all-air topmost chunk at uniform 15; got {:?}",
            result
        );
    }

    #[test]
    fn sky_invariants_fail_top_row_floor_below_15() {
        // All-air topmost chunk, but the top-row cell (8, 15, 8) stores 7
        // instead of 15. TopRowFloor must fire on that cell.
        //
        // The column directly under the broken top-row cell is filled with 14
        // rather than 15: with the broken Up neighbour at 7 the vertical-drop
        // rule no longer fires for (8, 14, 8), so the inward support there
        // tops out at 14 (the side cells in the y=14 plane contribute
        // saturating_sub(1) = 14). Holding the column at 14 keeps SourceExcess
        // silent on every cell below the broken top-row entry; TopRowFloor
        // fires first at (8, 15, 8).
        let table = make_test_table();
        let palette = make_palette(&[]);
        let mut light = air_storage();
        for y in 0..16usize {
            for z in 0..16usize {
                for x in 0..16usize {
                    light.set(x, y, z, 15);
                }
            }
        }
        for y in 0..15usize {
            light.set(8, y, 8, 14);
        }
        light.set(8, 15, 8, 7);

        let err = check_sky_light_invariants(&table, &palette, &light, /* is_topmost */ true)
            .expect_err("expected TopRowFloor violation");
        assert_eq!(err.kind, SkyViolationKind::TopRowFloor);
        assert_eq!(err.cell.y, 15);
        assert_eq!(err.stored, 7);
        assert_eq!(err.max_support, 15);
    }

    #[test]
    fn sky_invariants_fail_source_excess() {
        // Build a STONE-only fixture: every cell is solid with dampening=15
        // and no `PROPAGATES_SKYLIGHT_DOWN`. The sky-aware oracle's
        // vertical-drop rule never fires, every neighbour contributes
        // `0.saturating_sub(15) = 0`, and every y=15 cell skips TopRowFloor
        // because the propagates flag is cleared. A single bright stored
        // level at an interior cell then trips SourceExcess.
        const STONE: BlockStateId = BlockStateId(1);
        let state_count: usize = 2;
        let emission = vec![0u8; state_count].into_boxed_slice();
        let mut dampening = vec![0u8; state_count].into_boxed_slice();
        let occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let mut flags = vec![0u8; state_count].into_boxed_slice();
        dampening[STONE.0 as usize] = 15;
        flags[STONE.0 as usize] = flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE;
        let table = BlockStateLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        };

        let mut palette = BlockPalette::default();
        palette.fill(STONE);

        let mut light = air_storage();
        light.set(5, 8, 9, 10);

        let err = check_sky_light_invariants(&table, &palette, &light, /* is_topmost */ true)
            .expect_err("expected SourceExcess violation");
        assert_eq!(err.kind, SkyViolationKind::SourceExcess);
        assert_eq!(err.cell, BlockPos::new(5, 8, 9));
        assert_eq!(err.stored, 10);
        assert_eq!(err.max_support, 0);
    }

    #[test]
    fn sky_invariants_fail_source_excess_at_top_row_when_not_topmost() {
        // Uniform-15 all-air field but the chunk is NOT topmost in its
        // sky-having column. Under the new invariants, cells at y=15 are no
        // longer covered by the top-row exception, and the unified sky-aware
        // oracle attenuates the Down-neighbour at y=14 to 14 (the
        // vertical-drop rule fires only on the Up neighbour, which is
        // out-of-chunk at y=15). SourceExcess fires on (0, 15, 0).
        let table = make_test_table();
        let palette = make_palette(&[]);
        let light = LightStorage::Uniform(15);
        let err = check_sky_light_invariants(&table, &palette, &light, /* is_topmost */ false)
            .expect_err("expected SourceExcess violation");
        assert_eq!(err.kind, SkyViolationKind::SourceExcess);
        assert_eq!(err.cell, BlockPos::new(0, 15, 0));
        assert_eq!(err.stored, 15);
        assert_eq!(err.max_support, 14);
    }
}
