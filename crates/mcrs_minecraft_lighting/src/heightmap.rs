//! Typed helpers around `mcrs_engine::world::column::Heightmaps`.
//!
//! `Heightmaps` stores each entry as `Y + 1` of the topmost cell satisfying
//! the predicate — the empty cell on top of the topmost solid / motion-blocking
//! cell — encoded as an unsigned offset from the dimension `min_y`. An air-only
//! (unsurfaced) column reads back as `hm.min_y()` because the `PackedBitStorage`
//! backing is zero-initialized and `surface_get` adds `min_y` to the unsigned
//! stored value.
//!
//! These helpers are the single canonical entry point for that convention.
//! Raw `surface_set` / `motion_blocking_set` / `surface_get` /
//! `motion_blocking_get` callers inside this crate must go through the
//! helpers below so the `+ 1` arithmetic and the `min_y` sentinel handling
//! live in exactly one place.

use mcrs_engine::world::column::Heightmaps;

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
