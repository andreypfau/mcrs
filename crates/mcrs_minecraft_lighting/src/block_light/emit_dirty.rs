//! Block-light emit-dirty system and safety-net sweep. The
//! implementations currently live in `crate::emit_dirty`; this module
//! re-exports the block-side surface so callers can land on
//! `crate::block_light::emit_dirty::*` as the canonical path.

use bevy_ecs::prelude::{Changed, Commands, Query, With};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use mcrs_engine::world::column::{ColumnChunks, ColumnPosComponent, InColumn};
use crate::{emit_dirty, BlockBfsPending, BlockBfsQueues, BlockInbox, BlockLight, BlockLightDirty, BlockOutbox};

/// Removes `BlockBfsPending` from chunks whose block-channel outbox,
/// inbox, and queues queues are all empty. Emits `tracing::debug!`
/// each time it clears anything — every clear indicates a leftover
/// `BlockBfsPending` that the per-iteration clear inside
/// `propagate_increase_block_system` missed. Scheduled in parallel with
/// its sky-channel mirror under disjoint component access.
pub fn clear_block_bfs_pending_safety_net(
    chunks: Query<
        (Entity, &BlockOutbox, &BlockInbox, &BlockBfsQueues),
        With<BlockBfsPending>,
    >,
    mut commands: Commands,
) {
    for (entity, be, bi, bws) in chunks.iter() {
        if be.0.is_empty()
            && bi.0.is_empty()
            && bws.increase_queue.is_empty()
            && bws.decrease_queue.is_empty()
        {
            commands.entity(entity).remove::<BlockBfsPending>();
            tracing::debug!(?entity, "BlockBfsPending safety-net cleared");
        }
    }
}

// Producer half of the lighting codec wire. Filtered on `Changed<BlockLight>`
// (sky-layer counterpart filters on `Changed<SkyLight>`) so a chunk is
// announced whenever its light storage was `&mut`-accessed since the last
// tick — covering both the steady-state propagation pass and the post-attach
// initial seeding, even when the upstream propagate systems cleared
// `BlockBfsPending` / `SkyBfsPending` mid-tick under `LightConvergeSchedule`.
//
// Bevy 0.18 `Mut::deref_mut` marks the component changed for the lifetime of
// the query iteration; the `par_iter_mut` body in `propagate_increase_block`
// /`propagate_decrease_block` consistently dereferences `&mut light.0` for
// every matched chunk, so any tick that touches a chunk's BFS queue
// surfaces here, regardless of whether the propagate phase removed the
// per-channel parked marker once its queues drained. The downstream
// codec dedups by chunk before consulting the actual `LightStorage`, so
// over-fanning at warm-up is a negligible NULL pass at the consumer.
pub fn emit_block_light_dirty(
    chunks: Query<(Entity, &InColumn), (Changed<BlockLight>, With<BlockLight>)>,
    columns: Query<(&ColumnPosComponent, &ColumnChunks)>,
    mut writer: MessageWriter<BlockLightDirty>,
) {
    for (chunk, in_column) in chunks.iter() {
        let Ok((column_pos, chunk_index)) = columns.get(in_column.0) else {
            continue;
        };
        let Some(chunk_y) = emit_dirty::chunk_y_for_chunk(chunk_index, chunk) else {
            continue;
        };
        writer.write(BlockLightDirty {
            chunk,
            column_pos: column_pos.0,
            chunk_y,
        });
    }
}