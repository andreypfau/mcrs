//! Typed helpers around `mcrs_engine::world::column::Heightmaps` plus the
//! shared top-down scanner core consumed by `lifecycle::advance_scan` and
//! `heightmap_update::rescan_column_xz`.
//!
//! `Heightmaps` stores each entry as `Y + 1` of the topmost cell satisfying
//! the predicate — the empty cell on top of the topmost solid / motion-blocking
//! cell — encoded as an unsigned offset from the dimension `min_y`. An air-only
//! (unsurfaced) column reads back as `hm.min_y()` because the `PackedBitStorage`
//! backing is zero-initialized and `surface_get` adds `min_y` to the unsigned
//! stored value.
//!
//! The helpers below are the single canonical entry point for that convention.
//! Raw `surface_set` / `motion_blocking_set` / `surface_get` /
//! `motion_blocking_get` callers inside this crate must go through the
//! helpers so the `+ 1` arithmetic and the `min_y` sentinel handling live in
//! exactly one place.

use bevy_ecs::prelude::Entity;
use mcrs_engine::world::column::ColumnChunks;
use mcrs_minecraft_block::palette::BlockPalette;

use crate::bitset::BitSet256;
use crate::table::{flag_bits, BlockStateLightTable};
use mcrs_engine::world::column::Heightmaps;

const CHUNK_SIZE: i32 = 16;

/// Which heightmap variant a helper operates on. Used by the
/// [`record_topmost`] dispatcher when the caller already knows the variant
/// dynamically — for example, the shared top-down scanner core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeightmapVariant {
    Surface,
    MotionBlocking,
}

/// Record the topmost solid cell of the `(x, z)` column at `world_y` into
/// the world-surface heightmap. The helper applies the `+ 1` storage offset.
#[inline]
pub fn record_topmost_surface(hm: &mut Heightmaps, x: usize, z: usize, world_y: i32) {
    hm.surface_set(x, z, world_y + 1);
}

/// Read the world Y of the topmost solid cell of the `(x, z)` column.
/// Returns `None` for an unsurfaced (air-only) column.
#[inline]
pub fn topmost_surface_world_y(hm: &Heightmaps, x: usize, z: usize) -> Option<i32> {
    let stored = hm.surface_get(x, z);
    if stored == hm.min_y() {
        None
    } else {
        Some(stored - 1)
    }
}

/// Mark the `(x, z)` column as unsurfaced in the world-surface heightmap by
/// writing the `min_y` sentinel.
#[inline]
pub fn record_unsurfaced_column(hm: &mut Heightmaps, x: usize, z: usize) {
    let min_y = hm.min_y();
    hm.surface_set(x, z, min_y);
}

/// Mirror of [`record_topmost_surface`] for the motion-blocking heightmap.
#[inline]
pub fn record_topmost_motion_blocking(hm: &mut Heightmaps, x: usize, z: usize, world_y: i32) {
    hm.motion_blocking_set(x, z, world_y + 1);
}

/// Mirror of [`topmost_surface_world_y`] for the motion-blocking heightmap.
#[inline]
pub fn topmost_motion_blocking_world_y(hm: &Heightmaps, x: usize, z: usize) -> Option<i32> {
    let stored = hm.motion_blocking_get(x, z);
    if stored == hm.min_y() {
        None
    } else {
        Some(stored - 1)
    }
}

/// Mirror of [`record_unsurfaced_column`] for the motion-blocking heightmap.
#[inline]
pub fn record_unsurfaced_motion_column(hm: &mut Heightmaps, x: usize, z: usize) {
    let min_y = hm.min_y();
    hm.motion_blocking_set(x, z, min_y);
}

/// Variant-dispatching wrapper. Consumed by callers that walk a column
/// once and write into either heightmap variant per cell.
#[inline]
pub fn record_topmost(
    hm: &mut Heightmaps,
    variant: HeightmapVariant,
    x: usize,
    z: usize,
    world_y: i32,
) {
    match variant {
        HeightmapVariant::Surface => record_topmost_surface(hm, x, z, world_y),
        HeightmapVariant::MotionBlocking => record_topmost_motion_blocking(hm, x, z, world_y),
    }
}

/// Outcome of a top-down scan over a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanOutcome {
    /// Every `(x, z)` in the scan range closed for both heightmap variants.
    AllClosed,
    /// The scan walked past the dimension floor with at least one `(x, z)`
    /// still open. The unsurfaced cells are the responsibility of the
    /// caller's wrapper (it writes the `min_y` sentinel for them).
    ChimneyToBedrock,
    /// A chunk slot inside the cursor range was not loaded yet. The scan
    /// returns immediately so the caller can wait for the next
    /// `Changed<ColumnChunks>` event before resuming.
    AbsentSection,
}

/// Shared top-down heightmap scanner. Walks `chunks` from the slot at
/// `cursor` downward, evaluating `table.flags_for(palette.get(cell))` for
/// every `(x, z)` in `xz_range` at every cell of every loaded chunk. Fires
/// `on_closed(x, z, variant, world_y)` exactly once per `(x, z, variant)`
/// pair when the predicate first matches — the world Y of the topmost solid
/// (or motion-blocking) cell. Updates `*cursor` in place; the post-scan
/// value is the slot one below the last chunk processed, mirroring the
/// pre-refactor `advance_scan` cursor semantics.
///
/// `palette_fn` returns `None` for chunk entities the caller wants to skip
/// without halting the scan — used by `advance_scan` to fast-skip `IsAllAir`
/// chunks. A `None` from `palette_fn` is distinct from an absent slot in
/// `chunks.sections`: the former advances the cursor and continues, the
/// latter returns `ScanOutcome::AbsentSection` immediately.
#[inline]
pub fn scan_top_down<'a, P, F>(
    chunks: &ColumnChunks,
    palette_fn: P,
    table: &BlockStateLightTable,
    xz_range: &[(usize, usize)],
    cursor: &mut i32,
    mut on_closed: F,
) -> ScanOutcome
where
    P: Fn(Entity) -> Option<&'a BlockPalette>,
    F: FnMut(usize, usize, HeightmapVariant, i32),
{
    let mut surface_done = BitSet256::default();
    let mut motion_done = BitSet256::default();
    let mut surface_closed: usize = 0;
    let mut motion_closed: usize = 0;
    let total = xz_range.len();

    loop {
        if *cursor < chunks.min_section_y {
            return ScanOutcome::ChimneyToBedrock;
        }

        let rel_y = (*cursor - chunks.min_section_y) as usize;
        let Some(slot) = chunks.sections.get(rel_y) else {
            return ScanOutcome::AbsentSection;
        };
        let Some(chunk_entity) = slot else {
            return ScanOutcome::AbsentSection;
        };

        let chunk_base_y = *cursor * CHUNK_SIZE;
        let palette = match palette_fn(*chunk_entity) {
            Some(p) => p,
            None => {
                // Fast-skip: chunk's palette declined (e.g., IsAllAir). Advance
                // and continue. Note: rescan_column_xz's palette_fn returns
                // None only for missing-component errors, which it treats as
                // a transient gap; for those cases, the wrapper's intent
                // matches the lifecycle wrapper's IsAllAir fast-skip semantics
                // closely enough that a continue is correct (the column still
                // gets the same final result).
                *cursor -= 1;
                continue;
            }
        };

        'outer: for cell_y in (0..CHUNK_SIZE).rev() {
            for (xz_idx, &(x, z)) in xz_range.iter().enumerate() {
                let bit_idx = if total == 256 {
                    (z << 4) | x
                } else {
                    xz_idx
                };
                let s_open = !surface_done.is_set(bit_idx);
                let m_open = !motion_done.is_set(bit_idx);
                if !s_open && !m_open {
                    continue;
                }
                let state = palette.get((x as i32, cell_y, z as i32));
                let flags = table.flags_for(state);
                let world_y = chunk_base_y + cell_y;

                if s_open && (flags & flag_bits::IS_NOT_AIR) != 0 {
                    on_closed(x, z, HeightmapVariant::Surface, world_y);
                    surface_done.set(bit_idx);
                    surface_closed += 1;
                }
                if m_open && (flags & flag_bits::IS_MOTION_BLOCKING) != 0 {
                    on_closed(x, z, HeightmapVariant::MotionBlocking, world_y);
                    motion_done.set(bit_idx);
                    motion_closed += 1;
                }
                if surface_closed == total && motion_closed == total {
                    break 'outer;
                }
            }
        }

        *cursor -= 1;

        if surface_closed == total && motion_closed == total {
            return ScanOutcome::AllClosed;
        }
    }
}
