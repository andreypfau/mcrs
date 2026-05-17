//! Post-convergence emit-dirty systems.
//!
//! Four stages run after the convergence sub-schedule terminates:
//!
//! 1. `downgrade_light_storage` inspects every chunk pending on either
//!    channel (`Or<(With<BlockBfsPending>, With<SkyBfsPending>)>`) and
//!    downgrades `LightStorage::Mixed(arr)` to `Null` (all-zero nibbles)
//!    or `Uniform(15)` (all-0xFF bytes — every cell holds 15) when the
//!    homogeneity check passes. Only just-touched chunks need the check;
//!    running on all chunks every tick is wasted work. The system stays
//!    unified because the body inspects both layers in one
//!    `Option<&mut SkyLight>`-shaped pass.
//!
//! 2. `clear_block_bfs_pending_safety_net` + `clear_sky_bfs_pending_safety_net`
//!    (scheduled as a parallel pair) remove the channel-specific marker
//!    from chunks whose corresponding `*Egress`, `*Incoming`, and workspace
//!    queues are all empty. Each clear indicates a missed per-iteration
//!    clear inside the per-channel propagate system — emit
//!    `tracing::debug!` so the discrepancy is observable per channel.
//!
//! 3. `clear_light_tickets` removes `LightTicket` from chunks that no
//!    longer have any pending work AND carry neither `BlockBfsPending` nor
//!    `SkyBfsPending`. The ticket represents "my neighbours must stay
//!    loaded until I drain my queues"; once everything is empty, the ticket
//!    can be dropped and the chunk unload path becomes free to evict the
//!    chunk if it goes stale.

use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Commands, Entity, Or, Query, With, Without};
use mcrs_engine::world::column::{
    ColumnPosComponent, InColumn, ColumnChunks, ChunkLookup,
};
use mcrs_engine::world::lighting::LightTicket;

use crate::codec::{BlockLightDirty, SkyLightDirty};
use crate::components::{
    BlockBfsPending, BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, SkyBfsPending,
    SkyEgress, SkyIncoming, SkyLight, SkyLightWorkspace,
};
use crate::storage::LightStorage;

/// Downgrades `LightStorage::Mixed` to `Null` (all-zero) or `Uniform(15)`
/// (all-fifteen) on every chunk pending on either channel. The check
/// inspects the raw 2048-byte nibble array for the two homogeneous patterns
/// and leaves Mixed as-is otherwise. Stays as one system because the body
/// inspects both layers in one `Option<&mut SkyLight>`-shaped pass;
/// splitting into two systems would lose that shape.
pub fn downgrade_light_storage(
    mut chunks: Query<
        (&mut BlockLight, Option<&mut SkyLight>),
        Or<(With<BlockBfsPending>, With<SkyBfsPending>)>,
    >,
) {
    chunks.par_iter_mut().for_each(|(mut block_light, mut sky_light_opt)| {
        downgrade_storage_in_place(&mut block_light.0);
        if let Some(sky_light) = sky_light_opt.as_deref_mut() {
            downgrade_storage_in_place(&mut sky_light.0);
        }
    });
}

#[inline]
fn downgrade_storage_in_place(storage: &mut LightStorage) {
    if let LightStorage::Mixed(arr) = storage {
        let bytes = &arr.0;
        if bytes.iter().all(|&b| b == 0) {
            *storage = LightStorage::Null;
            return;
        }
        if bytes.iter().all(|&b| b == 0xFF) {
            *storage = LightStorage::Uniform(15);
        }
    }
}

/// Removes `BlockBfsPending` from chunks whose block-channel egress,
/// incoming, and workspace queues are all empty. Emits `tracing::debug!`
/// each time it clears anything — every clear indicates a leftover
/// `BlockBfsPending` that the per-iteration clear inside
/// `propagate_increase_block_system` missed. Scheduled in parallel with
/// its sky-channel mirror under disjoint component access.
pub fn clear_block_bfs_pending_safety_net(
    chunks: Query<
        (Entity, &BlockEgress, &BlockIncoming, &BlockLightWorkspace),
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

/// Removes `SkyBfsPending` from chunks whose sky-channel egress, incoming,
/// and workspace queues are all empty. Emits `tracing::debug!` each time
/// it clears anything — every clear indicates a leftover `SkyBfsPending`
/// that the per-iteration clear inside `propagate_increase_sky_system`
/// missed. Scheduled in parallel with its block-channel mirror under
/// disjoint component access.
pub fn clear_sky_bfs_pending_safety_net(
    chunks: Query<
        (Entity, &SkyEgress, &SkyIncoming, &SkyLightWorkspace),
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

/// Removes `LightTicket` from chunks that no longer have any pending work
/// (egress / incoming / workspace queues all empty) and carry neither
/// `BlockBfsPending` nor `SkyBfsPending`. The tuple-`Without` filter is
/// conjunction-of-negations: the ticket is cleared only when neither
/// channel has pending work. Once the ticket is gone, the chunk-unload
/// path is free to evict the chunk if no observer view keeps it loaded.
pub fn clear_light_tickets(
    chunks: Query<
        (
            Entity,
            &BlockEgress,
            &BlockIncoming,
            &SkyEgress,
            &SkyIncoming,
            &BlockLightWorkspace,
            &SkyLightWorkspace,
        ),
        (With<LightTicket>, Without<BlockBfsPending>, Without<SkyBfsPending>),
    >,
    mut commands: Commands,
) {
    for (entity, be, bi, se, si, bws, sws) in chunks.iter() {
        if be.0.is_empty()
            && bi.0.is_empty()
            && se.0.is_empty()
            && si.0.is_empty()
            && bws.increase_queue.is_empty()
            && bws.decrease_queue.is_empty()
            && sws.increase_queue.is_empty()
            && sws.decrease_queue.is_empty()
        {
            commands.entity(entity).remove::<LightTicket>();
        }
    }
}

#[inline]
fn chunk_y_for_chunk(index: &ColumnChunks, target: Entity) -> Option<i32> {
    let min_y = index.min_section_y;
    index.iter_wire().enumerate().find_map(|(idx, lookup)| {
        if let ChunkLookup::Loaded(e) = lookup {
            if e == target {
                return Some(min_y + idx as i32 - 1);
            }
        }
        None
    })
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
// per-channel pending marker once its queues drained. The downstream
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
        let Some(chunk_y) = chunk_y_for_chunk(chunk_index, chunk) else {
            continue;
        };
        writer.write(BlockLightDirty {
            chunk,
            column_pos: column_pos.0,
            chunk_y,
        });
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
        let Some(chunk_y) = chunk_y_for_chunk(chunk_index, chunk) else {
            continue;
        };
        writer.write(SkyLightDirty {
            chunk,
            column_pos: column_pos.0,
            chunk_y,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{
        BlockBfsPending, BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace,
        SkyBfsPending, SkyEgress, SkyIncoming, SkyLightWorkspace, Wavefront,
    };
    use crate::nibble::NibbleArray;
    use bevy_app::{App, Update};
    use mcrs_engine::world::lighting::LightTicket;

    fn build_downgrade_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, downgrade_light_storage);
        app
    }

    /// Builds an app running both per-channel safety-net systems. The two
    /// systems take disjoint queries (block-channel components vs sky-channel
    /// components) so Bevy slots them in parallel; the test app uses the
    /// default executor which preserves that scheduling shape.
    fn build_safety_net_app() -> App {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                clear_block_bfs_pending_safety_net,
                clear_sky_bfs_pending_safety_net,
            ),
        );
        app
    }

    fn build_clear_tickets_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, clear_light_tickets);
        app
    }

    #[test]
    fn downgrade_light_storage_converts_all_zero_mixed_to_null() {
        let mut app = build_downgrade_app();
        let arr = NibbleArray::zeros();
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Mixed(Box::new(arr))),
                BlockBfsPending,
            ))
            .id();
        app.update();
        let bl = app
            .world()
            .get::<BlockLight>(entity)
            .expect("block light");
        assert!(
            matches!(bl.0, LightStorage::Null),
            "all-zero Mixed downgrades to Null"
        );
    }

    #[test]
    fn downgrade_light_storage_converts_all_fifteen_mixed_to_uniform_15() {
        let mut app = build_downgrade_app();
        let arr = NibbleArray::filled(15);
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Mixed(Box::new(arr))),
                BlockBfsPending,
            ))
            .id();
        app.update();
        let bl = app
            .world()
            .get::<BlockLight>(entity)
            .expect("block light");
        assert!(
            matches!(bl.0, LightStorage::Uniform(15)),
            "all-fifteen Mixed downgrades to Uniform(15)"
        );
    }

    #[test]
    fn downgrade_light_storage_leaves_heterogeneous_mixed_unchanged() {
        let mut app = build_downgrade_app();
        let mut arr = NibbleArray::filled(15);
        arr.set(3, 4, 5, 7);
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Mixed(Box::new(arr))),
                BlockBfsPending,
            ))
            .id();
        app.update();
        let bl = app
            .world()
            .get::<BlockLight>(entity)
            .expect("block light");
        assert!(
            matches!(bl.0, LightStorage::Mixed(_)),
            "heterogeneous Mixed must stay Mixed"
        );
        if let LightStorage::Mixed(a) = &bl.0 {
            assert_eq!(a.get(3, 4, 5), 7);
        }
    }

    /// Spawn a chunk with the full eight-component drain state, optionally
    /// flagged on both channels and/or with a `LightTicket`. The default
    /// for `dirty=true` inserts BOTH `BlockBfsPending` and `SkyBfsPending`
    /// so the existing safety-net + ticket tests exercise the joint clear
    /// path. Per-channel isolation tests use the dedicated helpers below.
    fn spawn_clean_chunk(app: &mut App, dirty: bool, ticket: bool) -> bevy_ecs::entity::Entity {
        let mut e = app.world_mut().spawn((
            BlockEgress::default(),
            BlockIncoming::default(),
            SkyEgress::default(),
            SkyIncoming::default(),
            BlockLightWorkspace::default(),
            SkyLightWorkspace::default(),
        ));
        if dirty {
            e.insert(BlockBfsPending);
            e.insert(SkyBfsPending);
        }
        if ticket {
            e.insert(LightTicket);
        }
        e.id()
    }

    #[test]
    fn safety_net_pair_clears_when_all_queues_and_buffers_empty() {
        let mut app = build_safety_net_app();
        let entity = spawn_clean_chunk(&mut app, true, false);
        app.update();
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_none(),
            "BlockBfsPending cleared when queues/buffers all empty"
        );
        assert!(
            app.world().get::<SkyBfsPending>(entity).is_none(),
            "SkyBfsPending cleared when queues/buffers all empty"
        );
    }

    #[test]
    fn safety_net_pair_keeps_when_egress_nonempty() {
        let mut app = build_safety_net_app();
        let entity = spawn_clean_chunk(&mut app, true, false);
        let mut e = BlockEgress::default();
        e.0.push(Wavefront::new(0, 1, 2, 3));
        app.world_mut().entity_mut(entity).insert(e);
        app.update();
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_some(),
            "BlockBfsPending retained when BlockEgress is non-empty"
        );
    }

    #[test]
    fn safety_net_pair_keeps_when_workspace_queue_nonempty() {
        let mut app = build_safety_net_app();
        let entity = spawn_clean_chunk(&mut app, true, false);
        let mut ws = BlockLightWorkspace::default();
        ws.increase_queue.push(0u64);
        app.world_mut().entity_mut(entity).insert(ws);
        app.update();
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_some(),
            "BlockBfsPending retained when workspace queue is non-empty"
        );
    }

    #[test]
    fn clear_light_tickets_skips_chunks_with_pending_work() {
        let mut app = build_clear_tickets_app();
        let entity = spawn_clean_chunk(&mut app, false, true);
        let mut i = BlockIncoming::default();
        i.0.push(Wavefront::new(0, 1, 2, 3));
        app.world_mut().entity_mut(entity).insert(i);
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket retained when BlockIncoming is non-empty"
        );
    }

    #[test]
    fn clear_light_tickets_removes_when_all_pending_work_drained() {
        let mut app = build_clear_tickets_app();
        let entity = spawn_clean_chunk(&mut app, false, true);
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_none(),
            "LightTicket cleared when all queues/buffers empty and not dirty"
        );
    }

    #[test]
    fn clear_light_tickets_skips_dirty_chunks() {
        let mut app = build_clear_tickets_app();
        let entity = spawn_clean_chunk(&mut app, true, true);
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket retained on chunks with either per-channel marker (tuple-Without filter)"
        );
    }

    // ---- per-channel boolean-correctness + isolation tests ----

    /// Boolean correctness: the `clear_light_tickets` filter is
    /// conjunction-of-negations
    /// (`Without<BlockBfsPending>, Without<SkyBfsPending>`), so the ticket
    /// must be retained when block-side has pending work even if sky-side
    /// is clean.
    #[test]
    fn clear_light_tickets_retains_when_only_one_channel_pending_block_side() {
        let mut app = build_clear_tickets_app();
        let entity = app
            .world_mut()
            .spawn((
                BlockEgress::default(),
                BlockIncoming::default(),
                SkyEgress::default(),
                SkyIncoming::default(),
                BlockLightWorkspace::default(),
                SkyLightWorkspace::default(),
                LightTicket,
                BlockBfsPending,
            ))
            .id();
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket must be retained when only the block channel is pending"
        );
    }

    /// Mirror of `clear_light_tickets_retains_when_only_one_channel_pending_block_side`
    /// for the sky channel.
    #[test]
    fn clear_light_tickets_retains_when_only_one_channel_pending_sky_side() {
        let mut app = build_clear_tickets_app();
        let entity = app
            .world_mut()
            .spawn((
                BlockEgress::default(),
                BlockIncoming::default(),
                SkyEgress::default(),
                SkyIncoming::default(),
                BlockLightWorkspace::default(),
                SkyLightWorkspace::default(),
                LightTicket,
                SkyBfsPending,
            ))
            .id();
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket must be retained when only the sky channel is pending"
        );
    }

    /// Channel isolation: running only the block-side safety net must
    /// remove only `BlockBfsPending` and leave `SkyBfsPending` intact.
    /// The block-side system's query reads only block-channel components,
    /// so the sky marker is structurally outside its reach.
    #[test]
    fn block_safety_net_does_not_touch_sky_marker() {
        let mut app = App::new();
        app.add_systems(Update, clear_block_bfs_pending_safety_net);
        let entity = app
            .world_mut()
            .spawn((
                BlockEgress::default(),
                BlockIncoming::default(),
                SkyEgress::default(),
                SkyIncoming::default(),
                BlockLightWorkspace::default(),
                SkyLightWorkspace::default(),
                LightTicket,
                BlockBfsPending,
                SkyBfsPending,
            ))
            .id();
        app.update();
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_none(),
            "block safety net must clear BlockBfsPending on a quiet chunk"
        );
        assert!(
            app.world().get::<SkyBfsPending>(entity).is_some(),
            "block safety net must NOT touch SkyBfsPending"
        );
    }

    /// Mirror of `block_safety_net_does_not_touch_sky_marker` for the sky
    /// channel.
    #[test]
    fn sky_safety_net_does_not_touch_block_marker() {
        let mut app = App::new();
        app.add_systems(Update, clear_sky_bfs_pending_safety_net);
        let entity = app
            .world_mut()
            .spawn((
                BlockEgress::default(),
                BlockIncoming::default(),
                SkyEgress::default(),
                SkyIncoming::default(),
                BlockLightWorkspace::default(),
                SkyLightWorkspace::default(),
                LightTicket,
                BlockBfsPending,
                SkyBfsPending,
            ))
            .id();
        app.update();
        assert!(
            app.world().get::<SkyBfsPending>(entity).is_none(),
            "sky safety net must clear SkyBfsPending on a quiet chunk"
        );
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_some(),
            "sky safety net must NOT touch BlockBfsPending"
        );
    }
}
