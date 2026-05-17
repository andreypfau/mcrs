//! Block-light enqueue systems. The implementations currently live in
//! `crate::enqueue`; this module re-exports the block-side surface so
//! callers can land on `crate::block_light::enqueue::*` as the
//! canonical path. A future refactor will move the bodies here.

use bevy_ecs::change_detection::Res;
use bevy_ecs::prelude::{Added, Commands, Local, ParallelCommands, Query, With};
use bevy_ecs::entity::{Entity, EntityHashMap};
use bevy_ecs::message::MessageReader;
use mcrs_engine::geometry::{BlockPos, ChunkPos};
use mcrs_engine::world::chunk::ChunkLoaded;
use mcrs_engine::world::column::{ColumnChunks, ColumnIndex, InColumn};
use mcrs_engine::world::dimension::InDimension;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;
use crate::{BlockBfsPending, BlockBfsQueues, BlockInbox, BlockLight, BlockNeedsInitialSeed, BlockParkedEgress, CrossChunkWavefront};
use crate::bfs::{pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_WRITE_LEVEL};
use crate::distribute::{resolve_neighbor_chunk, ResolveOutcome};
use crate::enqueue::CARDINAL_DIRECTIONS;
use crate::geom::face_cell_to_chunk_xyz;
use crate::storage::LightStorage;
use crate::table::BlockStateLightTable;

pub fn enqueue_block_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockStateLightTable>,
    mut chunks: Query<(Entity, &mut BlockLight, &mut BlockBfsQueues)>,
    chunks_lookup: Query<(), (With<BlockLight>, With<BlockBfsQueues>)>,
    mut partitions: Local<EntityHashMap<Vec<BlockPlaced>>>,
    par_commands: ParallelCommands,
) {
    // Drop empty buckets to bound memory under long-running sessions where the
    // touched-chunks set could grow unboundedly; clear non-empty ones in place
    // to amortise the Vec allocation across consecutive ticks that touch the
    // same chunk.
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

    // One task per chunk entity; the per-entity body owns the entire bucket of
    // events for that chunk so two parallel tasks never touch the same queues.
    let partitions_ref = &*partitions;
    chunks
        .par_iter_mut()
        .for_each(|(entity, mut light, mut queues)| {
            let Some(events) = partitions_ref.get(&entity) else {
                return;
            };
            if events.is_empty() {
                return;
            }

            let mut pushed = false;

            for placed in events {
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

                if old_emission > new_emission {
                    // The decrease BFS only walks neighbours, so the seed cell
                    // itself must be cleared up front; otherwise the source
                    // position keeps its previous emitted level after the
                    // emitter is removed.
                    light.0.set(x as usize, y as usize, z as usize, 0);

                    queues.decrease_queue.push(pack_bfs_entry(
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
                    // `new_emission` before stepping outward, so the source
                    // position is established before any neighbour is reached.
                    queues.increase_queue.push(pack_bfs_entry(
                        x,
                        z,
                        y,
                        new_emission,
                        ALL_DIRECTIONS_BITSET,
                        FLAG_WRITE_LEVEL,
                    ));
                    pushed = true;
                }
            }

            if pushed {
                par_commands.command_scope(|mut cmd| {
                    cmd.entity(entity).insert(BlockBfsPending);
                });
            }
        });

    // Emit the lifecycle warning for chunks whose entities don't match the
    // queues query at all. The par_iter_mut body cannot do this because the
    // query filter excludes those entities, and tracing inside a parallel task
    // would interleave warning lines from different entities.
    for (entity, events) in partitions.iter() {
        if events.is_empty() {
            continue;
        }
        if chunks_lookup.get(*entity).is_err() {
            // Surface the first event's block_pos so the warning carries a
            // concrete coordinate (matches the previous per-event behaviour
            // closely enough for diagnostics).
            let first = events.first().unwrap();
            tracing::warn!(
                chunk = ?entity,
                block_pos = ?first.block_pos,
                "BlockPlaced.chunk missing BlockLight/BlockBfsQueues; lifecycle ordering hazard"
            );
        }
    }
}

/// Block-channel half of the seed-initial split. Scans the chunk's palette
/// for block-light emitters and seeds `BlockBfsQueues::increase_queue`
/// accordingly. Filter `With<BlockNeedsInitialSeed>` self-gates the system:
/// the marker is present only on chunks awaiting their initial block-light
/// seed; the marker also implies the chunk passed through
/// `attach_lighting_state`, which is the only path that inserts the
/// block-light bundle (so `With<BlockLight>` would be redundant). The marker
/// is always removed at the end of the body, so the system is idempotent
/// across ticks.
pub fn seed_block_emitters(
    table: Option<Res<BlockStateLightTable>>,
    mut chunks: Query<
        (Entity, &BlockPalette, &mut BlockBfsQueues),
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
            commands.entity(chunk_entity).insert(BlockBfsPending);
        }
        commands
            .entity(chunk_entity)
            .remove::<BlockNeedsInitialSeed>();
    }
}

/// Block-light half of the per-channel chunk-edge pull. Consumes
/// `Added<ChunkLoaded>` per chunk: reads each loaded cardinal neighbour's
/// face-cell `BlockLight` values into the new chunk's `BlockInbox`, then
/// drains any `BlockParkedEgress` entries that the neighbour buffered
/// while we were unloaded. Marks the new chunk and every touched loaded
/// neighbour `BlockBfsPending` when something actually moved.
///
/// Scheduled in parallel with `pull_sky_neighbor_edges`. The two systems
/// take disjoint `&BlockLight`/`&SkyLight` reads and disjoint
/// `&mut BlockParkedEgress`/`&mut SkyParkedEgress` writes, so Bevy's
/// conflict graph slots them simultaneously.
///
/// Unlike the sky channel, this system unconditionally skips a neighbour
/// that was also `Added<ChunkLoaded>` this tick. There is no block-light
/// fast-path that produces `LightStorage::Uniform(15)` at seed time (the
/// `Uniform(15)` heightmap fast-path is sky-only), so an `Uniform(15)`-
/// neighbour escape hatch would never legitimately fire and would risk
/// pulling from hand-authored uniforms that have not yet settled.
pub fn pull_block_neighbor_edges(
    table: Option<Res<BlockStateLightTable>>,
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension, &InColumn), Added<ChunkLoaded>>,
    column_indexes: Query<&ColumnIndex>,
    chunk_indexes: Query<&ColumnChunks>,
    block_light_read: Query<&BlockLight>,
    mut block_parked: Query<&mut BlockParkedEgress>,
    mut block_inbox: Query<&mut BlockInbox>,
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

            // Block channel: no `Uniform(15)`-neighbour escape hatch (sky
            // has one because the Case A heightmap fast-path writes
            // `Uniform(15)` skies at seed time). Always skip newly-loaded
            // neighbours; the natural outbox→distribute cascade routes
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
                                if let Ok(mut inc) = block_inbox.get_mut(new_chunk) {
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

            if let Ok(mut parked) = block_parked.get_mut(neighbour_entity) {
                if !parked.0.is_empty() {
                    parked.0.retain(|w| {
                        if w.face() == neighbour_expected_face {
                            if let Ok(mut inc) = block_inbox.get_mut(new_chunk) {
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
                commands.entity(neighbour_entity).insert(BlockBfsPending);
            }
        }

        if new_chunk_has_incoming {
            commands.entity(new_chunk).insert(BlockBfsPending);
        }
    }
}