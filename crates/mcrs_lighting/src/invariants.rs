#![cfg(any(test, debug_assertions))]

//! Per-cell block-light invariant checker for a single `ChunkSection`.
//!
//! Verifies three invariants for every cell in the 16×16×16 section:
//!
//! 1. **Source floor:** the stored level is at least the cell's own emission.
//! 2. **Support floor:** the stored level is at least the maximum inward
//!    contribution from the six cardinal neighbours, where each neighbour
//!    contributes `neighbour_level - max(1, dampening_of_self)` unless the
//!    cell-cell face is fully occluded.
//! 3. **Source-only excess:** when the stored level exceeds the inward
//!    support, the excess equals the cell's own emission — i.e. only
//!    emitter cells may be brighter than the surrounding field supports.
//!
//! Single-section variant: cells on the section boundary (any of x/y/z is 0
//! or 15) skip the support-floor contribution from the outward-facing
//! neighbour because that neighbour lives in a sibling section the checker
//! cannot resolve from intra-section state alone.
//!
//! Gated under `#[cfg(any(test, debug_assertions))]` because the check is
//! only ever called from tests or from a debug-only verification system —
//! production release builds compile it away entirely.

use crate::storage::LightStorage;
use crate::table::{flag_bits, BlockLightTable};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    SourceFloor,
    SupportFloor,
    SourceExcess,
}

#[derive(Debug, Clone, Copy)]
pub struct InvariantViolation {
    pub cell: BlockPos,
    pub stored: u8,
    pub emitted: u8,
    pub max_support: u8,
    pub kind: ViolationKind,
}

impl std::fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InvariantViolation {{ kind: {:?}, cell: {:?}, stored: {}, emitted: {}, max_support: {} }}",
            self.kind, self.cell, self.stored, self.emitted, self.max_support
        )
    }
}

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

const SECTION_DIM: i32 = 16;

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

#[inline]
fn direction_offset(d: Direction) -> (i32, i32, i32) {
    match d {
        Direction::Down => (0, -1, 0),
        Direction::Up => (0, 1, 0),
        Direction::North => (0, 0, -1),
        Direction::South => (0, 0, 1),
        Direction::West => (-1, 0, 0),
        Direction::East => (1, 0, 0),
    }
}

/// Inward contribution from `neighbour` to `(x,y,z)` along direction `d`
/// (which steps from the cell towards the neighbour). Returns `None` when
/// the neighbour is outside the section — the single-section checker
/// cannot resolve cross-section state and so the outward face is skipped.
fn neighbour_contribution(
    d: Direction,
    x: i32,
    y: i32,
    z: i32,
    self_state: BlockStateId,
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &LightStorage,
) -> Option<u8> {
    let (dx, dy, dz) = direction_offset(d);
    let nx = x + dx;
    let ny = y + dy;
    let nz = z + dz;
    if !(0..SECTION_DIM).contains(&nx)
        || !(0..SECTION_DIM).contains(&ny)
        || !(0..SECTION_DIM).contains(&nz)
    {
        return None;
    }

    let neighbour_state = palette.get(BlockPos::new(nx, ny, nz));
    let neighbour_level = light.get(nx as usize, ny as usize, nz as usize);
    let dampening = table.dampening_for(self_state).max(1);

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
        Some(0)
    } else {
        Some(neighbour_level.saturating_sub(dampening))
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
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &LightStorage,
) -> Option<u8> {
    let (dx, dy, dz) = direction_offset(d);
    let nx = x + dx;
    let ny = y + dy;
    let nz = z + dz;
    if !(0..SECTION_DIM).contains(&nx)
        || !(0..SECTION_DIM).contains(&ny)
        || !(0..SECTION_DIM).contains(&nz)
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

pub fn check_block_light_invariants(
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &LightStorage,
) -> Result<(), InvariantViolation> {
    for y in 0..SECTION_DIM {
        for z in 0..SECTION_DIM {
            for x in 0..SECTION_DIM {
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

/// Per-cell sky-light invariant checker for a single `ChunkSection`.
///
/// Enforces three invariants:
/// - `TopRowFloor`: when the section is the topmost of a sky-having column,
///   every y=15 air cell (`PROPAGATES_SKYLIGHT_DOWN`) must store the sky
///   maximum (15).
/// - `SupportFloor`: every cell's stored level is at least the maximum
///   sky-aware inward contribution from its six cardinal neighbours.
/// - `SourceExcess`: no cell may exceed its inward support, except for the
///   top-row air cells of a topmost-of-column section which receive their
///   light from the open-sky source above the section.
///
/// Inward contributions are computed against the sky-aware oracle, which
/// mirrors the BFS vertical-drop rule: a downward step from a level-15 cell
/// into a destination with `PROPAGATES_SKYLIGHT_DOWN` propagates 15
/// unattenuated rather than the unified attenuation.
///
/// The `is_topmost_in_skyhaving_column` flag is passed by the caller rather
/// than derived from ECS state so the checker stays a pure function reachable
/// from both unit tests and a debug-only verification system.
pub fn check_sky_light_invariants(
    table: &BlockLightTable,
    palette: &BlockPalette,
    light: &LightStorage,
    is_topmost_in_skyhaving_column: bool,
) -> Result<(), SkyInvariantViolation> {
    for y in 0..SECTION_DIM {
        for z in 0..SECTION_DIM {
            for x in 0..SECTION_DIM {
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
    use crate::nibble::NibbleArray;
    use crate::table::flag_bits;
    use mcrs_core::voxel_shape::VoxelShape;

    const AIR: BlockStateId = BlockStateId(0);
    const TORCH: BlockStateId = BlockStateId(0x1000);

    fn make_test_table() -> BlockLightTable {
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

        BlockLightTable {
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

    /// Returns a `LightStorage::Mixed` backed by a zeroed `NibbleArray`. The
    /// test helpers use this rather than the `Null` default so that
    /// `LightStorage::set` does not promote the storage to `Uniform(v)` on
    /// the first nonzero write (which would clobber every other cell with
    /// the same value and mask the per-cell behaviour under test).
    fn air_storage() -> LightStorage {
        LightStorage::Mixed(Box::new(NibbleArray::zeros()))
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
        // The torch sits at (0,0,0) so the y/z/x-major iteration hits the
        // under-lit emitter cell before any neighbour can trip a different
        // invariant first.
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
        // Place a torch at (0,0,0) so iteration hits the emitter first and
        // passes its source/support/excess checks before moving on to
        // (1,0,0). The next cell sees the torch as a 14-level inward
        // neighbour but its own stored level is 0, tripping SupportFloor.
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
        // Air palette with one cell at the iteration origin lit to 14. The
        // origin has emitted=0 and max_support=0 (all neighbours are 0), so
        // the bright stored level cannot be justified by either an emitter
        // or by inward support and SourceExcess fires.
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
        // neighbours in a sibling section, so the support-floor check must
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

    #[test]
    fn sky_invariants_pass_on_all_air_topmost() {
        // All-air section, topmost in a sky-having column, every cell at
        // level 15. TopRowFloor passes because every y=15 cell stores 15.
        // SupportFloor passes because every interior cell sees an above
        // neighbour at level 15 and, via the BFS vertical-drop rule, the
        // sky-aware oracle returns 15 unattenuated through air. SourceExcess
        // passes because every cell's stored level equals its inward support
        // (the only y=15 source cells fall under the top-row exception).
        let table = make_test_table();
        let palette = make_palette(&[]);
        let light = LightStorage::Uniform(15);
        let result = check_sky_light_invariants(&table, &palette, &light, /* is_topmost */ true);
        assert!(
            result.is_ok(),
            "expected Ok(()) for all-air topmost section at uniform 15; got {:?}",
            result
        );
    }

    #[test]
    fn sky_invariants_fail_top_row_floor_below_15() {
        // All-air topmost section, but the top-row cell (8, 15, 8) stores 7
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
        let table = BlockLightTable {
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
        // Uniform-15 all-air field but the section is NOT topmost in its
        // sky-having column. Under the new invariants, cells at y=15 are no
        // longer covered by the top-row exception, and the unified sky-aware
        // oracle attenuates the Down-neighbour at y=14 to 14 (the
        // vertical-drop rule fires only on the Up neighbour, which is
        // out-of-section at y=15). SourceExcess fires on (0, 15, 0).
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
