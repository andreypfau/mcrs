//! `update_client_blocks_per_dim` resolves recipients through the per-dim
//! `Column.PlayerObservers` Component, eliminating the two-frame buffer
//! rotation that caused the TNT silent-drop regression. The test drives the
//! per-dim system body directly with a populated change set and asserts the
//! emitted `OutboundPlayerPacket` carries the chunk's observer set.

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoSystem;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::player::Player;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::ChunkPos;
use mcrs_engine::world::dimension::InDimension;
use mcrs_engine::world::storage::column::{ColumnIndex, ColumnSlot};
use mcrs_minecraft::world::block_update::update_client_blocks_per_dim;
use mcrs_minecraft::world::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_block::block_update::ChunkNetworkSyncBlockChangesSet;
use mcrs_minecraft_block::palette::BlockPalette;

#[test]
fn block_update_resolves_observers_per_dim_emit_site() {
    let mut app = App::new();
    app.add_message::<OutboundPlayerPacket>();

    // Allocate a synthetic dimension entity, a column entity (carrying
    // PlayerObservers + acting as the lookup target via ColumnIndex), and a
    // chunk entity (the source of block-change events).
    // The player must carry the Player Component so the liveness filter in
    // update_client_blocks_per_dim passes it through (the filter uses
    // Query<Entity, With<Player>>).
    let player = app.world_mut().spawn(Player).id();

    let mut observers = PlayerObservers::default();
    observers.0.push(player);
    let column_entity = app.world_mut().spawn(observers).id();

    // Dim entity carries the ColumnIndex mapping (ColumnPos -> column entity).
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let column_pos = ColumnPos::from(chunk_pos);
    let mut column_index = ColumnIndex::default();
    column_index.0.insert(
        column_pos,
        ColumnSlot {
            entity: column_entity,
            section_count: 1,
        },
    );
    let dim_entity = app.world_mut().spawn(column_index).id();

    // Chunk entity with a populated change set — simulates a block-change
    // delta the way `apply_set_block_request` would have left it.
    let block_pos = BlockPos::new(2, 3, 4);
    let mut change_set = ChunkNetworkSyncBlockChangesSet::default();
    change_set.changes.insert(block_pos);
    let _chunk_entity = app
        .world_mut()
        .spawn((
            chunk_pos,
            InDimension(dim_entity),
            BlockPalette::default(),
            change_set,
        ))
        .id();

    // Drive the per-dim system body directly. Avoids the FixedPostUpdate
    // accumulator and keeps the test focused on what update_client_blocks_per_dim
    // emits given the input above.
    let world = app.world_mut();
    let mut sys = IntoSystem::into_system(update_client_blocks_per_dim);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);

    let buf = app.world().resource::<Messages<OutboundPlayerPacket>>();
    let mut cursor = buf.get_cursor();
    let mut block_update_count = 0;
    for pkt in cursor.read(buf) {
        if !matches!(pkt.data, PacketPayload::BlockUpdate { .. }) {
            continue;
        }
        match &pkt.target {
            PacketTarget::PlayerSet(set) => {
                assert!(
                    set.contains(&player),
                    "BlockUpdate PlayerSet target missing the chunk observer"
                );
                block_update_count += 1;
            }
            _ => panic!(
                "expected PacketTarget::PlayerSet for BlockUpdate, got {:?}",
                pkt.target
            ),
        }
    }
    assert_eq!(
        block_update_count, 1,
        "expected exactly one BlockUpdate packet per block change"
    );
}
