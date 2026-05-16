// Heightmap Y-storage convention: each entry stores `Y + 1` of the topmost
// cell satisfying the predicate (the empty cell on top of the topmost solid /
// motion-blocking cell). An air-only column reads back `min_y` because the
// PackedBitStorage backing is zero-initialized and `surface_get` adds `min_y`
// to the unsigned stored value.
//
// This convention MUST stay identical to `prime_heightmaps_on_column_spawn`
// in `lifecycle.rs`; the two systems write to the same column-level state and
// any drift between them would silently corrupt heightmaps after the first
// place or break.
//
// Early-out predicate: distinguishes places from breaks via `old_state` vs
// `new_state` flags. A place can only raise the stored value if the new state
// satisfies the predicate AND lies strictly above the current surface. A break
// can only lower the stored value if the old state satisfied the predicate AND
// the broken cell is the one currently recorded as the topmost satisfying cell
// (i.e. `placed_y + 1 == current_surface`). When neither condition holds for
// both heightmaps, the rescan is skipped. The earlier `y + 2 <= current_height`
// form ignored old/new states and mishandled breaks of the topmost cell.
//
// Concurrency: `Query<&mut Heightmaps>` plus a separate `Query<&BlockPalette>`
// give the scheduler exclusive write access to heightmap state for the
// duration of the system; no manual locking is needed.
use crate::heightmap::{scan_top_down, HeightmapVariant};
use crate::table::{flag_bits, BlockLightTable};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Entity, Query, Res};
use mcrs_engine::world::column::{Heightmaps, InColumn, ColumnChunks};
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;

/// HEIGHT-02 eager fused two-type heightmap update. Reads
/// `MessageReader<BlockPlaced>` and updates `Heightmaps` on the affected
/// `Column`. Applies the `y + 2 <= current_height` early-out per type;
/// falls back to a single top-down rescan when the early-out fails.
///
/// Runs in `FixedUpdate` with `.after(apply_set_block_request)` so the
/// `MessageReader<BlockPlaced>` sees this tick's writes; the
/// `FixedUpdate -> FixedPostUpdate` schedule boundary provides ordering
/// against `update_client_blocks` so downstream codec reads in
/// `FixedPostUpdate` observe up-to-date heightmap state.
pub fn update_heightmaps_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    chunks: Query<&InColumn>,
    mut columns: Query<(&mut Heightmaps, &ColumnChunks)>,
    palettes: Query<&BlockPalette>,
    table: Res<BlockLightTable>,
) {
    for placed in reader.read() {
        let Ok(in_column) = chunks.get(placed.chunk) else {
            continue;
        };
        let col_entity = in_column.0;
        let Ok((mut heightmaps, chunk_index)) = columns.get_mut(col_entity) else {
            continue;
        };

        let x = (placed.block_pos.x & 15) as usize;
        let z = (placed.block_pos.z & 15) as usize;
        let placed_y = placed.block_pos.y;

        let min_y = heightmaps.min_y();
        let max_y = min_y + heightmaps.height() as i32 - 1;
        if placed_y < min_y || placed_y > max_y {
            tracing::warn!(
                block_pos = ?placed.block_pos,
                min_y,
                max_y,
                "BlockPlaced outside dimension Y; ignored by heightmap"
            );
            continue;
        }

        let current_surface = heightmaps.surface_get(x, z);
        let current_motion = heightmaps.motion_blocking_get(x, z);

        let old_flags = table.flags_for(placed.old_state);
        let new_flags = table.flags_for(placed.new_state);
        let old_was_surface = (old_flags & flag_bits::IS_NOT_AIR) != 0;
        let new_is_surface = (new_flags & flag_bits::IS_NOT_AIR) != 0;
        let old_was_motion = (old_flags & flag_bits::IS_MOTION_BLOCKING) != 0;
        let new_is_motion = (new_flags & flag_bits::IS_MOTION_BLOCKING) != 0;

        let placed_y_plus_one = placed_y + 1;
        let surface_could_raise = new_is_surface && placed_y_plus_one > current_surface;
        let surface_could_lower = old_was_surface && placed_y_plus_one >= current_surface;
        let motion_could_raise = new_is_motion && placed_y_plus_one > current_motion;
        let motion_could_lower = old_was_motion && placed_y_plus_one >= current_motion;

        if !surface_could_raise
            && !surface_could_lower
            && !motion_could_raise
            && !motion_could_lower
        {
            continue;
        }

        let (new_surface, new_motion) =
            rescan_column_xz(chunk_index, &palettes, &table, x, z, min_y);
        heightmaps.surface_set(x, z, new_surface);
        heightmaps.motion_blocking_set(x, z, new_motion);
    }
}

fn rescan_column_xz(
    chunk_index: &ColumnChunks,
    palettes: &Query<&BlockPalette>,
    table: &BlockLightTable,
    x: usize,
    z: usize,
    min_y: i32,
) -> (i32, i32) {
    let mut world_surface_raw: Option<i32> = None;
    let mut motion_blocking_raw: Option<i32> = None;

    let palette_fn = |entity: Entity| -> Option<&BlockPalette> { palettes.get(entity).ok() };

    let xz = [(x, z)];
    let mut cursor = chunk_index.min_section_y + chunk_index.sections.len() as i32 - 1;

    let _ = scan_top_down(
        chunk_index,
        palette_fn,
        table,
        &xz,
        &mut cursor,
        |_x, _z, variant, world_y| match variant {
            HeightmapVariant::Surface => world_surface_raw = Some(world_y),
            HeightmapVariant::MotionBlocking => motion_blocking_raw = Some(world_y),
        },
    );

    (
        world_surface_raw.map(|y| y + 1).unwrap_or(min_y),
        motion_blocking_raw.map(|y| y + 1).unwrap_or(min_y),
    )
}
