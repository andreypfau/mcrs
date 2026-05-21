//! Block-light BFS wrappers and systems. The implementations currently
//! live in `crate::propagate`; this module re-exports the block-side
//! surface so callers can land on `crate::block_light::propagate::*`
//! as the canonical path. A future refactor will move the bodies here.

use bevy_ecs::change_detection::Res;
use bevy_ecs::prelude::{ParallelCommands, Query, With};
use bevy_ecs::entity::Entity;
use mcrs_minecraft_block::palette::BlockPalette;
use crate::{propagate, BlockBfsPending, BlockBfsQueues, BlockInbox, BlockLight, BlockOutbox, BlockOutboxDirty};
use crate::bfs::{propagate_decrease, propagate_increase};
use crate::table::BlockStateLightTable;

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "lighting::propagate_decrease", skip_all, fields(chunk_count = tracing::field::Empty))
)]
pub fn propagate_decrease_block_system(
    table: Res<BlockStateLightTable>,
    mut chunks: Query<
        (
            Entity,
            &BlockPalette,
            &mut BlockLight,
            &mut BlockBfsQueues,
            &mut BlockOutbox,
            &mut BlockInbox,
        ),
        With<BlockBfsPending>,
    >,
    commands: ParallelCommands,
) {
    #[cfg(feature = "telemetry-tracy")]
    tracing::Span::current().record("chunk_count", chunks.iter().count());
    chunks.par_iter_mut().for_each(
        |(entity, palette, mut light, mut queues, mut outbox, mut inbox)| {
            propagate::drain_incoming_into_queue(&mut inbox.0, &mut queues.increase_queue);
            propagate_decrease(&table, palette, &mut light.0, &mut queues, &mut outbox);
            if !outbox.0.is_empty() {
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).insert(BlockOutboxDirty);
                });
            }
            #[cfg(feature = "telemetry-tracy")]
            tracing::debug!(chunk = ?entity, queue_len = queues.decrease_queue.len(), "chunk bfs decrease block");
        },
    );
}

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "lighting::propagate_increase", skip_all, fields(chunk_count = tracing::field::Empty))
)]
pub fn propagate_increase_block_system(
    table: Res<BlockStateLightTable>,
    mut chunks: Query<
        (
            Entity,
            &BlockPalette,
            &mut BlockLight,
            &mut BlockBfsQueues,
            &mut BlockOutbox,
            &mut BlockInbox,
        ),
        With<BlockBfsPending>,
    >,
    commands: ParallelCommands,
) {
    #[cfg(feature = "telemetry-tracy")]
    tracing::Span::current().record("chunk_count", chunks.iter().count());
    chunks.par_iter_mut().for_each(
        |(entity, palette, mut light, mut queues, mut outbox, mut inbox)| {
            propagate::drain_incoming_into_queue(&mut inbox.0, &mut queues.increase_queue);
            propagate_increase(&table, palette, &mut light.0, &mut queues, &mut outbox);
            if !outbox.0.is_empty() {
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).insert(BlockOutboxDirty);
                });
            }
            if queues.increase_queue.is_empty() && queues.decrease_queue.is_empty() {
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).remove::<BlockBfsPending>();
                });
            }
            #[cfg(feature = "telemetry-tracy")]
            tracing::debug!(chunk = ?entity, queue_len = queues.increase_queue.len(), "chunk bfs increase block");
        },
    );
}