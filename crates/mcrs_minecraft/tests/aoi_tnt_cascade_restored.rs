//! Regression guard for the TNT silent-drop. With the per-dim block-update
//! migration, `tick_explode` (the `MessageWriter<BlockSetRequest>`) and
//! `apply_set_block_request` (the matching reader) live in the same
//! per-dim `World`, so emitted `BlockSetRequest` messages reach the reader
//! in the same tick. The reader in turn writes to a chunk's
//! `ChunkNetworkSyncBlockChangesSet`, and the per-dim wire emitter fans the
//! resulting `OutboundPlayerPacket` to the chunk's observers.
//!
//! The test wires the writer half (a hand-written `BlockSetRequest`) and
//! exercises the full `apply_set_block_request` -> `update_client_blocks_per_dim`
//! chain end-to-end inside a single sub-world. The full primed-TNT entity
//! pipeline (Fuse, Explosion, Detonator) is gated on entity scheduling not
//! required to prove the cross-system message hop is structurally fixed; the
//! cascade-flag assertion plus the per-dim writer-to-emitter integration
//! exercise the failure mode that the original regression introduced.

use bevy_app::{App, FixedPostUpdate, FixedUpdate};
use bevy_ecs::message::Messages;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::player::Player;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkIndex, ChunkPos};
use mcrs_engine::world::dimension::InDimension;
use mcrs_engine::world::storage::column::{ColumnIndex, ColumnSlot};
use mcrs_minecraft::world::block_update::{BlockUpdatePlugin, BlockUpdateWirePlugin};
use mcrs_minecraft::world::bus::{OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft::world::explosion::ExplosionConfig;
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

#[test]
fn tnt_cascade_propagates_through_block_update_per_dim() {
    // (a) cascading flag is on by default — the cascade is structurally restored
    // by the per-dim block-update migration.
    let cfg = ExplosionConfig::default();
    assert!(
        cfg.cascading_enabled,
        "ExplosionConfig::default().cascading_enabled must be true; \
         the cascade is structurally restored by the per-dim block-update migration"
    );

    // (b) Build a per-dim-shaped App: the writer (a BlockSetRequest emitted by
    // the test as a stand-in for tick_explode) and the reader
    // (apply_set_block_request from BlockUpdatePlugin) live in the same World,
    // so the message hop is single-frame and the chunk's
    // ChunkNetworkSyncBlockChangesSet sees the change.
    let mut app = App::new();
    app.add_message::<OutboundPlayerPacket>();
    // BlockUpdatePlugin no longer registers BlockSetRequest / BlockPlaced
    // itself — that responsibility belongs to the per-dim sub-app builder.
    // The test reproduces the per-dim shape by registering the same buffers
    // here before add_plugins.
    app.add_message::<BlockSetRequest>();
    app.add_message::<BlockPlaced>();
    app.add_plugins(BlockUpdatePlugin);
    app.add_plugins(BlockUpdateWirePlugin);

    // The player must carry the Player Component so the liveness filter in
    // update_client_blocks_per_dim passes it through.
    let player = app.world_mut().spawn(Player).id();
    let mut observers = PlayerObservers::default();
    observers.0.push(player);
    let column_entity = app.world_mut().spawn(observers).id();

    let chunk_positions: Vec<ChunkPos> = (0..3)
        .flat_map(|x| (0..3).map(move |z| ChunkPos::new(x, 0, z)))
        .collect();

    // Each 3x3 chunk lives in its own ColumnPos; one shared dim carries the
    // ChunkIndex (for the BlockSetRequest reader's lookup) and the ColumnIndex
    // (for the wire emitter's observer-set lookup). Each column maps back to the
    // single column_entity so every chunk's observers resolve to the same player.
    let mut chunk_index = ChunkIndex::default();
    let mut column_index = ColumnIndex::default();
    let dim_entity = app.world_mut().spawn_empty().id();

    for &chunk_pos in &chunk_positions {
        let chunk_entity = app
            .world_mut()
            .spawn((
                chunk_pos,
                InDimension(dim_entity),
                BlockPalette::default(),
            ))
            .id();
        chunk_index.insert(chunk_pos, chunk_entity);
        column_index.0.insert(
            ColumnPos::from(chunk_pos),
            ColumnSlot {
                entity: column_entity,
                section_count: 1,
            },
        );
    }
    app.world_mut()
        .entity_mut(dim_entity)
        .insert((chunk_index, column_index));

    // Tick once to let add_changes_set seed the per-chunk
    // ChunkNetworkSyncBlockChangesSet Component before any BlockSetRequest fires.
    app.world_mut().run_schedule(FixedUpdate);

    // Emit one BlockSetRequest per chunk — the "cascade simulation": after the
    // initial detonation, 9 secondary TNT positions would each emit a
    // BlockSetRequest to remove themselves. We model that here as the writer
    // half of the cascade chain (tick_explode's actual emission is gated on
    // the full entity pipeline which is out of scope for this regression test).
    {
        let mut writer = app
            .world_mut()
            .resource_mut::<Messages<BlockSetRequest>>();
        for &chunk_pos in &chunk_positions {
            let block_pos = BlockPos::new(
                chunk_pos.x * 16 + 8,
                chunk_pos.y * 16 + 8,
                chunk_pos.z * 16 + 8,
            );
            // Use a non-zero block state — apply_set_block_request short-circuits
            // when old_state == new_state. Default-init BlockPalettes are all
            // BlockStateId(0), so setting to a sentinel non-zero id ensures the
            // change is recorded.
            writer.write(BlockSetRequest {
                dimension: dim_entity,
                pos: block_pos,
                new_state: BlockStateId(1),
                flags: BlockUpdateFlags::all(),
                recursion_left: 1,
            });
        }
    }

    // Drive the schedule: FixedUpdate runs apply_set_block_request (which
    // writes into each chunk's ChunkNetworkSyncBlockChangesSet);
    // FixedPostUpdate runs update_client_blocks_per_dim which fans
    // OutboundPlayerPackets out to observers.
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(FixedPostUpdate);

    // Assertion: at least 9 BlockUpdate packets emitted — one per cascade event.
    let buf = app.world().resource::<Messages<OutboundPlayerPacket>>();
    let mut cursor = buf.get_cursor();
    let count = cursor
        .read(buf)
        .filter(|pkt| matches!(pkt.data, PacketPayload::BlockUpdate { .. }))
        .count();
    assert!(
        count >= 9,
        "expected at least 9 BlockUpdate packets from the 9 cascade events, observed {count}"
    );
}
