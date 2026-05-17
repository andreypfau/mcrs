//! Consumes `MessageReader<BlockPlaced>`, derives the chunk's intra-cell
//! coord via `rem_euclid(16)` on i32, looks up old/new emission via
//! `Res<BlockLightTable>`, and pushes a decrease and/or increase seed into
//! the chunk's `BlockLightWorkspace` queues per the emission-diff rule:
//! `old_emission > new_emission` → decrease seed at `old_emission`,
//! `new_emission > 0` → increase seed at `new_emission`. `LightDirty` is
//! inserted via `Commands::entity(placed.chunk).insert(LightDirty)` when at
//! least one seed was pushed.
//!
//! Dampening-only changes (`old_emission == new_emission &&
//! old_dampening != new_dampening`) emit a `tracing::warn!` and skip; the
//! cell will desync until cross-chunk distribute lands. Missing
//! `BlockLight`/`BlockLightWorkspace` components on the message's
//! `chunk` entity also emit a warning and skip — defensive against any
//! lifecycle-ordering hazard.

use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Added, Commands, Entity, Or, Query, Res, With};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{
    Column, ColumnIndex, Heightmaps, InColumn, ColumnChunks, ChunkLookup,
};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;

use crate::bfs::{
    normal_of, pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_RECHECK_LEVEL, FLAG_WRITE_LEVEL,
};
use crate::geom::face_cell_to_chunk_xyz;
use crate::heightmap::topmost_surface_world_y;
use crate::components::{
    BlockIncoming, BlockLight, BlockLightWorkspace, BlockNeedsInitialSeed, BlockPendingEgress,
    LightDirty, NeedsFullReseed, NeedsRetop, SkyIncoming, SkyLight, SkyLightSeededAsTopmost,
    SkyLightWorkspace, SkyNeedsInitialSeed, SkyPendingEgress, Wavefront,
};
use crate::nibble::NibbleArray;
use crate::storage::LightStorage;
use crate::table::{flag_bits, BlockLightTable};

pub fn enqueue_block_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut chunks: Query<(&mut BlockLight, &mut BlockLightWorkspace)>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok((mut light, mut workspace)) = chunks.get_mut(placed.chunk) else {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "BlockPlaced.chunk missing BlockLight/BlockLightWorkspace; lifecycle ordering hazard"
            );
            continue;
        };

        let old_emission = table.emission_for(placed.old_state);
        let new_emission = table.emission_for(placed.new_state);
        let old_dampening = table.dampening_for(placed.old_state);
        let new_dampening = table.dampening_for(placed.new_state);

        if old_emission == new_emission && old_dampening != new_dampening {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "dampening-only change not yet handled; light will desync until cross-chunk distribute lands"
            );
            continue;
        }

        let x = placed.block_pos.x.rem_euclid(16) as u8;
        let y = placed.block_pos.y.rem_euclid(16) as u8;
        let z = placed.block_pos.z.rem_euclid(16) as u8;

        let mut pushed = false;

        if old_emission > new_emission {
            // The decrease BFS only walks neighbours, so the seed cell itself
            // must be cleared up front; otherwise the source position keeps
            // its previous emitted level after the emitter is removed.
            light.0.set(x as usize, y as usize, z as usize, 0);

            workspace.decrease_queue.push(pack_bfs_entry(
                x,
                z,
                y,
                old_emission,
                ALL_DIRECTIONS_BITSET,
                0,
            ));
            pushed = true;
        }

        if new_emission > 0 {
            // `FLAG_WRITE_LEVEL` makes the BFS write the source cell to
            // `new_emission` before stepping outward, so the source position
            // is established before any neighbour is reached.
            workspace.increase_queue.push(pack_bfs_entry(
                x,
                z,
                y,
                new_emission,
                ALL_DIRECTIONS_BITSET,
                FLAG_WRITE_LEVEL,
            ));
            pushed = true;
        }

        if pushed {
            commands.entity(placed.chunk).insert(LightDirty);
        }
    }
}

/// Reacts to `BlockPlaced` by enqueuing sky-light decrease and increase seeds
/// whenever the placed block changes either its dampening or its
/// `PROPAGATES_SKYLIGHT_DOWN` flag.
///
/// Missing `SkyLight`/`SkyLightWorkspace` components on the target chunk
/// emit a `tracing::warn!` and skip without panic; this defends against
/// `BlockPlaced` reaching a skyless-dim chunk (where the bundle is never
/// attached) or arriving before the lighting bundle insertion has flushed.
pub fn enqueue_sky_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut chunks: Query<(
        &mut SkyLight,
        &mut SkyLightWorkspace,
        &ChunkPos,
        &InColumn,
    )>,
    columns: Query<&ColumnChunks>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok((mut light, mut workspace, chunk_pos, in_column)) =
            chunks.get_mut(placed.chunk)
        else {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "BlockPlaced.chunk missing SkyLight/SkyLightWorkspace; skipping sky enqueue"
            );
            continue;
        };

        let old_dampening = table.dampening_for(placed.old_state);
        let new_dampening = table.dampening_for(placed.new_state);
        let old_flags = table.flags_for(placed.old_state);
        let new_flags = table.flags_for(placed.new_state);

        let occlusion_changed = !std::ptr::eq(
            table.occlusion_for(placed.old_state) as *const _,
            table.occlusion_for(placed.new_state) as *const _,
        );

        let sky_changed = old_dampening != new_dampening
            || (old_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN)
                != (new_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN)
            || occlusion_changed;
        if !sky_changed {
            continue;
        }

        let is_topmost = match columns.get(in_column.0) {
            Ok(chunk_index) => {
                let top_chunk_y =
                    chunk_index.min_section_y + chunk_index.sections.len() as i32 - 1;
                chunk_pos.y == top_chunk_y
            }
            Err(_) => false,
        };

        let x = placed.block_pos.x.rem_euclid(16) as u8;
        let y = placed.block_pos.y.rem_euclid(16) as u8;
        let z = placed.block_pos.z.rem_euclid(16) as u8;

        let stored = light.0.get(x as usize, y as usize, z as usize);
        let opacity_rose = new_dampening > old_dampening
            || ((old_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN) != 0
                && (new_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN) == 0);
        // The decrease BFS only walks neighbours, so the seed cell must be
        // cleared up front whenever the post-change opacity can only fall
        // below `stored`; otherwise the source position keeps its previous
        // sky-light level even though the new block opaquifies the cell.
        if opacity_rose && stored > 0 {
            light.0.set(x as usize, y as usize, z as usize, 0);
        }
        workspace.decrease_queue.push(pack_bfs_entry(
            x,
            z,
            y,
            stored,
            ALL_DIRECTIONS_BITSET,
            0,
        ));

        if y == 15 && is_topmost {
            workspace.increase_queue.push(pack_bfs_entry(
                x,
                z,
                15,
                15,
                ALL_DIRECTIONS_BITSET,
                FLAG_WRITE_LEVEL,
            ));
        } else {
            for d in [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                let (dx, dy, dz) = normal_of(d);
                let nx = x as i8 + dx;
                let ny = y as i8 + dy;
                let nz = z as i8 + dz;
                if !(0..16).contains(&nx)
                    || !(0..16).contains(&ny)
                    || !(0..16).contains(&nz)
                {
                    continue;
                }
                let neighbour_level =
                    light.0.get(nx as usize, ny as usize, nz as usize);
                workspace.increase_queue.push(pack_bfs_entry(
                    nx as u8,
                    nz as u8,
                    ny as u8,
                    neighbour_level,
                    ALL_DIRECTIONS_BITSET,
                    FLAG_RECHECK_LEVEL,
                ));
            }
        }

        commands.entity(placed.chunk).insert(LightDirty);
    }
}

const CARDINAL_DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// Resolves a neighbour chunk by walking through `ColumnChunks` (for Y-axis
/// neighbours) or `ColumnIndex` + `ColumnChunks` (for X/Z-axis neighbours).
/// Returns `Some(entity)` only for `ChunkLookup::Loaded(entity)`; padding and
/// out-of-range and unloaded neighbours all return `None`.
fn resolve_loaded_neighbor(
    face: Direction,
    chunk_pos: ChunkPos,
    in_col: Entity,
    in_dim: Entity,
    column_indexes: &Query<&ColumnIndex>,
    chunk_indexes: &Query<&ColumnChunks>,
) -> Option<Entity> {
    match face {
        Direction::Up | Direction::Down => {
            let chunk_index = chunk_indexes.get(in_col).ok()?;
            let dy = if face == Direction::Up { 1 } else { -1 };
            match chunk_index.lookup(chunk_pos.y + dy) {
                ChunkLookup::Loaded(e) => Some(e),
                _ => None,
            }
        }
        _ => {
            let column_index = column_indexes.get(in_dim).ok()?;
            let (nx, nz) = match face {
                Direction::North => (chunk_pos.x, chunk_pos.z - 1),
                Direction::South => (chunk_pos.x, chunk_pos.z + 1),
                Direction::West => (chunk_pos.x - 1, chunk_pos.z),
                Direction::East => (chunk_pos.x + 1, chunk_pos.z),
                _ => unreachable!(),
            };
            let neighbour_col_pos =
                mcrs_engine::world::column::ColumnPos::new(nx, nz);
            let slot = column_index.0.get(&neighbour_col_pos)?;
            let neighbour_chunk_index = chunk_indexes.get(slot.entity).ok()?;
            match neighbour_chunk_index.lookup(chunk_pos.y) {
                ChunkLookup::Loaded(e) => Some(e),
                _ => None,
            }
        }
    }
}

/// Block-channel half of the seed-initial split. Scans the chunk's palette
/// for block-light emitters and seeds `BlockLightWorkspace::increase_queue`
/// accordingly. Filter `With<BlockNeedsInitialSeed>` self-gates the system:
/// the marker is present only on chunks awaiting their initial block-light
/// seed; the marker also implies the chunk passed through
/// `attach_lighting_state`, which is the only path that inserts the
/// block-light bundle (so `With<BlockLight>` would be redundant). The marker
/// is always removed at the end of the body, so the system is idempotent
/// across ticks.
pub fn seed_block_emitters(
    table: Option<Res<BlockLightTable>>,
    mut chunks: Query<
        (Entity, &BlockPalette, &mut BlockLightWorkspace),
        With<BlockNeedsInitialSeed>,
    >,
    mut commands: Commands,
) {
    let Some(table) = table else {
        return;
    };
    for (chunk_entity, palette, mut block_ws) in chunks.iter_mut() {
        let mut has_emitter = false;
        palette.for_each_distinct_state(|state| {
            if table.emission_for(state) > 0 {
                has_emitter = true;
            }
        });
        if has_emitter {
            for y in 0..16i32 {
                for z in 0..16i32 {
                    for x in 0..16i32 {
                        let state = palette.get(BlockPos::new(x, y, z));
                        let emission = table.emission_for(state);
                        if emission > 0 {
                            block_ws.increase_queue.push(pack_bfs_entry(
                                x as u8,
                                z as u8,
                                y as u8,
                                emission,
                                ALL_DIRECTIONS_BITSET,
                                FLAG_WRITE_LEVEL,
                            ));
                        }
                    }
                }
            }
            commands.entity(chunk_entity).insert(LightDirty);
        }
        commands
            .entity(chunk_entity)
            .remove::<BlockNeedsInitialSeed>();
    }
}

/// Sky-channel half of the seed-initial split, folded together with the old
/// partial-load 256-seed fallback.
///
/// The filter `Or<(With<SkyNeedsInitialSeed>, Added<SkyLight>)>` matches every
/// chunk needing initial sky-light work: the marker-present branch runs the
/// Case A/B/C heightmap classification body, while the marker-absent +
/// `Added<SkyLight>` branch runs the 256-seed partial-load fallback. The
/// `Added<SkyLight>` filter element fires once per chunk lifetime and the
/// body's `LightStorage::Null` self-gate keeps the fallback from re-seeding
/// once any other branch has written non-Null storage.
///
/// On new-topmost detection, this system inserts `NeedsRetop` on the previous
/// topmost chunk so `invalidate_previous_topmost` can run the decrease wave
/// through its top face in the same tick. The `apply_deferred` barrier
/// between `LightingSet::Enqueue` substages makes the marker visible.
///
/// The `SkyNeedsInitialSeed` marker is removed at the end of the body only
/// when it was present at entry (the `Added<SkyLight>`-only branch fires
/// without the marker).
pub fn seed_sky_initial(
    table: Option<Res<BlockLightTable>>,
    chunk_indexes: Query<&ColumnChunks>,
    heightmaps: Query<&Heightmaps>,
    sky_dims: Query<(), With<HasSkyLight>>,
    mut chunks: Query<
        (
            Entity,
            &BlockPalette,
            &InColumn,
            &InDimension,
            &ChunkPos,
            &mut SkyLightWorkspace,
            &mut SkyLight,
            Option<&SkyNeedsInitialSeed>,
        ),
        (
            With<SkyLight>,
            Or<(With<SkyNeedsInitialSeed>, Added<SkyLight>)>,
        ),
    >,
    mut commands: Commands,
) {
    let Some(_table) = table else {
        return;
    };
    // Track per-chunk "new topmost" outcomes so we can produce a single
    // `NeedsRetop` insert per previous-topmost entity at the tail of this
    // body. This system only inserts the marker —
    // `invalidate_previous_topmost` runs the decrease wave on the next
    // Enqueue substage after the `apply_deferred` barrier.
    let mut retop_targets: Vec<Entity> = Vec::new();

    for (
        chunk_entity,
        _palette,
        in_col,
        in_dim,
        chunk_pos,
        mut sky_ws,
        mut sky_light,
        marker_opt,
    ) in chunks.iter_mut()
    {
        let dim_has_sky = sky_dims.get(in_dim.0).is_ok();
        if !dim_has_sky {
            // Defensive: the `Or<(SkyNeedsInitialSeed, Added<SkyLight>)>` filter
            // matched, but the chunk's dim is skyless. Without HasSkyLight there
            // is nothing meaningful for this system to do; only clear the
            // marker if it was somehow inserted by a non-lifecycle path.
            if marker_opt.is_some() {
                commands
                    .entity(chunk_entity)
                    .remove::<SkyNeedsInitialSeed>();
            }
            continue;
        }

        let is_topmost = chunk_indexes
            .get(in_col.0)
            .ok()
            .map(|si| chunk_pos.y == si.min_section_y + si.sections.len() as i32 - 1)
            .unwrap_or(false);

        let mut sky_seeded = false;
        let mut seeded_topmost = false;

        if marker_opt.is_some() {
            // Marker-present branch: Case A/B/C heightmap fast-path.
            let chunk_base_y = chunk_pos.y * 16;
            let chunk_top_y = chunk_base_y + 15;

            match heightmaps.get(in_col.0) {
                Ok(hm) => {
                    let mut all_above = true;
                    let mut all_below = true;
                    for z in 0..16usize {
                        for x in 0..16usize {
                            match topmost_surface_world_y(hm, x, z) {
                                None => {
                                    all_below = false;
                                }
                                Some(s) => {
                                    if s > chunk_base_y {
                                        all_above = false;
                                    }
                                    if s <= chunk_top_y {
                                        all_below = false;
                                    }
                                }
                            }
                        }
                    }

                    if all_above {
                        // Case A: every column's first-air-above-surface is
                        // at or below this chunk's base. All 4096 cells are
                        // air at level 15. Store the compressed Uniform(15)
                        // form and skip LightDirty — there is no work to
                        // converge.
                        if chunk_base_y <= 0 {
                            // Cave-or-deeper chunks must never reach Case A
                            // on a real overworld. If they do, the column's
                            // heightmap is at sentinel when this system
                            // fired — capture the offending column for
                            // diagnosis.
                            let min_y = hm.min_y();
                            let mut sample = [0i32; 9];
                            let pts = [
                                (0usize, 0usize), (15, 0), (0, 15), (15, 15),
                                (7, 7), (0, 7), (15, 7), (7, 0), (7, 15),
                            ];
                            let mut sentinel_count = 0u16;
                            let mut min_read = i32::MAX;
                            let mut max_read = i32::MIN;
                            for z in 0..16usize {
                                for x in 0..16usize {
                                    let s = hm.surface_get(x, z);
                                    min_read = min_read.min(s);
                                    max_read = max_read.max(s);
                                    if s == min_y { sentinel_count += 1; }
                                }
                            }
                            for (i, (x, z)) in pts.iter().enumerate() {
                                sample[i] = hm.surface_get(*x, *z);
                            }
                            tracing::warn!(
                                target: "mcrs_lighting::case_a_cave",
                                chunk_x = chunk_pos.x,
                                chunk_y = chunk_pos.y,
                                chunk_z = chunk_pos.z,
                                chunk_base_y,
                                chunk_top_y,
                                column = ?in_col.0,
                                chunk = ?chunk_entity,
                                heightmap_min_y = min_y,
                                heightmap_min_read = min_read,
                                heightmap_max_read = max_read,
                                heightmap_sentinel_count = sentinel_count,
                                heightmap_sample = ?sample,
                                "Case A fired for cave chunk: chunk will be Uniform(15) but is below y=0 — heightmap likely unprimed"
                            );
                        }
                        sky_light.0 = LightStorage::Uniform(15);
                    } else if all_below {
                        // Case C: every column's surface is at or above
                        // this chunk's top. No sky light reaches here;
                        // storage stays Null (=0).
                    } else {
                        // Case B: straddling. Start from a uniform-15 nibble
                        // array (single 2 KiB memset) and zero out only the
                        // below-surface cells per (x, z) column.
                        let mut arr = NibbleArray::filled(15);
                        for z in 0..16usize {
                            for x in 0..16usize {
                                let s_opt = topmost_surface_world_y(hm, x, z);
                                let max_dark_local_y = match s_opt {
                                    Some(s) if s > chunk_base_y => {
                                        (s - chunk_base_y).min(16) as usize
                                    }
                                    _ => 0,
                                };
                                for y_local in 0..max_dark_local_y {
                                    arr.set(x, y_local, z, 0);
                                }
                                let lit_in_chunk =
                                    s_opt.map_or(true, |s| s <= chunk_top_y);
                                if lit_in_chunk {
                                    let first_seed_y: u8 = match s_opt {
                                        Some(s) if s >= chunk_base_y => {
                                            (s - chunk_base_y) as u8
                                        }
                                        _ => 0,
                                    };
                                    for y_seed_local in first_seed_y..=15u8 {
                                        sky_ws.increase_queue.push(pack_bfs_entry(
                                            x as u8,
                                            z as u8,
                                            y_seed_local,
                                            15,
                                            ALL_DIRECTIONS_BITSET,
                                            0,
                                        ));
                                    }
                                    sky_seeded = true;
                                }
                            }
                        }
                        sky_light.0 = LightStorage::Mixed(Box::new(arr));
                    }

                    if is_topmost {
                        commands
                            .entity(chunk_entity)
                            .insert(SkyLightSeededAsTopmost);
                        seeded_topmost = true;
                    }
                }
                Err(_) => {
                    // Defensive fallback when `Heightmaps` is missing.
                    // Reproduces the pre-fast-path behaviour: only the topmost
                    // chunk gets 256 seeds at y=15.
                    if is_topmost {
                        sky_ws.increase_queue.reserve(256);
                        for z in 0..16u8 {
                            for x in 0..16u8 {
                                sky_ws.increase_queue.push(pack_bfs_entry(
                                    x,
                                    z,
                                    15,
                                    15,
                                    ALL_DIRECTIONS_BITSET,
                                    FLAG_WRITE_LEVEL,
                                ));
                            }
                        }
                        commands
                            .entity(chunk_entity)
                            .insert(SkyLightSeededAsTopmost);
                        sky_seeded = true;
                        seeded_topmost = true;
                    }
                }
            }
        } else {
            // Marker-absent + Added<SkyLight> branch: the 256-seed
            // partial-load fallback. Fires once per chunk lifetime
            // (Added<SkyLight> is single-shot). The `LightStorage::Null`
            // self-gate prevents double-seeding when any earlier branch has
            // already written storage.
            if !matches!(sky_light.0, LightStorage::Null) {
                continue;
            }
            let Ok(chunk_index) = chunk_indexes.get(in_col.0) else {
                continue;
            };
            let top_chunk_y =
                chunk_index.min_section_y + chunk_index.sections.len() as i32 - 1;
            if chunk_pos.y != top_chunk_y {
                continue;
            }
            sky_ws.increase_queue.reserve(256);
            for z in 0..16u8 {
                for x in 0..16u8 {
                    sky_ws.increase_queue.push(pack_bfs_entry(
                        x,
                        z,
                        15,
                        15,
                        ALL_DIRECTIONS_BITSET,
                        FLAG_WRITE_LEVEL,
                    ));
                }
            }
            sky_seeded = true;
        }

        if sky_seeded {
            commands.entity(chunk_entity).insert(LightDirty);
        }
        if marker_opt.is_some() {
            commands
                .entity(chunk_entity)
                .remove::<SkyNeedsInitialSeed>();
        }
        if seeded_topmost {
            retop_targets.push(chunk_entity);
        }
        let _ = retop_targets.last();
    }

    // For each chunk that just became the new topmost, locate any prior
    // topmost (in the same column at a lower chunk_pos.y) and tag it with
    // `NeedsRetop` + `LightDirty`. The decrease wave itself runs in
    // `invalidate_previous_topmost` after the `apply_deferred` barrier.
    if !retop_targets.is_empty() {
        // We need access to (Entity, ChunkPos, InColumn, SkyLightSeededAsTopmost)
        // for every chunk in the affected columns. The producer side does not
        // need to read the previous-topmost workspace — only its identity —
        // so the query stays read-only on entity components and writes happen
        // via Commands.
        //
        // Re-derive the column/new_chunk_y pair from the freshly-completed
        // iteration above by recovering data through `chunks.get` (the query
        // is read-only here so the &mut SkyLight borrow is dropped).
        for new_topmost_entity in &retop_targets {
            let Ok((_, _, new_in_col, _, new_chunk_pos, _, _, _)) =
                chunks.get(*new_topmost_entity)
            else {
                continue;
            };
            let new_column = new_in_col.0;
            let new_chunk_y = new_chunk_pos.y;
            // Walk the column's chunk_index sections and find any loaded slot
            // below new_chunk_y; if the chunk-pos check + SkyLightSeededAsTopmost
            // marker both hold, schedule the handoff.
            let Ok(chunk_index) = chunk_indexes.get(new_column) else {
                continue;
            };
            for slot in chunk_index.sections.iter() {
                let Some(slot_entity) = slot else { continue };
                if *slot_entity == *new_topmost_entity {
                    continue;
                }
                let Ok((_, _, slot_in_col, _, slot_chunk_pos, _, _, _)) =
                    chunks.get(*slot_entity)
                else {
                    // Not in our filtered query (no SkyLight or no markers
                    // matched). Fall back to a probe via Commands —
                    // SkyLightSeededAsTopmost is the canonical predecessor
                    // marker; we can't read it from this query, so we
                    // unconditionally tag any column-mate below the new
                    // topmost. The consumer `invalidate_previous_topmost`
                    // filters on `With<NeedsRetop>`, so spurious tags on
                    // chunks that don't carry storage are harmless.
                    continue;
                };
                if slot_in_col.0 != new_column {
                    continue;
                }
                if slot_chunk_pos.y >= new_chunk_y {
                    continue;
                }
                commands
                    .entity(*slot_entity)
                    .remove::<SkyLightSeededAsTopmost>()
                    .insert(NeedsRetop)
                    .insert(LightDirty);
            }
        }
    }
}

/// Consumer half of the retopping handoff. Runs the per-cell decrease-queue
/// push body; populates the previous topmost chunk's
/// `SkyLightWorkspace.decrease_queue` with 256 entries carrying the chunk's
/// stored top-face sky levels. Removes the `NeedsRetop` marker at the end so
/// the system is idempotent.
///
/// Filter `With<NeedsRetop>` is sufficient because every previous-topmost that
/// should be invalidated has been tagged by `seed_sky_initial`. Scheduling
/// requirement: `invalidate_previous_topmost.after(seed_sky_initial)` so the
/// producer's `commands.insert(NeedsRetop)` is visible after the
/// `apply_deferred` barrier between `LightingSet::Enqueue` substages.
pub(crate) fn invalidate_previous_topmost(
    mut prev_chunks: Query<
        (Entity, &ChunkPos, &InColumn, &mut SkyLightWorkspace, &SkyLight),
        With<NeedsRetop>,
    >,
    mut commands: Commands,
) {
    for (prev_entity, _prev_chunk_pos, _prev_in_col, mut prev_sky_ws, prev_sky_light) in
        prev_chunks.iter_mut()
    {
        for z in 0..16u8 {
            for x in 0..16u8 {
                let stored = prev_sky_light.0.get(x as usize, 15, z as usize);
                prev_sky_ws.decrease_queue.push(pack_bfs_entry(
                    x,
                    z,
                    15,
                    stored,
                    ALL_DIRECTIONS_BITSET,
                    0,
                ));
            }
        }
        commands.entity(prev_entity).remove::<NeedsRetop>();
        commands.entity(prev_entity).insert(LightDirty);
    }
}

/// Block-light half of the per-channel chunk-edge pull. Consumes
/// `Added<ChunkLoaded>` per chunk: reads each loaded cardinal neighbour's
/// face-cell `BlockLight` values into the new chunk's `BlockIncoming`, then
/// drains any `BlockPendingEgress` entries that the neighbour buffered
/// while we were unloaded. Marks the new chunk and every touched loaded
/// neighbour `LightDirty` when something actually moved.
///
/// Scheduled in parallel with `pull_sky_neighbor_edges`. The two systems
/// take disjoint `&BlockLight`/`&SkyLight` reads and disjoint
/// `&mut BlockPendingEgress`/`&mut SkyPendingEgress` writes, so Bevy's
/// conflict graph slots them simultaneously.
///
/// Unlike the sky channel, this system unconditionally skips a neighbour
/// that was also `Added<ChunkLoaded>` this tick. There is no block-light
/// fast-path that produces `LightStorage::Uniform(15)` at seed time (the
/// `Uniform(15)` heightmap fast-path is sky-only), so an `Uniform(15)`-
/// neighbour escape hatch would never legitimately fire and would risk
/// pulling from hand-authored uniforms that have not yet settled.
pub fn pull_block_neighbor_edges(
    table: Option<Res<BlockLightTable>>,
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension, &InColumn), Added<ChunkLoaded>>,
    column_indexes: Query<&ColumnIndex>,
    chunk_indexes: Query<&ColumnChunks>,
    block_light_read: Query<&BlockLight>,
    mut block_pending: Query<&mut BlockPendingEgress>,
    mut block_incoming: Query<&mut BlockIncoming>,
    mut commands: Commands,
) {
    if table.is_none() {
        return;
    }

    let newly_loaded_set: rustc_hash::FxHashSet<Entity> =
        newly_loaded.iter().map(|(e, _, _, _)| e).collect();

    for (new_chunk, chunk_pos, in_dim, in_col) in newly_loaded.iter() {
        let mut new_chunk_has_incoming = false;

        // Cell-level pull cannot beat a `Uniform(15)` destination, so a
        // pre-check on the new chunk's block storage lets us skip the
        // per-face 256-cell read loop entirely when the chunk is already
        // saturated.
        let new_block_already_max = block_light_read
            .get(new_chunk)
            .ok()
            .map(|bl| matches!(bl.0, LightStorage::Uniform(15)))
            .unwrap_or(false);

        for face in CARDINAL_DIRECTIONS {
            let Some(neighbour_entity) = resolve_loaded_neighbor(
                face,
                *chunk_pos,
                in_col.0,
                in_dim.0,
                &column_indexes,
                &chunk_indexes,
            ) else {
                continue;
            };

            // Block channel: no `Uniform(15)`-neighbour escape hatch (sky
            // has one because the Case A heightmap fast-path writes
            // `Uniform(15)` skies at seed time). Always skip newly-loaded
            // neighbours; the natural egress→distribute cascade routes
            // any flow between fresh chunks during convergence.
            if newly_loaded_set.contains(&neighbour_entity) {
                continue;
            }

            let from_face = face.opposite();
            let dest_face = face.index() as u8;
            let neighbour_expected_face = from_face.index() as u8;

            let mut drained_pending_from_neighbour = false;

            if !new_block_already_max {
                for cell_a in 0..16u8 {
                    for cell_b in 0..16u8 {
                        let (nx, ny, nz) =
                            face_cell_to_chunk_xyz(from_face, cell_a, cell_b);

                        if let Ok(bl) = block_light_read.get(neighbour_entity) {
                            let level = bl.0.get(nx as usize, ny as usize, nz as usize);
                            if level > 0 {
                                let attenuated = level.saturating_sub(1);
                                if let Ok(mut inc) = block_incoming.get_mut(new_chunk) {
                                    inc.0.push(Wavefront::new(
                                        dest_face, cell_a, cell_b, attenuated,
                                    ));
                                    new_chunk_has_incoming = true;
                                }
                            }
                        }
                    }
                }
            }

            if let Ok(mut pending) = block_pending.get_mut(neighbour_entity) {
                if !pending.0.is_empty() {
                    pending.0.retain(|w| {
                        if w.face() == neighbour_expected_face {
                            if let Ok(mut inc) = block_incoming.get_mut(new_chunk) {
                                inc.0.push(Wavefront::new(
                                    dest_face,
                                    w.cell_x(),
                                    w.cell_z(),
                                    w.level(),
                                ));
                                new_chunk_has_incoming = true;
                                drained_pending_from_neighbour = true;
                            }
                            false
                        } else {
                            true
                        }
                    });
                }
            }

            if drained_pending_from_neighbour {
                commands.entity(neighbour_entity).insert(LightDirty);
            }
        }

        if new_chunk_has_incoming {
            commands.entity(new_chunk).insert(LightDirty);
        }
    }
}

/// Sky-light half of the per-channel chunk-edge pull. Mirror of
/// `pull_block_neighbor_edges` operating on `SkyLight` / `SkyPendingEgress`
/// / `SkyIncoming`.
///
/// Retains the sky-only escape hatch: if a newly-loaded neighbour already
/// holds `LightStorage::Uniform(15)` (the Case A heightmap fast-path
/// outcome, written by `seed_sky_initial`), pull from it even though it
/// was also `Added<ChunkLoaded>` this tick. The neighbour's storage is
/// final and must be observed now so that a dark (Case B/C) new chunk
/// receives the correct initial wavefront at its shared face. For all
/// other newly-loaded neighbours the egress→distribute cascade routes any
/// flow during convergence.
pub fn pull_sky_neighbor_edges(
    table: Option<Res<BlockLightTable>>,
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension, &InColumn), Added<ChunkLoaded>>,
    column_indexes: Query<&ColumnIndex>,
    chunk_indexes: Query<&ColumnChunks>,
    sky_light_read: Query<&SkyLight>,
    mut sky_pending: Query<&mut SkyPendingEgress>,
    mut sky_incoming: Query<&mut SkyIncoming>,
    mut commands: Commands,
) {
    if table.is_none() {
        return;
    }

    let newly_loaded_set: rustc_hash::FxHashSet<Entity> =
        newly_loaded.iter().map(|(e, _, _, _)| e).collect();

    for (new_chunk, chunk_pos, in_dim, in_col) in newly_loaded.iter() {
        let mut new_chunk_has_incoming = false;

        let new_sky_already_max = sky_light_read
            .get(new_chunk)
            .ok()
            .map(|sl| matches!(sl.0, LightStorage::Uniform(15)))
            .unwrap_or(false);

        for face in CARDINAL_DIRECTIONS {
            let Some(neighbour_entity) = resolve_loaded_neighbor(
                face,
                *chunk_pos,
                in_col.0,
                in_dim.0,
                &column_indexes,
                &chunk_indexes,
            ) else {
                continue;
            };

            if newly_loaded_set.contains(&neighbour_entity) {
                let neighbour_sky_is_uniform_15 = sky_light_read
                    .get(neighbour_entity)
                    .map(|sl| matches!(sl.0, LightStorage::Uniform(15)))
                    .unwrap_or(false);
                if !neighbour_sky_is_uniform_15 {
                    continue;
                }
            }

            let from_face = face.opposite();
            let dest_face = face.index() as u8;
            let neighbour_expected_face = from_face.index() as u8;

            let mut drained_pending_from_neighbour = false;

            if !new_sky_already_max {
                for cell_a in 0..16u8 {
                    for cell_b in 0..16u8 {
                        let (nx, ny, nz) =
                            face_cell_to_chunk_xyz(from_face, cell_a, cell_b);

                        if let Ok(sl) = sky_light_read.get(neighbour_entity) {
                            let level = sl.0.get(nx as usize, ny as usize, nz as usize);
                            if level > 0 {
                                let attenuated = level.saturating_sub(1);
                                if let Ok(mut inc) = sky_incoming.get_mut(new_chunk) {
                                    inc.0.push(Wavefront::new(
                                        dest_face, cell_a, cell_b, attenuated,
                                    ));
                                    new_chunk_has_incoming = true;
                                }
                            }
                        }
                    }
                }
            }

            if let Ok(mut pending) = sky_pending.get_mut(neighbour_entity) {
                if !pending.0.is_empty() {
                    pending.0.retain(|w| {
                        if w.face() == neighbour_expected_face {
                            if let Ok(mut inc) = sky_incoming.get_mut(new_chunk) {
                                inc.0.push(Wavefront::new(
                                    dest_face,
                                    w.cell_x(),
                                    w.cell_z(),
                                    w.level(),
                                ));
                                new_chunk_has_incoming = true;
                                drained_pending_from_neighbour = true;
                            }
                            false
                        } else {
                            true
                        }
                    });
                }
            }

            if drained_pending_from_neighbour {
                commands.entity(neighbour_entity).insert(LightDirty);
            }
        }

        if new_chunk_has_incoming {
            commands.entity(new_chunk).insert(LightDirty);
        }
    }
}

/// Consumes `Added<NeedsFullReseed>` on `Column` entities: iterates the
/// column's `ColumnChunks.sections` slots and re-inserts
/// `BlockNeedsInitialSeed` unconditionally plus `SkyNeedsInitialSeed` on
/// chunks whose dimension carries `HasSkyLight`, on every loaded chunk in the
/// column. Removes `NeedsFullReseed` from the column entity.
///
/// Gated on `ColumnHeightmapScan::is_finalized()`. When the scan is not yet
/// finalized, `Heightmaps::surface_get` returns the sentinel `min_y` for
/// every unclosed XZ column. Re-marking chunks with the per-channel markers
/// in that state causes `seed_sky_initial` to misclassify cave chunks as
/// Case A (Uniform(15)). The natural lifecycle in
/// `prime_heightmaps_on_column_spawn` inserts the markers once the scan
/// finalizes with a correctly primed heightmap, so deferring is safe.
pub fn consume_needs_full_reseed(
    newly_marked: Query<
        (Entity, &ColumnChunks, Option<&crate::lifecycle::ColumnHeightmapScan>),
        (With<Column>, Added<NeedsFullReseed>),
    >,
    in_dimensions: Query<&InDimension>,
    sky_dims: Query<(), With<HasSkyLight>>,
    mut commands: Commands,
) {
    for (column_entity, chunk_index, scan_opt) in newly_marked.iter() {
        let loaded = chunk_index.sections.iter().filter(|s| s.is_some()).count();
        let total = chunk_index.sections.len();
        let scan_finalized = scan_opt.map_or(false, |s| s.is_finalized());

        if !scan_finalized {
            tracing::warn!(
                target: "mcrs_lighting::consume_reseed",
                column = ?column_entity,
                chunks_loaded = loaded,
                chunks_total = total,
                scan_present = scan_opt.is_some(),
                "Dropping NeedsFullReseed: heightmap scan not finalized. The natural lifecycle in \
                 prime_heightmaps_on_column_spawn will insert the per-channel needs-initial markers \
                 when the scan closes; reseeding now would read sentinel min_y and mis-Uniform(15) cave chunks."
            );
            commands.entity(column_entity).remove::<NeedsFullReseed>();
            continue;
        }

        for slot in chunk_index.sections.iter() {
            if let Some(chunk_entity) = slot {
                let mut e = commands.entity(*chunk_entity);
                e.insert(BlockNeedsInitialSeed);
                if let Ok(in_dim) = in_dimensions.get(*chunk_entity) {
                    if sky_dims.get(in_dim.0).is_ok() {
                        e.insert(SkyNeedsInitialSeed);
                    }
                }
            }
        }
        commands.entity(column_entity).remove::<NeedsFullReseed>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs::{
        unpack_bfs_entry_flags, unpack_bfs_entry_level, unpack_bfs_entry_x, unpack_bfs_entry_y,
        unpack_bfs_entry_z,
    };
    use bevy_app::{App, Update};
    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::IntoScheduleConfigs;
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_engine::world::chunk::ChunkPos;
    use mcrs_engine::world::column::{Column, ColumnPos, ColumnIndex, ColumnSlot, InColumn, ColumnChunks};
    use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
    use mcrs_lighting_table_helpers::*;
    use mcrs_minecraft_block::block::BlockUpdateFlags;
    use mcrs_protocol::BlockStateId;

    mod mcrs_lighting_table_helpers {
        use super::*;
        use crate::table::{flag_bits, BlockLightTable};

        pub const AIR: BlockStateId = BlockStateId(0);
        pub const STONE: BlockStateId = BlockStateId(1);
        pub const TORCH_HI: BlockStateId = BlockStateId(2);
        pub const TORCH_LO: BlockStateId = BlockStateId(3);
        pub const LEAVES: BlockStateId = BlockStateId(4);

        pub fn make_test_table() -> BlockLightTable {
            let state_count = 5usize;
            let mut emission = vec![0u8; state_count].into_boxed_slice();
            let mut dampening = vec![0u8; state_count].into_boxed_slice();
            let occlusion: Box<[&'static VoxelShape]> =
                vec![VoxelShape::empty(); state_count].into_boxed_slice();
            let mut flags = vec![0u8; state_count].into_boxed_slice();

            emission[AIR.0 as usize] = 0;
            dampening[AIR.0 as usize] = 0;
            flags[AIR.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[STONE.0 as usize] = 0;
            dampening[STONE.0 as usize] = 0;
            flags[STONE.0 as usize] = 0;

            emission[TORCH_HI.0 as usize] = 14;
            dampening[TORCH_HI.0 as usize] = 0;
            flags[TORCH_HI.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[TORCH_LO.0 as usize] = 7;
            dampening[TORCH_LO.0 as usize] = 0;
            flags[TORCH_LO.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[LEAVES.0 as usize] = 0;
            dampening[LEAVES.0 as usize] = 1;
            flags[LEAVES.0 as usize] = flag_bits::IS_NOT_AIR;

            BlockLightTable {
                emission,
                dampening,
                occlusion,
                flags,
            }
        }
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, enqueue_block_light_on_block_placed);
        app
    }

    fn spawn_chunk(app: &mut App) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn((BlockLight::default(), BlockLightWorkspace::default()))
            .id()
    }

    fn write_placed(app: &mut App, placed: BlockPlaced) {
        app.world_mut()
            .resource_mut::<Messages<BlockPlaced>>()
            .write(placed);
    }

    fn block_placed(
        chunk: bevy_ecs::entity::Entity,
        block_pos: BlockPos,
        old_state: BlockStateId,
        new_state: BlockStateId,
    ) -> BlockPlaced {
        BlockPlaced {
            chunk,
            chunk_pos: ChunkPos::new(0, 0, 0),
            block_pos,
            old_state,
            new_state,
            flags: BlockUpdateFlags::empty(),
        }
    }

    #[test]
    fn enqueue_increase_on_emitter_placed() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 5, 9), AIR, TORCH_HI),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.increase_queue.len(), 1, "one increase seed");
        assert!(
            workspace.decrease_queue.is_empty(),
            "no decrease seed for 0 → 14"
        );
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 5);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 14);
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "LightDirty inserted"
        );
    }

    #[test]
    fn enqueue_decrease_on_emitter_removed() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 8, 8), TORCH_HI, AIR),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.decrease_queue.len(), 1, "one decrease seed");
        assert!(
            workspace.increase_queue.is_empty(),
            "no increase seed for 14 → 0"
        );
        let entry = workspace.decrease_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 8);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 8);
        assert_eq!(unpack_bfs_entry_z(entry), 8);
        assert_eq!(unpack_bfs_entry_level(entry), 14);
        assert!(app.world().get::<LightDirty>(entity).is_some());
    }

    #[test]
    fn enqueue_both_on_swap() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(1, 2, 3), TORCH_HI, TORCH_LO),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.decrease_queue.len(), 1);
        assert_eq!(workspace.increase_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(workspace.decrease_queue[0]),
            14,
            "decrease at old emission"
        );
        assert_eq!(
            unpack_bfs_entry_level(workspace.increase_queue[0]),
            7,
            "increase at new emission"
        );
        assert!(app.world().get::<LightDirty>(entity).is_some());
    }

    #[test]
    fn enqueue_no_op_on_zero_zero() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // AIR → STONE: both emission=0, both dampening=0 in the test table, so
        // the dampening-only-change branch does NOT trigger and the system
        // simply records no work.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, STONE),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert!(workspace.increase_queue.is_empty());
        assert!(workspace.decrease_queue.is_empty());
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on no-op"
        );
    }

    #[test]
    fn enqueue_dampening_only_change_warns() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // AIR (emission=0, dampening=0) → LEAVES (emission=0, dampening=1).
        // Pure dampening change; the system warns and skips.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert!(
            workspace.increase_queue.is_empty(),
            "dampening-only skips increase"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "dampening-only skips decrease"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on dampening-only change"
        );
    }

    #[test]
    fn enqueue_missing_components_warns() {
        let mut app = build_app();
        // Spawn an entity WITHOUT BlockLight/BlockLightWorkspace — emulates
        // a chunk the lighting lifecycle has not yet attached state to.
        let entity = app.world_mut().spawn(()).id();
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, TORCH_HI),
        );

        app.update();

        assert!(
            app.world().get::<BlockLightWorkspace>(entity).is_none(),
            "entity still has no workspace"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty must NOT be inserted on missing components"
        );
    }

    #[test]
    fn enqueue_uses_rem_euclid_for_negative_coords() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // BlockPos::new(-3, 5, -19) — rem_euclid(16) yields (13, 5, 13).
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(-3, 5, -19), AIR, TORCH_HI),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.increase_queue.len(), 1);
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 13, "x = -3 rem_euclid 16 = 13");
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 5);
        assert_eq!(unpack_bfs_entry_z(entry), 13, "z = -19 rem_euclid 16 = 13");
        assert_eq!(unpack_bfs_entry_level(entry), 14);
    }

    fn build_sky_initial_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, seed_sky_initial);
        app
    }

    fn spawn_fallback_dim(app: &mut App) -> bevy_ecs::entity::Entity {
        let mut e = app.world_mut().spawn(ColumnIndex::default());
        e.insert(HasSkyLight);
        e.id()
    }

    fn air_palette_local() -> BlockPalette {
        let mut p = BlockPalette::default();
        p.fill(AIR);
        p
    }

    /// Fallback branch: a topmost-of-column chunk freshly added to a
    /// sky-having dim (no `SkyNeedsInitialSeed` marker; no primed heightmap on
    /// the column) must seed 256 entries via the `Added<SkyLight>` arm.
    #[test]
    fn seed_sky_initial_seeds_topmost_chunk_only_via_fallback() {
        let mut app = build_sky_initial_app();
        let dim = spawn_fallback_dim(&mut app);

        let chunk = app.world_mut().spawn_empty().id();
        // Anchor the column on the dim and add the column to the dim's index
        // so `seed_sky_initial`'s dim-has-sky probe finds the dim.
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        app.world_mut().entity_mut(chunk).insert((
            air_palette_local(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            SkyLight::default(),
            SkyLightWorkspace::default(),
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(chunk)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            256,
            "topmost-of-column chunk seeds 256 entries (16 x 16 at y=15)"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "initial seed does not push decrease entries"
        );
        for entry in &workspace.increase_queue {
            assert_eq!(unpack_bfs_entry_y(*entry) as u8, 15, "y == 15");
            assert_eq!(unpack_bfs_entry_level(*entry), 15, "level == 15");
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_WRITE_LEVEL,
                0,
                "FLAG_WRITE_LEVEL bit set on every seed"
            );
        }
        assert!(
            app.world().get::<LightDirty>(chunk).is_some(),
            "LightDirty inserted on topmost-of-column seed"
        );
    }

    /// Counterpart to the fallback test: a non-topmost chunk freshly added to
    /// the same sky-having dim must not seed 256 entries.
    #[test]
    fn seed_sky_initial_skips_non_topmost_via_fallback() {
        let mut app = build_sky_initial_app();
        let dim = spawn_fallback_dim(&mut app);

        let chunk_below = app.world_mut().spawn_empty().id();
        let chunk_topmost = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_below), Some(chunk_topmost)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        // Only the below chunk gets SkyLight added; topmost is left bare
        // so this single test does not also seed an unrelated chunk.
        app.world_mut().entity_mut(chunk_below).insert((
            air_palette_local(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            SkyLight::default(),
            SkyLightWorkspace::default(),
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(chunk_below)
            .expect("sky workspace");
        assert!(
            workspace.increase_queue.is_empty(),
            "non-topmost chunk seeds nothing"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "non-topmost chunk seeds no decrease"
        );
        assert!(
            app.world().get::<LightDirty>(chunk_below).is_none(),
            "LightDirty NOT inserted on non-topmost-of-column chunk"
        );
    }

    fn build_sky_on_placed_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, enqueue_sky_light_on_block_placed);
        app
    }

    fn spawn_sky_chunk_topmost(app: &mut App) -> bevy_ecs::entity::Entity {
        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(ColumnChunks {
                min_section_y: 0,
                sections: vec![Some(chunk)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(chunk).insert((
            SkyLight::default(),
            SkyLightWorkspace::default(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
        ));
        chunk
    }

    fn spawn_sky_chunk_non_topmost(app: &mut App) -> bevy_ecs::entity::Entity {
        let chunk = app.world_mut().spawn_empty().id();
        let dummy_topmost = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(ColumnChunks {
                min_section_y: 0,
                sections: vec![Some(chunk), Some(dummy_topmost)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(chunk).insert((
            SkyLight::default(),
            SkyLightWorkspace::default(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
        ));
        chunk
    }

    #[test]
    fn enqueue_sky_on_block_placed_pushes_decrease_and_neighbour_seeds() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        // AIR (damp=0, propagates) -> LEAVES (damp=1, no propagates flag);
        // sky_changed predicate trips on both dampening and flag delta.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 10, 8), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert!(
            !workspace.decrease_queue.is_empty(),
            "dampening change pushes a decrease seed"
        );
        assert!(
            !workspace.increase_queue.is_empty(),
            "y=10 (non-top) pushes neighbour-support increase seeds"
        );
        // y=10 (intra-chunk, not 15) -> six neighbour seeds.
        assert_eq!(
            workspace.increase_queue.len(),
            6,
            "y < 15 produces exactly six neighbour-support seeds"
        );
        for entry in &workspace.increase_queue {
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_RECHECK_LEVEL,
                0,
                "every neighbour seed carries FLAG_RECHECK_LEVEL"
            );
        }
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "LightDirty inserted after dampening change"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_top_seeds_top_face() {
        // y == 15 path: a single top-face increase seed instead of six
        // neighbour seeds.
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            1,
            "y == 15 produces exactly one top-face seed"
        );
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 15);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 15);
        assert_ne!(
            unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
            0,
            "top-of-chunk seed carries FLAG_WRITE_LEVEL"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_skips_when_predicate_false() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        // AIR -> AIR: old_state == new_state, early-out before predicate.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, AIR),
        );
        // AIR -> TORCH_HI: both have dampening=0 AND PROPAGATES_SKYLIGHT_DOWN,
        // so sky_changed is false and the system continues without queueing.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(1, 1, 1), AIR, TORCH_HI),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert!(workspace.increase_queue.is_empty());
        assert!(workspace.decrease_queue.is_empty());
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on no-op sky enqueue"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_warns_missing_components() {
        use std::io;
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

        #[derive(Clone)]
        struct VecWriter(Arc<Mutex<Vec<u8>>>);

        impl io::Write for VecWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        impl<'a> MakeWriter<'a> for VecWriter {
            type Writer = VecWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let writer = VecWriter(Arc::clone(&captured));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let mut app = build_sky_on_placed_app();
            // Chunk without SkyLight/SkyLightWorkspace (skyless-dim shape).
            let entity = app
                .world_mut()
                .spawn((BlockLight::default(), BlockLightWorkspace::default()))
                .id();
            write_placed(
                &mut app,
                block_placed(entity, BlockPos::new(2, 3, 4), AIR, LEAVES),
            );

            app.update();

            assert!(
                app.world().get::<SkyLightWorkspace>(entity).is_none(),
                "entity still has no sky workspace"
            );
            assert!(
                app.world().get::<LightDirty>(entity).is_none(),
                "LightDirty must NOT be inserted when SkyLight is missing"
            );
        });

        let bytes = captured.lock().unwrap();
        let output = String::from_utf8_lossy(&bytes);
        assert!(
            output.contains("BlockPlaced.chunk missing SkyLight/SkyLightWorkspace"),
            "expected warn substring in captured tracing output, got: {output}"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_clears_seed_cell_on_opacity_rise() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        app.world_mut()
            .get_mut::<SkyLight>(entity)
            .expect("sky light")
            .0
            .set(8, 5, 8, 10);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), AIR, LEAVES),
        );

        app.update();

        let light = app.world().get::<SkyLight>(entity).expect("sky light");
        assert_eq!(
            light.0.get(8, 5, 8),
            0,
            "seed cell cleared because opacity rose"
        );
        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(workspace.decrease_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(workspace.decrease_queue[0]),
            10,
            "decrease seed carries pre-clear stored level"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_keeps_seed_cell_when_opacity_drops() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        app.world_mut()
            .get_mut::<SkyLight>(entity)
            .expect("sky light")
            .0
            .set(8, 5, 8, 3);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), LEAVES, AIR),
        );

        app.update();

        let light = app.world().get::<SkyLight>(entity).expect("sky light");
        assert_eq!(
            light.0.get(8, 5, 8),
            3,
            "seed cell unchanged because opacity did not rise"
        );
        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(workspace.decrease_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(workspace.decrease_queue[0]),
            3,
            "decrease seed carries stored level"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_skips_top_seed_when_not_topmost() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_non_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        // y=15 sits at the top of the chunk, so the Up neighbour at y=16
        // is outside the chunk and is skipped by the bounds guard. Five
        // neighbour-recheck seeds remain.
        assert_eq!(
            workspace.increase_queue.len(),
            5,
            "non-topmost chunk falls through to neighbour-recheck branch at y=15"
        );
        for entry in &workspace.increase_queue {
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_RECHECK_LEVEL,
                0,
                "every neighbour seed carries FLAG_RECHECK_LEVEL"
            );
            assert_eq!(
                unpack_bfs_entry_flags(*entry) & FLAG_WRITE_LEVEL,
                0,
                "no neighbour seed carries FLAG_WRITE_LEVEL"
            );
        }
    }

    #[test]
    fn enqueue_sky_on_block_placed_emits_top_seed_when_topmost() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            1,
            "topmost chunk emits a single top-face seed at y=15"
        );
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 15);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 15);
        assert_ne!(
            unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
            0,
            "top-face seed carries FLAG_WRITE_LEVEL"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_trips_on_occlusion_only_change() {
        const SHAPE_A: BlockStateId = BlockStateId(10);
        const SHAPE_B: BlockStateId = BlockStateId(11);

        let state_count = 12usize;
        let mut emission = vec![0u8; state_count].into_boxed_slice();
        let mut dampening = vec![0u8; state_count].into_boxed_slice();
        let mut occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let mut flags = vec![0u8; state_count].into_boxed_slice();

        emission[AIR.0 as usize] = 0;
        dampening[AIR.0 as usize] = 0;
        flags[AIR.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

        // Two states share dampening and flag bits but project distinct
        // occlusion shapes. `dampening = 5` keeps `PROPAGATES_SKYLIGHT_DOWN`
        // cleared on both (matching the production `compute_flags` invariant)
        // so the dampening and flag arms of `sky_changed` stay silent and the
        // test exclusively exercises the occlusion-shape pointer comparison.
        dampening[SHAPE_A.0 as usize] = 5;
        dampening[SHAPE_B.0 as usize] = 5;
        flags[SHAPE_A.0 as usize] =
            flag_bits::IS_CONDITIONALLY_OPAQUE | flag_bits::IS_NOT_AIR;
        flags[SHAPE_B.0 as usize] =
            flag_bits::IS_CONDITIONALLY_OPAQUE | flag_bits::IS_NOT_AIR;
        occlusion[SHAPE_A.0 as usize] = VoxelShape::empty();
        occlusion[SHAPE_B.0 as usize] = VoxelShape::block();

        let table = BlockLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        };

        assert!(
            !std::ptr::eq(
                table.occlusion_for(SHAPE_A) as *const _,
                table.occlusion_for(SHAPE_B) as *const _,
            ),
            "fixture must mint distinct occlusion shape pointers"
        );
        assert_eq!(table.dampening_for(SHAPE_A), table.dampening_for(SHAPE_B));
        assert_eq!(
            table.flags_for(SHAPE_A) & flag_bits::PROPAGATES_SKYLIGHT_DOWN,
            table.flags_for(SHAPE_B) & flag_bits::PROPAGATES_SKYLIGHT_DOWN,
        );

        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(table);
        app.add_systems(Update, enqueue_sky_light_on_block_placed);
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), SHAPE_A, SHAPE_B),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.decrease_queue.len(),
            1,
            "occlusion-only delta still pushes a decrease seed"
        );
        assert_eq!(
            workspace.increase_queue.len(),
            6,
            "y != 15 path enqueues six neighbour-recheck seeds"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "occlusion-only delta inserts LightDirty"
        );
    }

    fn build_seed_initial_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        // Register all three seed systems with the strict ordering used in
        // plugin.rs: (seed_block_emitters, seed_sky_initial) run together,
        // then `invalidate_previous_topmost` runs after `seed_sky_initial` so
        // the `NeedsRetop` handoff is visible to the consumer.
        app.add_systems(
            Update,
            (
                (seed_block_emitters, seed_sky_initial),
                invalidate_previous_topmost.after(seed_sky_initial),
            ),
        );
        app
    }

    fn spawn_palette_with_torches(positions: &[(i32, i32, i32)]) -> BlockPalette {
        let mut palette = BlockPalette::default();
        palette.fill(AIR);
        for &(x, y, z) in positions {
            palette.set(BlockPos::new(x, y, z), TORCH_HI);
        }
        palette
    }

    fn spawn_dimension(app: &mut App, with_sky: bool) -> bevy_ecs::entity::Entity {
        let mut e = app.world_mut().spawn(ColumnIndex::default());
        if with_sky {
            e.insert(HasSkyLight);
        }
        e.id()
    }

    fn spawn_topmost_chunk_for_seed(
        app: &mut App,
        dim: bevy_ecs::entity::Entity,
        palette: BlockPalette,
        sky: bool,
    ) -> (bevy_ecs::entity::Entity, bevy_ecs::entity::Entity) {
        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let mut emut = app.world_mut().entity_mut(chunk);
        emut.insert((
            palette,
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            BlockNeedsInitialSeed,
        ));
        if sky {
            emut.insert((
                SkyLight::default(),
                SkyLightWorkspace::default(),
                SkyNeedsInitialSeed,
            ));
        }
        (chunk, column)
    }

    #[test]
    fn seed_block_emitters_and_sky_initial_emit_block_emitters_and_sky_source() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);
        let palette = spawn_palette_with_torches(&[
            (0, 0, 0),
            (5, 5, 5),
            (10, 1, 8),
            (3, 12, 7),
            (15, 15, 15),
        ]);
        let (chunk, _col) = spawn_topmost_chunk_for_seed(&mut app, dim, palette, true);

        app.update();

        let block_ws = app
            .world()
            .get::<BlockLightWorkspace>(chunk)
            .expect("block ws");
        assert_eq!(
            block_ws.increase_queue.len(),
            5,
            "five torches emit five increase seeds"
        );
        let sky_ws = app
            .world()
            .get::<SkyLightWorkspace>(chunk)
            .expect("sky ws");
        assert_eq!(
            sky_ws.increase_queue.len(),
            256,
            "topmost on sky-having dim with absent Heightmaps falls back to 256 sky entries"
        );
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(chunk).is_some(),
            "SkyLightSeededAsTopmost inserted"
        );
        assert!(
            app.world().get::<LightDirty>(chunk).is_some(),
            "LightDirty inserted"
        );
        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk).is_none(),
            "BlockNeedsInitialSeed removed by seed_block_emitters"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk).is_none(),
            "SkyNeedsInitialSeed removed by seed_sky_initial"
        );
    }

    /// Regression test: when a new chunk takes over as the column's topmost,
    /// the previous topmost gets the `NeedsRetop` handoff and
    /// `invalidate_previous_topmost` runs the decrease wave through its top
    /// face within the same tick chain.
    #[test]
    fn retopping_handoff_completes_in_one_tick() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);

        // Chunk A at chunk-Y 0 with SkyLightSeededAsTopmost already; stored
        // sky level 12 across the top face.
        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), Some(chunk_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        let mut palette_a = BlockPalette::default();
        palette_a.fill(AIR);
        let mut a_sky_light = SkyLight::default();
        for z in 0..16usize {
            for x in 0..16usize {
                a_sky_light.0.set(x, 15, z, 12);
            }
        }
        app.world_mut().entity_mut(chunk_a).insert((
            palette_a,
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            a_sky_light,
            SkyLightWorkspace::default(),
            SkyLightSeededAsTopmost,
        ));

        // Chunk B at chunk-Y 1 (the new topmost) needs initial light. Both
        // per-channel markers are present to trigger seed_block_emitters and
        // seed_sky_initial.
        let mut palette_b = BlockPalette::default();
        palette_b.fill(AIR);
        app.world_mut().entity_mut(chunk_b).insert((
            palette_b,
            ChunkPos::new(0, 1, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            SkyLight::default(),
            SkyLightWorkspace::default(),
            BlockNeedsInitialSeed,
            SkyNeedsInitialSeed,
        ));

        app.update();

        // Previous topmost A: marker removed, LightDirty inserted, decrease
        // wave seeded with stored level 12.
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(chunk_a).is_none(),
            "previous topmost's SkyLightSeededAsTopmost removed by seed_sky_initial"
        );
        assert!(
            app.world().get::<NeedsRetop>(chunk_a).is_none(),
            "NeedsRetop consumed by invalidate_previous_topmost"
        );
        assert!(
            app.world().get::<LightDirty>(chunk_a).is_some(),
            "previous topmost marked LightDirty"
        );
        let a_ws = app
            .world()
            .get::<SkyLightWorkspace>(chunk_a)
            .expect("sky ws on A");
        assert_eq!(
            a_ws.decrease_queue.len(),
            256,
            "previous topmost gets 256 decrease seeds"
        );
        for entry in &a_ws.decrease_queue {
            assert_eq!(
                unpack_bfs_entry_level(*entry),
                12,
                "decrease seed carries stored level"
            );
            assert_eq!(unpack_bfs_entry_y(*entry) as u8, 15);
        }

        // New topmost B: marker inserted, increase queue seeded.
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(chunk_b).is_some(),
            "new topmost seeded"
        );
        let b_ws = app
            .world()
            .get::<SkyLightWorkspace>(chunk_b)
            .expect("sky ws on B");
        assert_eq!(b_ws.increase_queue.len(), 256);
    }

    #[test]
    fn seed_sky_initial_skips_skyless_dim_for_sky_seed() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, false);
        let palette = spawn_palette_with_torches(&[(2, 2, 2)]);
        // Skyless dim: spawn the chunk without a SkyLight/SkyLightWorkspace
        // (matching the skyless-dimension contract).
        let (chunk, _col) = spawn_topmost_chunk_for_seed(&mut app, dim, palette, false);

        app.update();

        // Block-light emitter seed lands as usual.
        let block_ws = app
            .world()
            .get::<BlockLightWorkspace>(chunk)
            .expect("block ws");
        assert_eq!(block_ws.increase_queue.len(), 1);

        // No sky workspace was attached, so sky pathways are inert.
        assert!(
            app.world().get::<SkyLightWorkspace>(chunk).is_none(),
            "skyless dim has no sky workspace"
        );
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(chunk).is_none(),
            "skyless dim chunk does not insert SkyLightSeededAsTopmost"
        );
        assert!(
            app.world().get::<LightDirty>(chunk).is_some(),
            "chunk still marked LightDirty"
        );
    }

    /// Regression test: the `Added<SkyLight>` fallback
    /// branch of `seed_sky_initial` fires on a topmost-of-column chunk in a
    /// sky-having dim whose column has no primed heightmap. The fallback
    /// pushes 256 seeds even though `SkyNeedsInitialSeed` was never inserted.
    #[test]
    fn seed_sky_initial_fallback_branch_seeds_256_on_partial_load() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);

        // Single topmost chunk; no `SkyNeedsInitialSeed`, no Heightmaps on
        // the column. The `Added<SkyLight>` arm of the filter must fire.
        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let mut palette = BlockPalette::default();
        palette.fill(AIR);
        app.world_mut().entity_mut(chunk).insert((
            palette,
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            SkyLight::default(),
            SkyLightWorkspace::default(),
            // NOTE: no SkyNeedsInitialSeed — the fallback fires on Added<SkyLight>.
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(chunk)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            256,
            "fallback arm seeds 256 entries on topmost-of-column"
        );
        assert!(
            app.world().get::<LightDirty>(chunk).is_some(),
            "LightDirty inserted by the fallback branch"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk).is_none(),
            "marker was never inserted; fallback path does not remove what isn't there"
        );
    }

    fn build_pull_block_neighbor_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, pull_block_neighbor_edges);
        app
    }

    fn build_pull_sky_neighbor_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, pull_sky_neighbor_edges);
        app
    }

    /// Spawns two single-chunk columns at (0,0) and (1,0), wires the
    /// dimension's `ColumnIndex` so `resolve_loaded_neighbor` finds them,
    /// and returns `(column_a, column_b)`. The caller fills in per-chunk
    /// components.
    fn spawn_two_neighbor_columns(
        app: &mut App,
        dim: bevy_ecs::entity::Entity,
        chunk_a: bevy_ecs::entity::Entity,
        chunk_b: bevy_ecs::entity::Entity,
    ) -> (bevy_ecs::entity::Entity, bevy_ecs::entity::Entity) {
        let column_a = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let column_b = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        let mut col_index = app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("column index");
        col_index.0.insert(
            ColumnPos::new(0, 0),
            ColumnSlot {
                entity: column_a,
                section_count: 1,
            },
        );
        col_index.0.insert(
            ColumnPos::new(1, 0),
            ColumnSlot {
                entity: column_b,
                section_count: 1,
            },
        );

        (column_a, column_b)
    }

    #[test]
    fn pull_block_neighbor_edges_pulls_from_loaded_neighbor() {
        let mut app = build_pull_block_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        // Two adjacent columns: column_a at x=0, column_b at x=1, both at z=0.
        // Chunk A in column_a at chunk_pos (0,0,0) with BlockLight Uniform(8).
        // Chunk B in column_b at chunk_pos (1,0,0) with BlockLight Null;
        // when B gets Added<ChunkLoaded>, it should pull face cells from A
        // (A is West of B; from B's frame, light enters via the West face).
        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // Chunk A: already loaded, with uniform block light = 8.
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight(crate::storage::LightStorage::Uniform(8)),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
            ChunkLoaded,
        ));

        // Chunk B: just-loaded; Added<ChunkLoaded> fires on its insertion.
        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
        ));

        // Drain the existing Added<ChunkLoaded> flag for chunk_a by running
        // one tick first with chunk_b not yet ChunkLoaded.
        app.update();

        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("incoming on B");
        assert_eq!(
            incoming.0.len(),
            256,
            "B pulls 16x16 face cells from A (block-light)"
        );
        let west_index = Direction::West.index() as u8;
        for w in incoming.0.iter() {
            assert_eq!(w.face(), west_index, "face index is West (entry from A)");
            assert_eq!(w.level(), 7, "level = 8 - 1 manhattan attenuation");
        }
        assert!(
            app.world().get::<LightDirty>(chunk_b).is_some(),
            "B marked LightDirty (pulled face cells into its incoming)"
        );
        // Pure non-mutating face-cell read on A — no state change on A.
        assert!(
            app.world().get::<LightDirty>(chunk_a).is_none(),
            "neighbour A stays clean — non-mutating face-cell pull is not a state change on A"
        );
    }

    #[test]
    fn pull_sky_neighbor_edges_pulls_from_loaded_neighbor() {
        let mut app = build_pull_sky_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        // Mirror of the block-side test on the sky channel.
        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // Chunk A: already loaded, with uniform sky light = 8.
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            SkyLight(crate::storage::LightStorage::Uniform(8)),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
            ChunkLoaded,
        ));

        // Chunk B: just-loaded; Added<ChunkLoaded> fires on its insertion.
        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
        ));

        app.update();

        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let incoming = app
            .world()
            .get::<SkyIncoming>(chunk_b)
            .expect("sky incoming on B");
        assert_eq!(
            incoming.0.len(),
            256,
            "B pulls 16x16 face cells from A (sky-light)"
        );
        let west_index = Direction::West.index() as u8;
        for w in incoming.0.iter() {
            assert_eq!(w.face(), west_index, "face index is West (entry from A)");
            assert_eq!(w.level(), 7, "level = 8 - 1 manhattan attenuation");
        }
        assert!(
            app.world().get::<LightDirty>(chunk_b).is_some(),
            "B marked LightDirty (pulled face cells into its incoming)"
        );
        assert!(
            app.world().get::<LightDirty>(chunk_a).is_none(),
            "neighbour A stays clean — non-mutating face-cell pull is not a state change on A"
        );
    }

    #[test]
    fn pull_block_neighbor_edges_drains_pending_egress_on_load() {
        let mut app = build_pull_block_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // A is West of B. From A's frame, the East face (index 5) points
        // toward B. So A's BlockPendingEgress entry with face=East addresses
        // B; the pull system should drain it.
        let east_index = Direction::East.index() as u8;
        let mut pending = BlockPendingEgress::default();
        pending.0.push(Wavefront::new(east_index, 3, 5, 9));

        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight::default(),
            pending,
            BlockIncoming::default(),
            ChunkLoaded,
        ));

        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
        ));

        // Tick once to consume the initial Added<ChunkLoaded> on A.
        app.update();

        let a_pending_before = app
            .world()
            .get::<BlockPendingEgress>(chunk_a)
            .expect("pending on A");
        assert_eq!(
            a_pending_before.0.len(),
            1,
            "pending entry survives first tick"
        );

        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let a_pending_after = app
            .world()
            .get::<BlockPendingEgress>(chunk_a)
            .expect("pending on A");
        assert!(
            a_pending_after.0.is_empty(),
            "A's pending egress drained after B loaded"
        );

        let b_incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("incoming on B");
        let west_index = Direction::West.index() as u8;
        let drained = b_incoming
            .0
            .iter()
            .find(|w| w.cell_x() == 3 && w.cell_z() == 5 && w.level() == 9);
        assert!(
            drained.is_some(),
            "drained pending wavefront landed in B's incoming"
        );
        assert_eq!(drained.unwrap().face(), west_index);

        assert!(
            app.world().get::<LightDirty>(chunk_a).is_some(),
            "A marked LightDirty"
        );
    }

    /// Block-channel asymmetry: there is no `Uniform(15)`-neighbour escape
    /// hatch on the block side (Assumption A2 — no block-light fast-path
    /// produces `Uniform(15)` at seed time). Two chunks loading in the same
    /// tick must NOT pull from each other, even if one neighbour happens to
    /// carry a hand-authored `Uniform(15)` block-light value.
    #[test]
    fn pull_block_neighbor_skips_newly_loaded_neighbor() {
        let mut app = build_pull_block_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // Chunk A: hand-authored `Uniform(15)` block light. In production no
        // seed-time fast-path ever produces this for block channel, but the
        // test fixture sets it to confirm the system still skips A because
        // A is `Added<ChunkLoaded>` this tick.
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight(crate::storage::LightStorage::Uniform(15)),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
        ));

        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
        ));

        // Both chunks land in newly_loaded_set in the same tick.
        app.world_mut().entity_mut(chunk_a).insert(ChunkLoaded);
        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let b_incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("incoming on B");
        assert!(
            b_incoming.0.is_empty(),
            "B must NOT receive face cells from A — block channel has no Uniform(15) escape hatch"
        );
        assert!(
            app.world().get::<LightDirty>(chunk_b).is_none(),
            "B has no incoming wavefronts, so no LightDirty marker should be inserted"
        );
    }

    fn build_consume_needs_full_reseed_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, consume_needs_full_reseed);
        app
    }

    #[test]
    fn consume_needs_full_reseed_marks_all_loaded_chunks_when_scan_finalized() {
        let mut app = build_consume_needs_full_reseed_app();

        // Mint a sky-having dimension so each chunk's `InDimension` lookup
        // resolves to a `HasSkyLight` carrier and the per-channel
        // `SkyNeedsInitialSeed` marker is re-inserted alongside the block one.
        let dim = app.world_mut().spawn(HasSkyLight).id();
        let chunk_a = app.world_mut().spawn(InDimension(dim)).id();
        let chunk_b = app.world_mut().spawn(InDimension(dim)).id();
        let chunk_unloaded_slot: Option<bevy_ecs::entity::Entity> = None;
        // Attach a finalized scan so the system treats the heightmap as
        // primed and proceeds with the reseed.
        let mut scan = crate::lifecycle::ColumnHeightmapScan::new(0, 2);
        scan.scan_cursor = -1;
        assert!(scan.is_finalized());
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), chunk_unloaded_slot, Some(chunk_b)]
                        .into_boxed_slice(),
                },
                scan,
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_a).is_some(),
            "chunk A re-marked BlockNeedsInitialSeed"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk_a).is_some(),
            "chunk A re-marked SkyNeedsInitialSeed (sky-having dim)"
        );
        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_b).is_some(),
            "chunk B re-marked BlockNeedsInitialSeed"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk_b).is_some(),
            "chunk B re-marked SkyNeedsInitialSeed (sky-having dim)"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed removed from column"
        );
    }

    /// Regression: when the column's heightmap scan is not yet finalized
    /// (sentinel reads), `consume_needs_full_reseed` must DROP the reseed
    /// instead of re-marking chunks. Re-marking now would cause
    /// `seed_sky_initial` to read sentinel `min_y` and misclassify cave
    /// chunks as Case A (Uniform(15)). The natural lifecycle in
    /// `prime_heightmaps_on_column_spawn` inserts the per-channel markers
    /// once the scan closes.
    #[test]
    fn consume_needs_full_reseed_drops_reseed_when_scan_not_finalized() {
        let mut app = build_consume_needs_full_reseed_app();

        let dim = app.world_mut().spawn(HasSkyLight).id();
        let chunk_a = app.world_mut().spawn(InDimension(dim)).id();
        let chunk_b = app.world_mut().spawn(InDimension(dim)).id();
        // No ColumnHeightmapScan attached → unfinalized.
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), None, Some(chunk_b)]
                        .into_boxed_slice(),
                },
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_a).is_none(),
            "chunk A must NOT be re-marked when scan unfinalized"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk_a).is_none(),
            "chunk A sky marker must NOT be re-marked when scan unfinalized"
        );
        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_b).is_none(),
            "chunk B must NOT be re-marked when scan unfinalized"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed cleared from column"
        );
    }

    /// A scan present but still mid-scan (not finalized) must also drop the
    /// reseed. Same rationale as the no-scan case.
    #[test]
    fn consume_needs_full_reseed_drops_reseed_when_scan_mid_progress() {
        let mut app = build_consume_needs_full_reseed_app();

        let dim = app.world_mut().spawn(HasSkyLight).id();
        let chunk_a = app.world_mut().spawn(InDimension(dim)).id();
        // Mid-scan: cursor still at top of range, no bits closed.
        let scan = crate::lifecycle::ColumnHeightmapScan::new(0, 2);
        assert!(!scan.is_finalized());
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), None, None].into_boxed_slice(),
                },
                scan,
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_a).is_none(),
            "chunk must NOT be re-marked when scan is mid-progress"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed cleared from column"
        );
    }

    #[test]
    fn pull_sky_neighbor_pulls_uniform_15_neighbor_even_when_newly_loaded() {
        // Regression test for the case where a Case-A (Uniform(15)) neighbour
        // and a dark (Case-B) chunk both receive Added<ChunkLoaded> in the
        // same tick. An unconditional skip on newly-loaded neighbours would
        // leave the dark chunk at 0 because A's level-15 face cells never
        // reach it. The escape hatch in `pull_sky_neighbor_edges` lets the
        // pull fire when the neighbour is already settled `Uniform(15)`.
        let mut app = build_pull_sky_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // Chunk A: Case A — sky light already at Uniform(15) (the heightmap
        // fast-path outcome from seed_sky_initial, observable here before
        // the pull system runs).
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            SkyLight(crate::storage::LightStorage::Uniform(15)),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
        ));

        // Chunk B: Case B — sky light starts at Null (dark), has a
        // SkyIncoming buffer for the pull to write into.
        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
        ));

        // Insert ChunkLoaded on both in the same tick so both land in
        // newly_loaded_set. The pull system runs once after both inserts.
        app.world_mut().entity_mut(chunk_a).insert(ChunkLoaded);
        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let incoming = app
            .world()
            .get::<SkyIncoming>(chunk_b)
            .expect("sky incoming on B");
        assert_eq!(
            incoming.0.len(),
            256,
            "B must receive 256 sky-light face-cell entries from A (16x16 at level 14)"
        );
        let west_index = Direction::West.index() as u8;
        for w in incoming.0.iter() {
            assert_eq!(
                w.face(),
                west_index,
                "all entries enter from the West face (A is West of B)"
            );
            assert_eq!(
                w.level(),
                14,
                "level = 15 - 1 manhattan attenuation"
            );
        }
        assert!(
            app.world().get::<LightDirty>(chunk_b).is_some(),
            "B must be marked LightDirty so the BFS converge loop runs"
        );
    }
}
