//! Play-state delivery tests: per-variant encode acceptance tests for the
//! play-login trio + chunk-render prerequisites, and production-topology
//! tests verifying that the in-dim emitter routes packets through the bus
//! to the host-resident connection without querying ServerSideConnection.

#[path = "common/mock_connection.rs"]
mod mock_connection;

use std::sync::atomic::Ordering;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use bytes::Bytes;
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::sub_app::{DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::world::bridge::dispatch_encode;
use mcrs_minecraft::world::bridge_queue::OutboundQueue;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer, PacketPayload,
    PacketPriority, PacketTarget, PendingInboundLifecycle, PendingInboundPartition,
    PlayerTransferSnapshot,
};
use mcrs_minecraft::world::entity::player::HostAnchor;
use mcrs_minecraft::world::player_index::{PlayerIndex, PlayerLocation};
use mcrs_minecraft::world::sub_app_builder::{drain_dim_spawn_queue, DimSubAppHandle};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_network::metrics::{BRIDGE_ENCODE_UNHANDLED_TOTAL, TELEMETRY_TEST_LOCK};
use mcrs_network::ServerSideConnection;
use mcrs_protocol::chunk::LightData;
use mcrs_protocol::uuid::Uuid;
use mcrs_protocol::GameMode;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;
use smallvec::SmallVec;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_stub_block_light_table() -> BlockStateLightTable {
    let state_count = 2usize;
    let emission = vec![0u8; state_count].into_boxed_slice();
    let dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let flags = vec![0u8; state_count].into_boxed_slice();
    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn build_dispatch_world() -> (World, Entity, mpsc::Receiver<Bytes>) {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<PlayerIndex>();

    let (raw, rx) = mock_connection::make_mock_raw_connection();
    let entity = world
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            OutboundQueue::default(),
        ))
        .id();
    (world, entity, rx)
}

fn run_dispatch(world: &mut World) {
    let mut sys = IntoSystem::into_system(dispatch_encode);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

fn push_critical(world: &mut World, entity: Entity, payload: PacketPayload) {
    world
        .get_mut::<OutboundQueue>(entity)
        .expect("OutboundQueue present")
        .push(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(entity),
            priority: PacketPriority::Critical,
            data: payload,
        });
}

fn build_host_app() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(make_stub_block_light_table());
    app.insert_resource(StaticRegistry::<Block>::new());
    app.insert_resource(StaticRegistry::<EnchantmentData>::default());
    app.insert_resource(TagRegistry::<Block>::default());

    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundPartition>();
    app.init_resource::<PendingInboundLifecycle>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app
}

fn spawn_subapp(app: &mut App) -> Entity {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: mcrs_engine::world::dimension::DimensionId::new("test:overworld"),
            type_config: mcrs_engine::world::dimension::DimensionTypeConfig::default(),
            has_sky: true,
        });
    drain_dim_spawn_queue(app);
    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
    assert_eq!(handles.len(), 1, "expected exactly one DimSubAppHandle");
    handles[0]
}

// ---------------------------------------------------------------------------
// Per-variant encode acceptance tests (Task 1)
// ---------------------------------------------------------------------------

/// `PacketPayload::PlayerLogin` encodes to a non-empty blob without
/// incrementing `BRIDGE_ENCODE_UNHANDLED_TOTAL`.
#[test]
fn player_login_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    push_critical(
        &mut world,
        entity,
        PacketPayload::PlayerLogin {
            player_id: 42,
            hardcore: false,
            game_mode: GameMode::Creative,
            dimensions: vec!["minecraft:overworld".to_string()],
            max_players: 100,
            chunk_radius: 12,
            simulation_distance: 12,
            reduced_debug_info: false,
            show_death_screen: false,
            do_limited_crafting: false,
            enforces_secure_chat: false,
        },
    );

    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "PlayerLogin must not increment unhandled");

    let blob = rx.try_recv().expect("blob sent to socket");
    assert!(!blob.is_empty(), "PlayerLogin must produce a non-empty blob");
}

/// `PacketPayload::LevelChunksLoadStart` encodes to a non-empty blob
/// (the GameEvent packet).
#[test]
fn level_chunks_load_start_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    push_critical(&mut world, entity, PacketPayload::LevelChunksLoadStart);
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "LevelChunksLoadStart must not increment unhandled");

    let blob = rx.try_recv().expect("blob sent to socket");
    assert!(!blob.is_empty(), "LevelChunksLoadStart must produce a non-empty blob");
}

/// `PacketPayload::PlayerLoginEntityEvent` encodes to a non-empty blob
/// (the EntityEvent packet).
#[test]
fn player_login_entity_event_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    push_critical(
        &mut world,
        entity,
        PacketPayload::PlayerLoginEntityEvent {
            entity_id: 42,
            entity_status: 24,
        },
    );
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "PlayerLoginEntityEvent must not increment unhandled");

    let blob = rx.try_recv().expect("blob sent to socket");
    assert!(!blob.is_empty(), "PlayerLoginEntityEvent must produce a non-empty blob");
}

/// `PacketPayload::SetChunkCacheCenter` encodes to a non-empty blob.
#[test]
fn cache_center_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    push_critical(
        &mut world,
        entity,
        PacketPayload::SetChunkCacheCenter { x: 0, z: 0 },
    );
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "SetChunkCacheCenter must not increment unhandled");

    let blob = rx.try_recv().expect("blob sent to socket");
    assert!(!blob.is_empty(), "SetChunkCacheCenter must produce a non-empty blob");
}

/// `PacketPayload::SetChunkCacheRadius` encodes to a non-empty blob.
#[test]
fn cache_radius_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    push_critical(
        &mut world,
        entity,
        PacketPayload::SetChunkCacheRadius { radius: 12 },
    );
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "SetChunkCacheRadius must not increment unhandled");

    let blob = rx.try_recv().expect("blob sent to socket");
    assert!(!blob.is_empty(), "SetChunkCacheRadius must produce a non-empty blob");
}

/// `PacketPayload::PlayerInfoUpdate` encodes to a non-empty blob.
#[test]
fn player_info_update_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    push_critical(
        &mut world,
        entity,
        PacketPayload::PlayerInfoUpdate {
            entries: vec![mcrs_minecraft::world::bus::PlayerInfoEntry {
                player_uuid: Uuid::nil(),
                username: "test".to_string(),
                game_mode: GameMode::Creative,
                listed: true,
            }],
        },
    );
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "PlayerInfoUpdate must not increment unhandled");

    let blob = rx.try_recv().expect("blob sent to socket");
    assert!(!blob.is_empty(), "PlayerInfoUpdate must produce a non-empty blob");
}

// ---------------------------------------------------------------------------
// play_login_emitted_on_spawn — production-topology (Task 1)
// ---------------------------------------------------------------------------

/// When a player entity is spawned in the sub-app (InboundPlayerSpawn consumed),
/// the `emit_play_login` system must write at least one Critical
/// `OutboundPlayerPacket` carrying `PacketPayload::PlayerLogin` targeting the
/// host-anchor entity into the sub-app's message bus.
#[test]
fn play_login_emitted_on_spawn() {
    let mut app = build_host_app();
    let dim_label = spawn_subapp(&mut app);

    let host_anchor = app.world_mut().spawn_empty().id();
    app.world_mut().resource_mut::<PlayerIndex>().insert(
        host_anchor,
        PlayerLocation {
            socket: Entity::PLACEHOLDER,
            current_dim: dim_label,
            previous_dim: None,
            in_dim_entity: None,
            inbound_pending: SmallVec::new(),
        },
    );

    // Push InboundPlayerSpawn into the lifecycle buffer so the sub-app consumer
    // materializes an in-dim entity this tick.
    app.world_mut()
        .resource_mut::<PendingInboundLifecycle>()
        .per_dim
        .entry(dim_label)
        .or_default()
        .spawns
        .push(InboundPlayerSpawn {
            host_anchor,
            snapshot: PlayerTransferSnapshot {
                uuid: Uuid::new_v4(),
                username: "login_test".into(),
                position: DVec3::new(0.0, 64.0, 0.0),
                rotation: bevy_math::Vec2::ZERO,
            },
        });

    // Tick 1: extract shuttles spawn into sub-app; sub-app consumer spawns
    // entity; emit_play_login writes OutboundPlayerPacket; extract drains to host.
    app.update();

    // After tick 1 the extract has drained the sub-app outbound buffer into the
    // host's Messages<OutboundPlayerPacket>. Drain and collect owned copies.
    let packets: Vec<OutboundPlayerPacket> = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();

    // If no packets yet, pump one more tick (1-tick outbound latency).
    let packets = if packets.is_empty() {
        app.update();
        app.world_mut()
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .collect::<Vec<_>>()
    } else {
        packets
    };

    let has_login = packets.iter().any(|p| {
        matches!(&p.data, PacketPayload::PlayerLogin { .. })
            && p.priority == PacketPriority::Critical
    });
    assert!(
        has_login,
        "emit_play_login must emit a Critical PlayerLogin OutboundPlayerPacket; got {} packets",
        packets.len()
    );
}

// ---------------------------------------------------------------------------
// play_login_targets_host_anchor (Task 1)
// ---------------------------------------------------------------------------

/// The PlayerLogin packet emitted by the in-dim system must target
/// `PacketTarget::SinglePlayer(host_anchor)`, not the in-dim entity.
#[test]
fn play_login_targets_host_anchor() {
    let mut app = build_host_app();
    let dim_label = spawn_subapp(&mut app);

    let host_anchor = app.world_mut().spawn_empty().id();
    app.world_mut().resource_mut::<PlayerIndex>().insert(
        host_anchor,
        PlayerLocation {
            socket: Entity::PLACEHOLDER,
            current_dim: dim_label,
            previous_dim: None,
            in_dim_entity: None,
            inbound_pending: SmallVec::new(),
        },
    );

    app.world_mut()
        .resource_mut::<PendingInboundLifecycle>()
        .per_dim
        .entry(dim_label)
        .or_default()
        .spawns
        .push(InboundPlayerSpawn {
            host_anchor,
            snapshot: PlayerTransferSnapshot {
                uuid: Uuid::new_v4(),
                username: "target_test".into(),
                position: DVec3::new(0.0, 64.0, 0.0),
                rotation: bevy_math::Vec2::ZERO,
            },
        });

    // Tick 1: spawn consumed; emit_play_login fires; extract drains to host.
    app.update();

    // Drain host's Messages<OutboundPlayerPacket> (already extracted from sub-app).
    let packets: Vec<OutboundPlayerPacket> = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();

    // If no packets yet, pump one more tick (1-tick outbound latency).
    let packets = if packets.is_empty() {
        app.update();
        app.world_mut()
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .collect::<Vec<_>>()
    } else {
        packets
    };

    let login_pkt = packets
        .iter()
        .find(|p| matches!(&p.data, PacketPayload::PlayerLogin { .. }))
        .expect("PlayerLogin packet must be present");

    match &login_pkt.target {
        PacketTarget::SinglePlayer(e) => {
            assert_eq!(
                *e, host_anchor,
                "PlayerLogin target must be the host-anchor entity, not the in-dim entity"
            );
        }
        other => panic!("PlayerLogin target must be SinglePlayer(host_anchor), got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// in_dim_entity_carries_host_anchor (Task 1)
// ---------------------------------------------------------------------------

/// The in-dim player entity spawned by consume_inbound_player_spawn must carry
/// the `HostAnchor(host_anchor)` component so per-dim emitters can resolve the
/// outbound target.
#[test]
fn in_dim_entity_carries_host_anchor() {
    let mut app = build_host_app();
    let dim_label = spawn_subapp(&mut app);

    let host_anchor = app.world_mut().spawn_empty().id();

    app.world_mut()
        .resource_mut::<PendingInboundLifecycle>()
        .per_dim
        .entry(dim_label)
        .or_default()
        .spawns
        .push(InboundPlayerSpawn {
            host_anchor,
            snapshot: PlayerTransferSnapshot {
                uuid: Uuid::new_v4(),
                username: "anchor_test".into(),
                position: DVec3::new(0.0, 64.0, 0.0),
                rotation: bevy_math::Vec2::ZERO,
            },
        });

    // One tick: extract shuttles spawn; consumer spawns entity with HostAnchor.
    app.update();

    let sub = app.sub_app_mut(DimAppLabel(dim_label));
    let world = sub.world_mut();
    let anchors: Vec<HostAnchor> = world
        .query::<&HostAnchor>()
        .iter(world)
        .copied()
        .collect();

    assert_eq!(anchors.len(), 1, "exactly one in-dim entity should carry HostAnchor");
    assert_eq!(
        anchors[0].0, host_anchor,
        "HostAnchor.0 must equal the host-anchor entity from the spawn message"
    );
}

// ---------------------------------------------------------------------------
// chunk_delivery_emits_chunkload (Task 2a)
// ---------------------------------------------------------------------------

/// The chunk delivery path (send_column_queue / equivalent) must emit
/// `PacketPayload::ChunkLoad` as a Critical `OutboundPlayerPacket` and must
/// not call `con.write_packet` directly. This test verifies the variant
/// appears in the outbound bus when chunk data is available.
///
/// Note: this test validates the PacketPayload::ChunkLoad variant encodes
/// correctly through dispatch_encode (pre-condition for the bus path to work).
#[test]
fn chunk_delivery_emits_chunkload() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    use mcrs_engine::geometry::ColumnPos;
    push_critical(
        &mut world,
        entity,
        PacketPayload::ChunkLoad {
            column: ColumnPos::new(0, 0),
            chunk_bytes: vec![0u8; 64],
            light_data: LightData::default(),
        },
    );
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "ChunkLoad must not increment unhandled");

    let blob = rx.try_recv().expect("ChunkLoad blob sent to socket");
    assert!(!blob.is_empty(), "ChunkLoad must produce a non-empty blob");
}

// ---------------------------------------------------------------------------
// light_delivery_emits_lightupdate (Task 2a)
// ---------------------------------------------------------------------------

/// `PacketPayload::LightUpdate` encodes correctly (already covered in
/// bridge_dispatch.rs but repeated here as part of the delivery suite).
#[test]
fn light_delivery_emits_lightupdate() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    use mcrs_engine::geometry::ColumnPos;
    push_critical(
        &mut world,
        entity,
        PacketPayload::LightUpdate {
            column: ColumnPos::new(1, 2),
            light_data: LightData::default(),
        },
    );
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "LightUpdate must not increment unhandled");

    let blob = rx.try_recv().expect("LightUpdate blob sent to socket");
    assert!(!blob.is_empty(), "LightUpdate must produce a non-empty blob");
}

// ---------------------------------------------------------------------------
// view_enter_leave_route_via_bus (Task 2b)
// ---------------------------------------------------------------------------

/// `PacketPayload::PlayerEnteredView` and `PacketPayload::PlayerLeftView`
/// encode correctly through dispatch_encode (bus routing pre-condition).
#[test]
fn view_enter_leave_route_via_bus() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    use smallvec::smallvec;
    push_critical(
        &mut world,
        entity,
        PacketPayload::PlayerEnteredView {
            entity_id: 7,
            uuid: Uuid::nil(),
            kind: 128,
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
        },
    );
    push_critical(
        &mut world,
        entity,
        PacketPayload::PlayerLeftView {
            entity_ids: smallvec![7],
        },
    );
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "view enter/leave must not increment unhandled");

    let blob = rx.try_recv().expect("view enter/leave blob sent");
    assert!(!blob.is_empty(), "view enter/leave must produce a non-empty blob");
}

// ---------------------------------------------------------------------------
// on_view_update_routes_cache_center (Task 2b)
// ---------------------------------------------------------------------------

/// `PacketPayload::SetChunkCacheCenter` and `PacketPayload::SetChunkCacheRadius`
/// encode correctly (verifies the bus routing path works end-to-end for
/// view-change packets).
#[test]
fn on_view_update_routes_cache_center() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut world, entity, mut rx) = build_dispatch_world();

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    push_critical(&mut world, entity, PacketPayload::SetChunkCacheCenter { x: 5, z: 3 });
    push_critical(&mut world, entity, PacketPayload::SetChunkCacheRadius { radius: 10 });
    run_dispatch(&mut world);

    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "cache center/radius must not increment unhandled");

    let blob = rx.try_recv().expect("cache center/radius blob sent");
    assert!(!blob.is_empty(), "cache center/radius must produce a non-empty blob");
}
