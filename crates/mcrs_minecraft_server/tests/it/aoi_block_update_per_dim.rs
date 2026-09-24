//! `update_client_blocks_per_dim` resolves recipients through the per-dim
//! `Column.PlayerObservers` Component, eliminating the two-frame buffer
//! rotation that caused the TNT silent-drop regression. The test drives the
//! per-dim system body directly with placed blocks and asserts the emitted
//! `OutboundPlayerPacket` carries the chunk's observer set.

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoSystem;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::block::BlockUpdateFlags;
use mcrs_minecraft_level::block_update::BlockPlaced;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::column::{ColumnIndex, ColumnSlot};
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_server::world::block_update::update_client_blocks_per_dim;
use mcrs_minecraft_server::world::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_server::world::entity::player::HostAnchor;

#[test]
fn block_update_resolves_observers_per_dim_emit_site() {
    let mut app = App::new();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<BlockPlaced>();

    // Allocate a synthetic dimension entity, a column entity (carrying
    // PlayerObservers + acting as the lookup target via ColumnIndex), and a
    // chunk entity (the source of block-change events).
    // The player must carry the Player Component so the liveness filter in
    // update_client_blocks_per_dim passes it through (the filter uses
    // Query<Entity, With<Player>>), and its HostAnchor because that is what the
    // bus addresses: the session registry is keyed by anchor.
    let anchor = app.world_mut().spawn_empty().id();
    let player = app.world_mut().spawn((Player, HostAnchor(anchor))).id();

    let mut observers = PlayerObservers::default();
    observers.0.push(player);
    let column_entity = app.world_mut().spawn(observers).id();

    // Dim entity carries the ColumnIndex mapping (ColumnPos -> column entity).
    let chunk_pos = SectionPos::new(0, 0, 0);
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

    let chunk_entity = app
        .world_mut()
        .spawn((chunk_pos, InDimension(dim_entity), ChunkBlocks::default()))
        .id();

    // The block placed twice goes out once, and the one placed without telling
    // clients does not go out at all.
    let placed = |block_pos: BlockPos, flags: BlockUpdateFlags| BlockPlaced {
        chunk: chunk_entity,
        chunk_pos,
        block_pos,
        old_state: BlockStateId(0).into(),
        new_state: BlockStateId(0).into(),
        flags,
    };
    let block_pos = BlockPos::new(2, 3, 4);
    app.world_mut()
        .write_message(placed(block_pos, BlockUpdateFlags::all()));
    app.world_mut()
        .write_message(placed(block_pos, BlockUpdateFlags::all()));
    app.world_mut()
        .write_message(placed(BlockPos::new(5, 6, 7), BlockUpdateFlags::empty()));

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
        if !matches!(pkt.data, PacketPayload::BlockUpdate(_)) {
            continue;
        }
        match &pkt.target {
            PacketTarget::PlayerSet(set) => {
                assert!(
                    set.contains(&anchor),
                    "BlockUpdate PlayerSet target missing the chunk observer's anchor"
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
        "expected exactly one BlockUpdate packet per changed block"
    );
}
