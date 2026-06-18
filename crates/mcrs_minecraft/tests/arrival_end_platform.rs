//! RED scaffold — End-platform arrival: live-block mutation proof (MOVE-04).
//!
//! Pre-inserts a loaded chunk into a target sub-app world, drives an
//! `ArrivalCause::EndPlatform` move, and asserts that the 5×5 obsidian floor
//! is present and the blocks above are air.
//!
//! RED until the arrival resolution system is wired (later plans).

use bevy_app::{App, AppLabel, SubApp};
use bevy_ecs::prelude::*;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::session::{
    MoveId, PlayerSessionCounter, SessionRegistry,
};
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::channels::{
    DimSender, ToDimReceiver, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};
use mcrs_engine::world::chunk::{Chunk, ChunkIndex, ChunkPos};
use mcrs_engine::world::dimension::Dimension;
use mcrs_engine::world::in_flight::InFlightMoves;
use mcrs_engine::world::sub_app::DimDespawnQueue;
use mcrs_minecraft::world::bus::{
    ArrivalCause, InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn,
    MovePayload, OutboundPlayerAttached, OutboundPlayerDisconnect, OutboundPlayerPacket,
    OutboundPlayerTransfer,
};
use mcrs_minecraft::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_minecraft::world::sub_app_builder::DimSubAppHandle;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::uuid::Uuid;
use mcrs_protocol::BlockStateId;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DimTick;

#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct TestDimLabel(u8);

/// Obsidian block state id (vanilla network id 1126).
const OBSIDIAN_STATE: BlockStateId = BlockStateId(1126);

fn register_sub_messages(sub_app: &mut SubApp) {
    sub_app.add_message::<OutboundPlayerPacket>();
    sub_app.add_message::<InboundPlayerPacket>();
    sub_app.add_message::<OutboundPlayerTransfer>();
    sub_app.add_message::<InboundPlayerSpawn>();
    sub_app.add_message::<OutboundPlayerAttached>();
    sub_app.add_message::<OutboundPlayerDisconnect>();
    sub_app.add_message::<InboundPlayerDespawn>();
}

fn make_dim_channels(
    app: &mut App,
    label_entity: Entity,
) -> (
    flume::Receiver<ToDim>,
    flume::Receiver<ToDim>,
    flume::Sender<FromDim>,
) {
    let (srv_tx, srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    app.world_mut()
        .resource_mut::<DimChannelsResource>()
        .insert(label_entity, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
    (srv_rx, ctl_rx, from_tx)
}

/// End-platform arrival: 5×5 obsidian floor + air above.
///
/// The test pre-loads the target chunk, injects a SpawnEntity with
/// ArrivalCause::EndPlatform, drives ticks, then asserts via BlockPalette::get
/// that obsidian is present at every floor position.
///
/// RED until the arrival resolution system is wired.
#[test]
fn end_platform_creates_obsidian_floor_and_clears_above() {
    // Platform origin — arrival y=64 means floor at y=63.
    let arrival_pos = DVec3::new(0.0, 64.0, 0.0);
    let floor_y = arrival_pos.y as i32 - 1;

    let mut app = App::new();

    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app.init_resource::<SessionRegistry>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.init_resource::<DimDespawnQueue>();
    app.init_resource::<InFlightMoves>();

    let source_label = app.world_mut().spawn(DimSubAppHandle).id();
    let dest_label = app.world_mut().spawn(DimSubAppHandle).id();

    // Source sub-app: minimal.
    let (src_srv_rx, src_ctl_rx, _from_src_tx) = make_dim_channels(&mut app, source_label);
    {
        let mut sub = SubApp::new();
        sub.update_schedule = Some(DimTick.intern());
        sub.add_schedule(Schedule::new(DimTick));
        register_sub_messages(&mut sub);
        sub.insert_resource(ToDimReceiver::<ToDim> {
            serverbound: src_srv_rx,
            control: src_ctl_rx,
        });
        sub.world_mut().spawn(Transform {
            translation: arrival_pos,
            ..Default::default()
        });
        sub.set_extract(|_main_world, _sub_world| {});
        app.insert_sub_app(TestDimLabel(0), sub);
    }

    // Target (End) sub-app: pre-load a chunk so BlockSetRequest writes land.
    let (dest_srv_rx, dest_ctl_rx, _from_dest_tx) = make_dim_channels(&mut app, dest_label);
    {
        let mut sub = SubApp::new();
        sub.update_schedule = Some(DimTick.intern());
        sub.add_schedule(Schedule::new(DimTick));
        register_sub_messages(&mut sub);
        sub.insert_resource(ToDimReceiver::<ToDim> {
            serverbound: dest_srv_rx,
            control: dest_ctl_rx,
        });

        // Pre-insert a loaded chunk at the arrival chunk position.
        let chunk_pos = ChunkPos::from(BlockPos::new(
            arrival_pos.x as i32,
            arrival_pos.y as i32,
            arrival_pos.z as i32,
        ));
        let chunk_entity = sub.world_mut().spawn((Chunk, BlockPalette::default())).id();
        let dim_entity = sub.world_mut().spawn(Dimension).id();
        let mut chunk_index = ChunkIndex::default();
        chunk_index.insert(chunk_pos, chunk_entity);
        sub.world_mut().entity_mut(dim_entity).insert(chunk_index);

        sub.set_extract(|_main_world, _sub_world| {});
        app.insert_sub_app(TestDimLabel(1), sub);
    }

    // Inject SpawnEntity directly to the target control channel.
    {
        let channels = app.world().resource::<DimChannelsResource>();
        let entry = channels.get(dest_label).expect("dest channels");
        entry
            .control_sender
            .try_send(ToDim::SpawnEntity {
                move_id: MoveId(7),
                epoch: 0,
                cause: ArrivalCause::EndPlatform,
                payload: MovePayload::Player {
                    uuid: Uuid::nil(),
                    username: "end-test".to_string(),
                },
                player: None,
            })
            .expect("send SpawnEntity to dest");
    }

    // Drive several ticks so the arrival system (when wired) can process
    // SpawnEntity and apply_set_block_request can pick up the writes.
    for _ in 0..4 {
        app.update();
        mcrs_minecraft::runner::pump_channels(&mut app);
    }

    // RED assertion: 5×5 obsidian floor via BlockPalette::get.
    let chunk_entity = {
        let w = app.sub_app_mut(TestDimLabel(1)).world_mut();
        w.query_filtered::<Entity, (With<Chunk>, With<BlockPalette>)>()
            .single(w)
            .expect("pre-loaded chunk entity")
    };

    let mut obsidian_floor_ok = true;
    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            let pos =
                BlockPos::new(arrival_pos.x as i32 + dx, floor_y, arrival_pos.z as i32 + dz);
            let state = app
                .sub_app(TestDimLabel(1))
                .world()
                .entity(chunk_entity)
                .get::<BlockPalette>()
                .expect("BlockPalette")
                .get(pos);
            if state != OBSIDIAN_STATE {
                obsidian_floor_ok = false;
            }
        }
    }
    assert!(
        obsidian_floor_ok,
        "End platform 5×5 obsidian floor must be present (RED: arrival system not wired yet)"
    );

    // Check that the 3 blocks directly above the center are air (BlockStateId(0)).
    for dy in 0..3i32 {
        for dx in -1i32..=1 {
            for dz in -1i32..=1 {
                let pos = BlockPos::new(
                    arrival_pos.x as i32 + dx,
                    floor_y + 1 + dy,
                    arrival_pos.z as i32 + dz,
                );
                let state = app
                    .sub_app(TestDimLabel(1))
                    .world()
                    .entity(chunk_entity)
                    .get::<BlockPalette>()
                    .expect("BlockPalette")
                    .get(pos);
                assert_eq!(
                    state,
                    BlockStateId(0),
                    "block above End platform at {pos:?} must be air (RED)"
                );
            }
        }
    }
}
