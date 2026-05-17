//! Post-convergence emit-dirty systems shared by both channels.
//!
//! Four stages run after the convergence sub-schedule terminates:
//!
//! 1. `downgrade_light_storage` inspects every chunk parked on either
//!    channel (`Or<(With<BlockBfsPending>, With<SkyBfsPending>)>`) and
//!    downgrades `LightStorage::Dense(arr)` to `Empty` (all-zero nibbles)
//!    or `Uniform(15)` (all-0xFF bytes — every cell holds 15) when the
//!    homogeneity check passes. Only just-touched chunks need the check;
//!    running on all chunks every tick is wasted work. The system stays
//!    unified because the body inspects both layers in one
//!    `Option<&mut SkyLight>`-shaped pass.
//!
//! 2. `clear_block_bfs_pending_safety_net` + `clear_sky_bfs_pending_safety_net`
//!    (scheduled as a parallel pair) live under their respective per-channel
//!    `crate::{block_light,sky_light}::emit_dirty` modules.
//!
//! 3. `clear_light_tickets` removes `LightTicket` from chunks that no
//!    longer have any parked work AND carry neither `BlockBfsPending` nor
//!    `SkyBfsPending`. The ticket represents "my neighbours must stay
//!    loaded until I drain my queues"; once everything is empty, the ticket
//!    can be dropped and the chunk unload path becomes free to evict the
//!    chunk if it goes stale.
//!
//! `chunk_y_for_chunk` is a shared helper used by both per-channel
//! `emit_*_light_dirty` systems.

use bevy_ecs::prelude::{Commands, Entity, Or, Query, With, Without};
use mcrs_engine::world::column::{ChunkLookup, ColumnChunks};
use mcrs_engine::world::lighting::LightTicket;
use crate::{
    BlockBfsPending, BlockBfsQueues, BlockInbox, BlockLight, BlockOutbox, SkyBfsPending,
    SkyBfsQueues, SkyInbox, SkyLight, SkyOutbox,
};
use crate::storage::LightStorage;

/// Downgrades `LightStorage::Dense` to `Empty` (all-zero) or `Uniform(15)`
/// (all-fifteen) on every chunk parked on either channel. The check
/// inspects the raw 2048-byte nibble array for the two homogeneous patterns
/// and leaves Dense as-is otherwise. Stays as one system because the body
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
    if let LightStorage::Dense(arr) = storage {
        let bytes = &arr.0;
        if bytes.iter().all(|&b| b == 0) {
            *storage = LightStorage::Empty;
            return;
        }
        if bytes.iter().all(|&b| b == 0xFF) {
            *storage = LightStorage::Uniform(15);
        }
    }
}

/// Removes `LightTicket` from chunks that no longer have any parked work
/// (outbox / inbox / queues queues all empty) and carry neither
/// `BlockBfsPending` nor `SkyBfsPending`. The tuple-`Without` filter is
/// conjunction-of-negations: the ticket is cleared only when neither
/// channel has parked work. Once the ticket is gone, the chunk-unload
/// path is free to evict the chunk if no observer view keeps it loaded.
pub fn clear_light_tickets(
    chunks: Query<
        (
            Entity,
            &BlockOutbox,
            &BlockInbox,
            &SkyOutbox,
            &SkyInbox,
            &BlockBfsQueues,
            &SkyBfsQueues,
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

/// Locate the chunk-Y of `target` inside `index`. Used by both
/// `emit_block_light_dirty` and `emit_sky_light_dirty` to compose the
/// outgoing message payload.
#[inline]
pub(crate) fn chunk_y_for_chunk(index: &ColumnChunks, target: Entity) -> Option<i32> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CrossChunkWavefront;
    use crate::nibble::LightNibbles;
    use bevy_app::{App, Update};
    use mcrs_engine::world::lighting::LightTicket;
    use crate::block_light::emit_dirty::clear_block_bfs_pending_safety_net;
    use crate::sky_light::emit_dirty::clear_sky_bfs_pending_safety_net;

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
    fn downgrade_light_storage_converts_all_zero_dense_to_empty() {
        let mut app = build_downgrade_app();
        let arr = LightNibbles::zeros();
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Dense(Box::new(arr))),
                BlockBfsPending,
            ))
            .id();
        app.update();
        let bl = app
            .world()
            .get::<BlockLight>(entity)
            .expect("block light");
        assert!(
            matches!(bl.0, LightStorage::Empty),
            "all-zero Dense downgrades to Empty"
        );
    }

    #[test]
    fn downgrade_light_storage_converts_all_fifteen_dense_to_uniform_15() {
        let mut app = build_downgrade_app();
        let arr = LightNibbles::filled(15);
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Dense(Box::new(arr))),
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
            "all-fifteen Dense downgrades to Uniform(15)"
        );
    }

    #[test]
    fn downgrade_light_storage_leaves_heterogeneous_dense_unchanged() {
        let mut app = build_downgrade_app();
        let mut arr = LightNibbles::filled(15);
        arr.set(3, 4, 5, 7);
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Dense(Box::new(arr))),
                BlockBfsPending,
            ))
            .id();
        app.update();
        let bl = app
            .world()
            .get::<BlockLight>(entity)
            .expect("block light");
        assert!(
            matches!(bl.0, LightStorage::Dense(_)),
            "heterogeneous Dense must stay Dense"
        );
        if let LightStorage::Dense(a) = &bl.0 {
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
            BlockOutbox::default(),
            BlockInbox::default(),
            SkyOutbox::default(),
            SkyInbox::default(),
            BlockBfsQueues::default(),
            SkyBfsQueues::default(),
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
        let mut e = BlockOutbox::default();
        e.0.push(CrossChunkWavefront::new(0, 1, 2, 3));
        app.world_mut().entity_mut(entity).insert(e);
        app.update();
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_some(),
            "BlockBfsPending retained when BlockOutbox is non-empty"
        );
    }

    #[test]
    fn safety_net_pair_keeps_when_workspace_queue_nonempty() {
        let mut app = build_safety_net_app();
        let entity = spawn_clean_chunk(&mut app, true, false);
        let mut ws = BlockBfsQueues::default();
        ws.increase_queue.push(0u64);
        app.world_mut().entity_mut(entity).insert(ws);
        app.update();
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_some(),
            "BlockBfsPending retained when queues queue is non-empty"
        );
    }

    #[test]
    fn clear_light_tickets_skips_chunks_with_pending_work() {
        let mut app = build_clear_tickets_app();
        let entity = spawn_clean_chunk(&mut app, false, true);
        let mut i = BlockInbox::default();
        i.0.push(CrossChunkWavefront::new(0, 1, 2, 3));
        app.world_mut().entity_mut(entity).insert(i);
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket retained when BlockInbox is non-empty"
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
    /// must be retained when block-side has parked work even if sky-side
    /// is clean.
    #[test]
    fn clear_light_tickets_retains_when_only_one_channel_pending_block_side() {
        let mut app = build_clear_tickets_app();
        let entity = app
            .world_mut()
            .spawn((
                BlockOutbox::default(),
                BlockInbox::default(),
                SkyOutbox::default(),
                SkyInbox::default(),
                BlockBfsQueues::default(),
                SkyBfsQueues::default(),
                LightTicket,
                BlockBfsPending,
            ))
            .id();
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket must be retained when only the block channel is parked"
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
                BlockOutbox::default(),
                BlockInbox::default(),
                SkyOutbox::default(),
                SkyInbox::default(),
                BlockBfsQueues::default(),
                SkyBfsQueues::default(),
                LightTicket,
                SkyBfsPending,
            ))
            .id();
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket must be retained when only the sky channel is parked"
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
                BlockOutbox::default(),
                BlockInbox::default(),
                SkyOutbox::default(),
                SkyInbox::default(),
                BlockBfsQueues::default(),
                SkyBfsQueues::default(),
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
                BlockOutbox::default(),
                BlockInbox::default(),
                SkyOutbox::default(),
                SkyInbox::default(),
                BlockBfsQueues::default(),
                SkyBfsQueues::default(),
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
