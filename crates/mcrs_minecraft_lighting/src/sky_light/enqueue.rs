//! Sky-light enqueue systems. The implementations currently live in
//! `crate::enqueue`; this module re-exports the sky-side surface
//! (including the column-walker fast path for partial-load folds) so
//! callers can land on `crate::sky_light::enqueue::*` as the canonical
//! path. A future refactor will move the bodies here.

use bevy_ecs::change_detection::Res;
use bevy_ecs::prelude::{Added, Commands, Local, Or, ParallelCommands, Query, With, Without};
use bevy_ecs::entity::{Entity, EntityHashMap};
use bevy_ecs::message::MessageReader;
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::geometry::ChunkPos;
use mcrs_engine::world::chunk::ChunkLoaded;
use mcrs_engine::world::column::{ColumnChunks, ColumnIndex, Heightmaps, InColumn};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;
use crate::{CrossChunkWavefront, SkyBfsPending, SkyBfsQueues, SkyInbox, SkyLight, SkyNeedsInitialSeed, SkyParkedEgress, WasTopmostAtSeed};
use crate::bfs::{normal_of, pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_RECHECK_LEVEL, FLAG_WRITE_LEVEL};
use crate::codec::LightStorage;
use crate::distribute::{resolve_neighbor_chunk, ResolveOutcome};
use crate::enqueue::CARDINAL_DIRECTIONS;
use crate::geom::face_cell_to_chunk_xyz;
use crate::heightmap::topmost_surface_world_y;
use crate::nibble::LightNibbles;
use crate::sky_light::components::NeedsRetop;
use crate::table::{flag_bits, BlockStateLightTable};

/// Reacts to `BlockPlaced` by enqueuing sky-light decrease and increase seeds
/// whenever the placed block changes either its dampening or its
/// `PROPAGATES_SKYLIGHT_DOWN` flag.
///
/// Missing `SkyLight`/`SkyBfsQueues` components on the target chunk
/// emit a `tracing::warn!` and skip without panic; this defends against
/// `BlockPlaced` reaching a skyless-dim chunk (where the bundle is never
/// attached) or arriving before the lighting bundle insertion has flushed.
pub fn enqueue_sky_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockStateLightTable>,
    mut chunks: Query<(
        Entity,
        &mut SkyLight,
        &mut SkyBfsQueues,
        &ChunkPos,
        &InColumn,
    )>,
    chunks_lookup: Query<(), (With<SkyLight>, With<SkyBfsQueues>)>,
    columns: Query<&ColumnChunks>,
    mut partitions: Local<EntityHashMap<Vec<BlockPlaced>>>,
    par_commands: ParallelCommands,
) {
    partitions.retain(|_, bucket| {
        if bucket.is_empty() {
            false
        } else {
            bucket.clear();
            true
        }
    });

    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }
        partitions.entry(placed.chunk).or_default().push(*placed);
    }

    let partitions_ref = &*partitions;
    let columns_ref = &columns;
    chunks
        .par_iter_mut()
        .for_each(|(entity, mut light, mut queues, chunk_pos, in_column)| {
            let Some(events) = partitions_ref.get(&entity) else {
                return;
            };
            if events.is_empty() {
                return;
            }

            // Resolve topmost-of-column once per task; the column's section
            // count does not change within a tick, so caching here avoids N
            // redundant ColumnChunks reads on a section that receives multiple
            // BlockPlaced events.
            let is_topmost = match columns_ref.get(in_column.0) {
                Ok(chunk_index) => {
                    let top_chunk_y =
                        chunk_index.min_section_y + chunk_index.sections.len() as i32 - 1;
                    chunk_pos.y == top_chunk_y
                }
                Err(_) => false,
            };

            let mut pushed = false;

            for placed in events {
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

                let x = placed.block_pos.x.rem_euclid(16) as u8;
                let y = placed.block_pos.y.rem_euclid(16) as u8;
                let z = placed.block_pos.z.rem_euclid(16) as u8;

                let stored = light.0.get(x as usize, y as usize, z as usize);
                let opacity_rose = new_dampening > old_dampening
                    || ((old_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN) != 0
                        && (new_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN) == 0);
                // The decrease BFS only walks neighbours, so the seed cell
                // must be cleared up front whenever the post-change opacity
                // can only fall below `stored`; otherwise the source position
                // keeps its previous sky-light level even though the new block
                // opaquifies the cell.
                if opacity_rose && stored > 0 {
                    light.0.set(x as usize, y as usize, z as usize, 0);
                }
                queues.decrease_queue.push(pack_bfs_entry(
                    x,
                    z,
                    y,
                    stored,
                    ALL_DIRECTIONS_BITSET,
                    0,
                ));

                if y == 15 && is_topmost {
                    queues.increase_queue.push(pack_bfs_entry(
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
                        queues.increase_queue.push(pack_bfs_entry(
                            nx as u8,
                            nz as u8,
                            ny as u8,
                            neighbour_level,
                            ALL_DIRECTIONS_BITSET,
                            FLAG_RECHECK_LEVEL,
                        ));
                    }
                }

                pushed = true;
            }

            if pushed {
                par_commands.command_scope(|mut cmd| {
                    cmd.entity(entity).insert(SkyBfsPending);
                });
            }
        });

    for (entity, events) in partitions.iter() {
        if events.is_empty() {
            continue;
        }
        if chunks_lookup.get(*entity).is_err() {
            let first = events.first().unwrap();
            tracing::warn!(
                chunk = ?entity,
                block_pos = ?first.block_pos,
                "BlockPlaced.chunk missing SkyLight/SkyBfsQueues; skipping sky enqueue"
            );
        }
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
/// body's `LightStorage::Empty` self-gate keeps the fallback from re-seeding
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
    table: Option<Res<BlockStateLightTable>>,
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
            &mut SkyBfsQueues,
            &mut SkyLight,
            Option<&SkyNeedsInitialSeed>,
            Option<&WasTopmostAtSeed>,
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
        _topmost_marker_opt,
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
                        // form and skip SkyBfsPending — there is no work to
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
                        let mut arr = LightNibbles::filled(15);
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
                        sky_light.0 = LightStorage::Dense(Box::new(arr));
                    }

                    if is_topmost {
                        commands
                            .entity(chunk_entity)
                            .insert(WasTopmostAtSeed);
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
                            .insert(WasTopmostAtSeed);
                        sky_seeded = true;
                        seeded_topmost = true;
                    }
                }
            }
        } else {
            // Marker-absent + Added<SkyLight> branch: the 256-seed
            // partial-load fallback. Fires once per chunk lifetime
            // (Added<SkyLight> is single-shot). The `LightStorage::Empty`
            // self-gate prevents double-seeding when any earlier branch has
            // already written storage.
            if !matches!(sky_light.0, LightStorage::Empty) {
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
            commands.entity(chunk_entity).insert(SkyBfsPending);
        }
        if marker_opt.is_some() {
            commands
                .entity(chunk_entity)
                .remove::<SkyNeedsInitialSeed>();
        }
        if seeded_topmost {
            retop_targets.push(chunk_entity);
        }
    }

    // For each chunk that just became the new topmost, locate any prior
    // topmost (in the same column at a lower chunk_pos.y) and tag it with
    // `NeedsRetop` + `SkyBfsPending`. The decrease wave itself runs in
    // `invalidate_previous_topmost` after the `apply_deferred` barrier.
    if !retop_targets.is_empty() {
        // We need access to (Entity, ChunkPos, InColumn, WasTopmostAtSeed)
        // for every chunk in the affected columns. The producer side does not
        // need to read the previous-topmost queues — only its identity —
        // so the query stays read-only on entity components and writes happen
        // via Commands.
        //
        // Re-derive the column/new_chunk_y pair from the freshly-completed
        // iteration above by recovering data through `chunks.get` (the query
        // is read-only here so the &mut SkyLight borrow is dropped).
        for new_topmost_entity in &retop_targets {
            let Ok((_, _, new_in_col, _, new_chunk_pos, _, _, _, _)) =
                chunks.get(*new_topmost_entity)
            else {
                continue;
            };
            let new_column = new_in_col.0;
            let new_chunk_y = new_chunk_pos.y;
            // Walk the column's chunk_index sections and find any loaded slot
            // below new_chunk_y. If the slot matches our query filter we read
            // `WasTopmostAtSeed` and tag only the actual predecessor;
            // otherwise (the mask in `prime_heightmaps_on_column_spawn` didn't
            // re-insert `SkyNeedsInitialSeed` on this slot) we fall back to a
            // conservative tag and rely on `invalidate_previous_topmost`'s
            // `With<SkyLight>` filter plus its non-sky cleanup pass to drop
            // spurious markers.
            let Ok(chunk_index) = chunk_indexes.get(new_column) else {
                continue;
            };
            for (section_idx, slot) in chunk_index.sections.iter().enumerate() {
                let Some(slot_entity) = slot else { continue };
                if *slot_entity == *new_topmost_entity {
                    continue;
                }
                let slot_chunk_y = chunk_index.min_section_y + section_idx as i32;
                if slot_chunk_y >= new_chunk_y {
                    continue;
                }
                match chunks.get(*slot_entity) {
                    Ok((_, _, slot_in_col, _, _, _, _, _, slot_topmost_marker)) => {
                        if slot_in_col.0 != new_column {
                            continue;
                        }
                        if slot_topmost_marker.is_none() {
                            continue;
                        }
                        commands
                            .entity(*slot_entity)
                            .remove::<WasTopmostAtSeed>()
                            .insert(NeedsRetop)
                            .insert(SkyBfsPending);
                    }
                    Err(_) => {
                        commands
                            .entity(*slot_entity)
                            .remove::<WasTopmostAtSeed>()
                            .insert(NeedsRetop)
                            .insert(SkyBfsPending);
                    }
                }
            }
        }
    }
}

/// Sky-light half of the per-channel chunk-edge pull. Mirror of
/// `pull_block_neighbor_edges` operating on `SkyLight` / `SkyParkedEgress`
/// / `SkyInbox`. Marks every touched loaded neighbour and the new chunk
/// itself with `SkyBfsPending` when something actually moved.
///
/// Retains the sky-only escape hatch: if a newly-loaded neighbour already
/// holds `LightStorage::Uniform(15)` (the Case A heightmap fast-path
/// outcome, written by `seed_sky_initial`), pull from it even though it
/// was also `Added<ChunkLoaded>` this tick. The neighbour's storage is
/// final and must be observed now so that a dark (Case B/C) new chunk
/// receives the correct initial wavefront at its shared face. For all
/// other newly-loaded neighbours the outbox→distribute cascade routes any
/// flow during convergence.
pub fn pull_sky_neighbor_edges(
    table: Option<Res<BlockStateLightTable>>,
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension, &InColumn), Added<ChunkLoaded>>,
    column_indexes: Query<&ColumnIndex>,
    chunk_indexes: Query<&ColumnChunks>,
    sky_light_read: Query<&SkyLight>,
    mut sky_parked: Query<&mut SkyParkedEgress>,
    mut sky_inbox: Query<&mut SkyInbox>,
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
            let Some(ResolveOutcome::Loaded { dst_entity: neighbour_entity, .. }) =
                resolve_neighbor_chunk(
                    *chunk_pos,
                    *in_col,
                    *in_dim,
                    face,
                    &column_indexes,
                    &chunk_indexes,
                )
            else {
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
                                if let Ok(mut inc) = sky_inbox.get_mut(new_chunk) {
                                    inc.0.push(CrossChunkWavefront::new(
                                        dest_face, cell_a, cell_b, attenuated,
                                    ));
                                    new_chunk_has_incoming = true;
                                }
                            }
                        }
                    }
                }
            }

            if let Ok(mut parked) = sky_parked.get_mut(neighbour_entity) {
                if !parked.0.is_empty() {
                    parked.0.retain(|w| {
                        if w.face() == neighbour_expected_face {
                            if let Ok(mut inc) = sky_inbox.get_mut(new_chunk) {
                                inc.0.push(CrossChunkWavefront::new(
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
                commands.entity(neighbour_entity).insert(SkyBfsPending);
            }
        }

        if new_chunk_has_incoming {
            commands.entity(new_chunk).insert(SkyBfsPending);
        }
    }
}
/// Consumer half of the retopping handoff. Runs the per-cell decrease-queue
/// push body; populates the previous topmost chunk's
/// `SkyBfsQueues.decrease_queue` with 256 entries carrying the chunk's
/// stored top-face sky levels. Removes the `NeedsRetop` marker at the end so
/// the system is idempotent.
///
/// Filter `With<NeedsRetop>` is sufficient because every previous-topmost that
/// should be invalidated has been tagged by `seed_sky_initial`. Scheduling
/// requirement: `invalidate_previous_topmost.after(seed_sky_initial)` so the
/// producer's `commands.insert(NeedsRetop)` is visible after the
/// `apply_deferred` barrier between `LightingSet::Enqueue` substages.
///
/// Cleanup pass: the producer's Err-branch fallback tags column-mates without
/// being able to verify they carry `SkyLight`. The `prev_chunks` query's
/// `With<SkyLight>` filter drops those silently, so without a cleanup pass
/// the `NeedsRetop` marker would persist on non-sky entities indefinitely.
/// `orphan_chunks` strips the marker from any chunk that lacks `SkyLight`.
pub(crate) fn invalidate_previous_topmost(
    mut prev_chunks: Query<
        (Entity, &ChunkPos, &InColumn, &mut SkyBfsQueues, &SkyLight),
        With<NeedsRetop>,
    >,
    orphan_chunks: Query<Entity, (With<NeedsRetop>, Without<SkyLight>)>,
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
        commands.entity(prev_entity).insert(SkyBfsPending);
    }
    for orphan_entity in orphan_chunks.iter() {
        commands.entity(orphan_entity).remove::<NeedsRetop>();
    }
}
