//! Sky-light BFS wrappers, including the column-walker fast path for
//! all-air chunks. Each system iterates `Query<..., With<SkyBfsPending>>`
//! in parallel via `par_iter_mut`; per-worker `Commands` accumulation goes
//! through `ParallelCommands`.

use bevy_ecs::change_detection::Res;
use bevy_ecs::prelude::{ParallelCommands, Query, With};
use bevy_ecs::entity::Entity;
use mcrs_core::voxel_shape::Direction;
use mcrs_minecraft_block::palette::BlockPalette;
use crate::bfs::{propagate_decrease_sky, propagate_increase_sky, unpack_bfs_entry_level, unpack_bfs_entry_y};
use crate::codec::LightStorage;
use crate::propagate::drain_incoming_into_queue;
use crate::table::BlockStateLightTable;
use crate::{CrossChunkWavefront, IsAllAir, SkyBfsPending, SkyBfsQueues, SkyInbox, SkyLight, SkyOutbox, SkyOutboxDirty};

/// Five non-Up faces used by the column-walker fast path to dump 256
/// wavefronts per face onto `SkyOutbox` (1280 entries total) when an
/// `IsAllAir` chunk short-circuits the BFS.
pub(crate) const COLUMN_WALKER_FACES: [Direction; 5] = [
    Direction::Down,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// Column-walker predicate: an all-air chunk whose only queued work is the
/// 256 top-face level-15 seeds is advanced in O(1) by writing
/// `LightStorage::Uniform(15)` and dumping wavefronts onto the five non-Up
/// faces, instead of running the per-cell BFS.
///
/// All three conditions must hold:
/// - `is_all_air` is true,
/// - `queues.decrease_queue` is empty,
/// - every entry in `queues.increase_queue` is at y=15 with level=15.
pub(crate) fn try_column_walker_fast_path(is_all_air: bool, queues: &SkyBfsQueues) -> bool {
    if !is_all_air {
        return false;
    }
    if !queues.decrease_queue.is_empty() {
        return false;
    }
    if queues.increase_queue.is_empty() {
        return false;
    }
    queues.increase_queue.iter().all(|&e| {
        let y = (unpack_bfs_entry_y(e) as usize) & 0xF;
        let lvl = unpack_bfs_entry_level(e);
        y == 15 && lvl == 15
    })
}

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "lighting::propagate_decrease_sky", skip_all, fields(chunk_count = tracing::field::Empty))
)]
pub fn propagate_decrease_sky_system(
    table: Res<BlockStateLightTable>,
    mut chunks: Query<
        (
            Entity,
            &BlockPalette,
            &mut SkyLight,
            &mut SkyBfsQueues,
            &mut SkyOutbox,
            &mut SkyInbox,
        ),
        With<SkyBfsPending>,
    >,
    commands: ParallelCommands,
) {
    #[cfg(feature = "telemetry-tracy")]
    tracing::Span::current().record("chunk_count", chunks.iter().count());
    chunks.par_iter_mut().for_each(
        |(entity, palette, mut light, mut queues, mut outbox, mut inbox)| {
            drain_incoming_into_queue(&mut inbox.0, &mut queues.increase_queue);
            propagate_decrease_sky(&table, palette, &mut light.0, &mut queues, &mut outbox);
            if !outbox.0.is_empty() {
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).insert(SkyOutboxDirty);
                });
            }
            #[cfg(feature = "telemetry-tracy")]
            tracing::debug!(chunk = ?entity, queue_len = queues.decrease_queue.len(), "chunk bfs decrease sky");
        },
    );
}

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "lighting::propagate_increase_sky", skip_all, fields(chunk_count = tracing::field::Empty))
)]
pub fn propagate_increase_sky_system(
    table: Res<BlockStateLightTable>,
    mut chunks: Query<
        (
            Entity,
            &BlockPalette,
            &mut SkyLight,
            &mut SkyBfsQueues,
            &mut SkyOutbox,
            &mut SkyInbox,
            Option<&IsAllAir>,
        ),
        With<SkyBfsPending>,
    >,
    commands: ParallelCommands,
) {
    #[cfg(feature = "telemetry-tracy")]
    tracing::Span::current().record("chunk_count", chunks.iter().count());
    chunks.par_iter_mut().for_each(
        |(entity, palette, mut light, mut queues, mut outbox, mut inbox, is_all_air)| {
            drain_incoming_into_queue(&mut inbox.0, &mut queues.increase_queue);

            if try_column_walker_fast_path(is_all_air.is_some(), &queues) {
                light.0 = LightStorage::Uniform(15);
                // SmallVec inline capacity is 16; reserve up front so the 1280
                // per-cell pushes below collapse to a single heap allocation
                // instead of multiple incremental reallocations.
                outbox.0.reserve(1280);
                // Per-face (cell_x, cell_z) pairing follows the chunk_xyz_to_face_cell
                // axis contract: Y-normal faces drop y, Z-normal faces drop z and
                // pack (x, y), X-normal faces drop x and pack (y, z).
                for face in COLUMN_WALKER_FACES {
                    let face_idx = face.index() as u8;
                    for a in 0..16u8 {
                        for b in 0..16u8 {
                            let (cx, cz) = match face {
                                Direction::Down | Direction::Up => (b, a),
                                Direction::North | Direction::South => (b, a),
                                Direction::West | Direction::East => (a, b),
                            };
                            outbox.0.push(CrossChunkWavefront::new(face_idx, cx, cz, 15));
                        }
                    }
                }
                queues.increase_queue.clear();
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).insert(SkyOutboxDirty);
                    if queues.decrease_queue.is_empty() {
                        cmd.entity(entity).remove::<SkyBfsPending>();
                    }
                });
                return;
            }

            propagate_increase_sky(&table, palette, &mut light.0, &mut queues, &mut outbox);
            if !outbox.0.is_empty() {
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).insert(SkyOutboxDirty);
                });
            }
            if queues.increase_queue.is_empty() && queues.decrease_queue.is_empty() {
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).remove::<SkyBfsPending>();
                });
            }
            #[cfg(feature = "telemetry-tracy")]
            tracing::debug!(chunk = ?entity, queue_len = queues.increase_queue.len(), "chunk bfs increase sky");
        },
    );
}
