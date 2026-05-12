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
use mcrs_minecraft::world::palette::BlockPalette;
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
    if nx < 0 || nx >= SECTION_DIM || ny < 0 || ny >= SECTION_DIM || nz < 0 || nz >= SECTION_DIM {
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
                    {
                        if contribution > max_inward_support {
                            max_inward_support = contribution;
                        }
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
