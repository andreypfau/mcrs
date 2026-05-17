//! Sky-light emit-dirty system and safety-net sweep. The
//! implementations currently live in `crate::emit_dirty`; this module
//! re-exports the sky-side surface so callers can land on
//! `crate::sky_light::emit_dirty::*` as the canonical path.

use bevy_ecs::prelude::{Changed, Commands, Query, With};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use mcrs_engine::world::column::{ColumnChunks, ColumnPosComponent, InColumn};
use crate::{emit_dirty, SkyBfsPending, SkyBfsQueues, SkyInbox, SkyLight, SkyLightDirty, SkyOutbox};

/// Removes `SkyBfsPending` from chunks whose sky-channel outbox, inbox,
/// and queues queues are all empty. Emits `tracing::debug!` each time
/// it clears anything — every clear indicates a leftover `SkyBfsPending`
/// that the per-iteration clear inside `propagate_increase_sky_system`
/// missed. Scheduled in parallel with its block-channel mirror under
/// disjoint component access.
pub fn clear_sky_bfs_pending_safety_net(
    chunks: Query<
        (Entity, &SkyOutbox, &SkyInbox, &SkyBfsQueues),
        With<SkyBfsPending>,
    >,
    mut commands: Commands,
) {
    for (entity, se, si, sws) in chunks.iter() {
        if se.0.is_empty()
            && si.0.is_empty()
            && sws.increase_queue.is_empty()
            && sws.decrease_queue.is_empty()
        {
            commands.entity(entity).remove::<SkyBfsPending>();
            tracing::debug!(?entity, "SkyBfsPending safety-net cleared");
        }
    }
}

pub fn emit_sky_light_dirty(
    chunks: Query<(Entity, &InColumn), (Changed<SkyLight>, With<SkyLight>)>,
    columns: Query<(&ColumnPosComponent, &ColumnChunks)>,
    mut writer: MessageWriter<SkyLightDirty>,
) {
    for (chunk, in_column) in chunks.iter() {
        let Ok((column_pos, chunk_index)) = columns.get(in_column.0) else {
            continue;
        };
        let Some(chunk_y) = emit_dirty::chunk_y_for_chunk(chunk_index, chunk) else {
            continue;
        };
        writer.write(SkyLightDirty {
            chunk,
            column_pos: column_pos.0,
            chunk_y,
        });
    }
}