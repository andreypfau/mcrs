#![cfg(any(test, debug_assertions))]

//! Channel-shared invariant helpers used by both block and sky checkers.
//!
//! The per-channel `check_*_light_invariants` entry points live under
//! `crate::block_light::invariants` and `crate::sky_light::invariants` and
//! call into these helpers for the iteration scaffold (`CHUNK_DIM`,
//! `DIRECTIONS`, `direction_offset`) and the block-flavoured inward
//! contribution oracle (`neighbour_contribution`). The two violation types
//! (`InvariantViolation`, `ViolationKind`) live here because they originate
//! in the shared scaffold even though only the block checker emits them
//! today; keeping them shared avoids a third copy when a future
//! cross-channel checker fires.
//!
//! Gated under `#[cfg(any(test, debug_assertions))]` because the check is
//! only ever called from tests or from a debug-only verification system —
//! production release builds compile it away entirely.

use crate::storage::LightStorage;
use crate::table::{flag_bits, BlockStateLightTable};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::geometry::chunk_pos::BLOCKS;
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

pub(crate) const CHUNK_DIM: i32 = BLOCKS::SIZE as i32;

pub(crate) const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

#[inline]
pub(crate) fn direction_offset(d: Direction) -> (i32, i32, i32) {
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
/// the neighbour is outside the chunk — the single-chunk checker
/// cannot resolve cross-chunk state and so the outward face is skipped.
pub(crate) fn neighbour_contribution(
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
