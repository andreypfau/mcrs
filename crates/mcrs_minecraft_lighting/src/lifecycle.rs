// Heightmaps zero-init convention: `Heightmaps::new(height)` and
// `Heightmaps::with_min_y(height, min_y)` zero-initialize the backing
// `PackedBitStorage` long arrays, so `surface_get(x, z) == min_y` for any
// unprimed (x, z) until this system overwrites with the real top-down-scan
// result. Downstream eager-update code MUST use `min_y` as the
// "no surface found" sentinel to stay deterministic.

use crate::bitset::BitSet256;
use crate::bundle::{BlockLightBundle, SkyLightBundle};
use crate::components::{ChunkNeedsInitialLight, IsAllAir};
use crate::table::{flag_bits, BlockLightTable};
use bevy_ecs::prelude::{Added, Changed, Commands, Component, Entity, Has, Query, Res, With};
use mcrs_engine::world::chunk::{ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{Column, ColumnChunks, Heightmaps};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
use mcrs_minecraft_block::palette::BlockPalette;

const SECTION_SIZE: i32 = 16;

#[inline]
const fn xz_idx(x: usize, z: usize) -> usize {
    debug_assert!(x < 16 && z < 16);
    (z << 4) | x
}

#[inline]
const fn idx_to_xz(idx: usize) -> (usize, usize) {
    debug_assert!(idx < 256);
    (idx & 15, idx >> 4)
}

/// Per-column state for the incremental top-down heightmap scan.
///
/// The scan walks section slots from `max_section_y` downward, stopping at
/// the first absent slot. As it walks, it writes `Heightmaps::surface_set` /
/// `Heightmaps::motion_blocking_set` the moment it closes each XZ column
/// for that variant, and records closure in a per-variant 256-bit bitset.
/// Finalization is fully derived (`is_finalized()`); the predicate fires
/// either when the cursor drops below `min_section_y` (chimney-to-bedrock)
/// or both variants have closed every XZ column.
///
/// Why the bitset and not "compare `hm.surface_get` against `min_y`"?
/// At chimney-to-bedrock finalization the heightmap legitimately holds
/// `min_y` for the all-air columns, indistinguishable from "not yet
/// closed". The bitset is the unambiguous source of truth for closure.
///
/// Partial-column contract: until `is_finalized()` returns true, entries
/// for XZ columns the scan has not yet closed hold the sentinel value
/// equal to `min_y`. Callers that need accurate heightmap values for
/// partial columns must check `is_finalized()` before trusting
/// `surface_get` / `motion_blocking_get`.
/// `update_heightmaps_on_block_placed` is the authoritative writer for
/// post-finalization edits — its rescan path tolerates sentinel reads and
/// writes the correct value regardless of scan state.
///
/// Total state size: 4 (cursor) + 4 (min_y) + 32 + 32 (bitsets) = 72 bytes.
#[derive(Component, Debug)]
pub struct ColumnHeightmapScan {
    /// Section Y coordinate to process next. Decrements as sections are scanned.
    /// Initial value: `max_section_y` (the topmost section).
    pub scan_cursor: i32,
    min_section_y: i32,
    world_surface_done: BitSet256,
    motion_blocking_done: BitSet256,
}

impl ColumnHeightmapScan {
    /// Create a fresh scan state with explicit section-y bounds. Caller
    /// passes both directly from the column's `ColumnChunks` / dimension
    /// metadata; no off-by-one arithmetic happens here.
    #[inline]
    pub fn new(min_section_y: i32, max_section_y: i32) -> Self {
        debug_assert!(max_section_y >= min_section_y);
        Self {
            scan_cursor: max_section_y,
            min_section_y,
            world_surface_done: BitSet256::default(),
            motion_blocking_done: BitSet256::default(),
        }
    }

    /// Finalization predicate. True once the cursor has dropped below the
    /// floor (chimney-to-bedrock path completed) or both heightmap variants
    /// have closed every XZ column.
    #[inline]
    pub fn is_finalized(&self) -> bool {
        self.scan_cursor < self.min_section_y
            || (self.world_surface_done.is_full() && self.motion_blocking_done.is_full())
    }
}

/// Stage 2.5 of the chunk-column lifecycle.
///
/// Runs a single stateful top-down scan per column that simultaneously
/// writes the heightmap and tracks the lighting gate. On every
/// `Changed<ColumnChunks>` event, advances the per-column
/// `ColumnHeightmapScan` as far as the currently-loaded sections allow.
/// The scan writes `Heightmaps::{surface,motion_blocking}_set` the moment
/// it closes each XZ column for that variant. When both variants have
/// closed every XZ column — or the cursor drops below `min_section_y`
/// (chimney-to-bedrock) — `is_finalized()` flips to true and
/// `ChunkNeedsInitialLight` is inserted on every currently-loaded section.
///
/// Late-arrival path: once the scan is finalized, any newly-registered
/// section receives `ChunkNeedsInitialLight` immediately.
///
/// `IsAllAir` is a fast-skip: sections where every block is air-equivalent
/// cannot contribute a qualifying block. The cursor advances past such
/// sections without per-block work.
///
/// Pitfall #1 safety: this system lives in `mcrs_minecraft_lighting`
/// because it consumes `BlockLightTable`. The engine crate stays free of
/// any lighting-side imports. Runs after `ColumnLifecycleSet::ReconcileIndex`
/// (Stage 2) so the column's `ColumnChunks` is fully populated for the
/// sections that triggered the column spawn.
pub fn prime_heightmaps_on_column_spawn(
    changed_columns: Query<(Entity, &ColumnChunks), (With<Column>, Changed<ColumnChunks>)>,
    sections: Query<(&BlockPalette, Has<IsAllAir>)>,
    mut col_state: Query<(&mut Heightmaps, Option<&mut ColumnHeightmapScan>)>,
    table: Res<BlockLightTable>,
    mut commands: Commands,
) {
    for (column_entity, section_index) in changed_columns.iter() {
        let Ok((mut hm, scan_opt)) = col_state.get_mut(column_entity) else {
            continue;
        };

        let min_section_y = section_index.min_section_y;
        let section_count = section_index.sections.len();
        let max_section_y = min_section_y + section_count as i32 - 1;

        if let Some(mut scan) = scan_opt {
            if scan.is_finalized() {
                // Late-arrival: insert ChunkNeedsInitialLight on any section
                // slot present. The Changed event fired because a new section
                // just landed; the older sections already have the marker
                // (consumed or pending) so the re-insert is a no-op for them.
                for slot in section_index.sections.iter() {
                    let Some(section_entity) = slot else { continue };
                    commands
                        .entity(*section_entity)
                        .insert(ChunkNeedsInitialLight);
                }
                continue;
            }

            advance_scan(&mut scan, &mut hm, section_index, &sections, &table, &mut commands);
        } else {
            // First observation of this column: init the scan state and
            // attempt to advance it in the same tick.
            let mut scan = ColumnHeightmapScan::new(min_section_y, max_section_y);
            advance_scan(&mut scan, &mut hm, section_index, &sections, &table, &mut commands);
            commands.entity(column_entity).insert(scan);
        }
    }
}

/// Advance the scan cursor as far as possible given the currently-loaded
/// sections, writing the heightmap for each XZ column the moment it closes
/// for that variant. When `is_finalized()` would return true, finishes by
/// inserting `ChunkNeedsInitialLight` on every currently-loaded section.
///
/// Invariant: callers must not invoke `advance_scan` on an already-finalized
/// scan. The outer system takes the late-arrival path instead.
fn advance_scan(
    scan: &mut ColumnHeightmapScan,
    hm: &mut Heightmaps,
    section_index: &ColumnChunks,
    sections: &Query<(&BlockPalette, Has<IsAllAir>)>,
    table: &BlockLightTable,
    commands: &mut Commands,
) {
    debug_assert!(!scan.is_finalized());
    let min_section_y = scan.min_section_y;
    let min_y = hm.min_y();

    loop {
        if scan.scan_cursor < min_section_y {
            // Chimney-to-bedrock: every still-open XZ column has its
            // heightmap entry set to min_y for both variants. The backing
            // storage already reads back as min_y (sentinel), but writing
            // explicitly here keeps post-finalization callers from observing
            // a transient inconsistency if the implementation of `Heightmaps`
            // ever changes its zero-init contract.
            for idx in 0..256 {
                let (x, z) = idx_to_xz(idx);
                if !scan.world_surface_done.is_set(idx) {
                    hm.surface_set(x, z, min_y);
                }
                if !scan.motion_blocking_done.is_set(idx) {
                    hm.motion_blocking_set(x, z, min_y);
                }
            }
            insert_initial_light_markers(section_index, commands);
            return;
        }

        let rel_y = (scan.scan_cursor - min_section_y) as usize;
        let Some(Some(section_entity)) = section_index.sections.get(rel_y) else {
            // Section not yet loaded — stop here and wait.
            return;
        };

        let Ok((palette, is_all_air)) = sections.get(*section_entity) else {
            // Section present but palette not accessible; treat like a gap.
            return;
        };

        if is_all_air {
            scan.scan_cursor -= 1;
            continue;
        }

        let section_base_y = scan.scan_cursor * SECTION_SIZE;

        'outer: for cell_y in (0..SECTION_SIZE).rev() {
            for z in 0..16usize {
                for x in 0..16usize {
                    let idx = xz_idx(x, z);
                    let ws_open = !scan.world_surface_done.is_set(idx);
                    let mb_open = !scan.motion_blocking_done.is_set(idx);
                    if !ws_open && !mb_open {
                        continue;
                    }
                    let state = palette.get((x as i32, cell_y, z as i32));
                    let flags = table.flags_for(state);
                    let world_y = section_base_y + cell_y;

                    if ws_open && (flags & flag_bits::IS_NOT_AIR) != 0 {
                        hm.surface_set(x, z, world_y + 1);
                        scan.world_surface_done.set(idx);
                    }
                    if mb_open && (flags & flag_bits::IS_MOTION_BLOCKING) != 0 {
                        hm.motion_blocking_set(x, z, world_y + 1);
                        scan.motion_blocking_done.set(idx);
                    }
                    if scan.world_surface_done.is_full()
                        && scan.motion_blocking_done.is_full()
                    {
                        break 'outer;
                    }
                }
            }
        }

        scan.scan_cursor -= 1;

        if scan.world_surface_done.is_full() && scan.motion_blocking_done.is_full() {
            insert_initial_light_markers(section_index, commands);
            return;
        }
    }
}

/// Insert `ChunkNeedsInitialLight` on every currently-loaded section in the
/// column. Does not mutate scan state — finalization is read from
/// `is_finalized()`.
fn insert_initial_light_markers(section_index: &ColumnChunks, commands: &mut Commands) {
    for slot in section_index.sections.iter() {
        let Some(section_entity) = slot else { continue };
        commands
            .entity(*section_entity)
            .insert(ChunkNeedsInitialLight);
    }
}

/// Stage 3 of the chunk-column lifecycle. For every newly-loaded
/// `ChunkSection` (sections with `Added<ChunkLoaded>`), attach the per-section
/// lighting bundles.
///
/// `BlockLightBundle` is inserted unconditionally. `SkyLightBundle` is
/// inserted only when the parent `Dimension` carries `HasSkyLight`.
/// `IsAllAir` is inserted when the section's `BlockPalette` contains only
/// air-equivalent states (`emission == 0 && dampening == 0`).
///
/// `ChunkNeedsInitialLight` is NOT inserted here. It is inserted by
/// `prime_heightmaps_on_column_spawn` once the column's heightmap scan
/// finalizes, so that `seed_initial_light` always reads a fully-primed
/// heightmap. Inserting it here (before the scan finalizes) would cause
/// cave-air sections to be seeded with stale heightmap data.
pub fn attach_lighting_state(
    newly_loaded: Query<(Entity, &BlockPalette, &InDimension, &ChunkPos), Added<ChunkLoaded>>,
    sky_dims: Query<(), With<HasSkyLight>>,
    table: Res<BlockLightTable>,
    mut commands: Commands,
) {
    for (section_entity, palette, in_dim, _chunk_pos) in newly_loaded.iter() {
        let mut entity_commands = commands.entity(section_entity);
        entity_commands.insert(BlockLightBundle::default());
        if sky_dims.get(in_dim.0).is_ok() {
            entity_commands.insert(SkyLightBundle::default());
        }

        if is_section_all_air(palette, &table) {
            entity_commands.insert(IsAllAir);
        }
    }
}

/// `true` if every cell in the section's palette has `emission == 0` and
/// `dampening == 0`. Uses `BlockPalette::for_each_distinct_state` to avoid
/// scanning all 4096 cells when the palette holds only a handful of states.
fn is_section_all_air(palette: &BlockPalette, table: &BlockLightTable) -> bool {
    let mut all_air = true;
    palette.for_each_distinct_state(|state| {
        if !all_air {
            return;
        }
        let idx = state.0 as usize;
        if idx >= table.len() || table.emission[idx] != 0 || table.dampening[idx] != 0 {
            all_air = false;
        }
    });
    all_air
}
