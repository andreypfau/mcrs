//! CI-testable subset of the BRIDGE-09 E2E gate.
//!
//! Four in-process integration tests exercise the steady-state path without a
//! real TCP socket:
//!
//! - `e2e_login_handshake_completes`: synthetic login → `PlayerIndex` entry + `HostAnchorRef`.
//! - `e2e_packet_round_trip`: outbound packet injected via `Messages<OutboundPlayerPacket>`;
//!   asserts it travels through `bridge_outbound` → `OutboundQueue` → `dispatch_encode` →
//!   blob on the mock socket channel within 2 ticks.
//! - `e2e_aoi_surrounding_update`: two players in the same dim; one enters the other's
//!   view range; asserts `TrackedBy` update (AOI-02).
//! - `e2e_join_releases_joining_world`: full production-topology join — host emits
//!   `InboundPlayerSpawn` on Game transition → sub-app materializes the in-dim entity →
//!   `emit_play_login` writes `PacketPayload::PlayerLogin` → `bridge_outbound` +
//!   `dispatch_encode` coalesce a non-empty blob to the mock socket. This is the regression
//!   test that would have caught the original "Joining world" hang: it exercises a real
//!   connection crossing into a sub-app and verifies the play-login blob reaches the
//!   host-resident connection, not just the in-process bus.

#[path = "common/mock_connection.rs"]
mod mock_connection;

mod harness;

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::snapshot::RegistrySnapshot;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::configuration::emit_initial_player_spawn;
use mcrs_minecraft::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft::world::aoi::TrackedBy;
use mcrs_minecraft::world::bridge::{
    bridge_outbound, bridge_player_attach, dispatch_encode, partition_main_inbound,
};
use mcrs_minecraft::world::bridge_queue::{InboundRateBucket, OutboundQueue};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer, PacketPayload,
    PacketPriority, PacketTarget, PendingInboundLifecycle, PendingInboundPartition,
};
use mcrs_minecraft::world::player_index::{HostAnchorRef, PlayerIndex, PlayerLocation};
use mcrs_minecraft::world::sub_app_builder::{drain_dim_spawn_queue, DimSubAppHandle};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_network::ServerSideConnection;
use mcrs_protocol::uuid::Uuid;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;
use smallvec::SmallVec;

// ---------------------------------------------------------------------------
// e2e_login_handshake_completes
// ---------------------------------------------------------------------------

/// A synthetic login drives the `LoginPlugin` observer chain so a connection
/// entity reaches the in-game state: `PlayerIndex` carries one entry with a
/// `HostAnchorRef` that points back to the connection entity.
///
/// Exercises BRIDGE-01/02/06/07 steady-state setup: the login path is the
/// prerequisite for any bridge packet routing.
#[test]
fn e2e_login_handshake_completes() {
    let mut app = App::new();
    app.add_plugins(LoginPlugin);
    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundLifecycle>();
    app.add_message::<InboundPlayerDespawn>();

    let connection_entity = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(connection_entity).insert((
        GameProfile {
            id: Uuid::new_v4(),
            username: "test_e2e".into(),
            properties: Vec::new(),
        },
        LoginState::Accepted,
    ));
    app.update();

    let world = app.world();

    assert_eq!(
        world.resource::<PlayerIndex>().len(),
        1,
        "PlayerIndex must have one entry after accepted login",
    );

    let host_anchor_ref = world
        .entity(connection_entity)
        .get::<HostAnchorRef>()
        .copied()
        .expect("connection entity must carry HostAnchorRef after login");

    assert!(
        world.get_entity(host_anchor_ref.0).is_ok(),
        "host-anchor entity must exist in the world",
    );

    let location = world
        .resource::<PlayerIndex>()
        .get(&host_anchor_ref.0)
        .expect("PlayerLocation must be present for host-anchor");

    assert_eq!(
        location.socket, connection_entity,
        "PlayerLocation.socket must point back at the connection entity",
    );
    assert!(
        location.in_dim_entity.is_none(),
        "newly logged-in player must not yet be placed in a dim",
    );
}

// ---------------------------------------------------------------------------
// e2e_packet_round_trip
// ---------------------------------------------------------------------------

/// An outbound packet written into `Messages<OutboundPlayerPacket>` by a
/// DimSubApp simulation system eventually reaches the mock socket as an
/// encoded blob after running through `bridge_outbound` (push to
/// `OutboundQueue`) and `dispatch_encode` (encode + coalesce + try_send).
///
/// Asserts the round-trip completes within the same tick (the two systems are
/// run sequentially in this test, mirroring their `FixedPostUpdate` order in
/// production). Uses `BlockUpdate` (a MAPPED variant that produces real bytes)
/// so the blob is non-empty.
#[test]
fn e2e_packet_round_trip() {
    use mcrs_engine::geometry::BlockPos;
    use mcrs_protocol::BlockStateId;

    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<PlayerIndex>();
    world.init_resource::<PendingInboundPartition>();

    let player = Entity::from_raw_u32(1).expect("nonzero");
    let dim = Entity::from_raw_u32(2).expect("nonzero");

    let (raw, mut rx) = mock_connection::make_mock_raw_connection();
    let socket = world
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            OutboundQueue::default(),
            InboundRateBucket::new(),
        ))
        .id();

    world.resource_mut::<PlayerIndex>().insert(
        player,
        PlayerLocation {
            socket,
            current_dim: dim,
            previous_dim: None,
            in_dim_entity: Some(socket),
            inbound_pending: SmallVec::new(),
        },
    );

    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(player),
            priority: PacketPriority::Normal,
            data: PacketPayload::BlockUpdate {
                position: BlockPos::new(0, 64, 0),
                new_state: BlockStateId(1),
            },
        });

    run_system(&mut world, bridge_outbound);

    let queue = world
        .get::<OutboundQueue>(socket)
        .expect("OutboundQueue must exist");
    assert_eq!(
        queue.total_len(),
        1,
        "bridge_outbound must push the packet into the connection's OutboundQueue",
    );

    run_system(&mut world, dispatch_encode);

    let blob = rx
        .try_recv()
        .expect("dispatch_encode must coalesce and send exactly one blob to the socket channel");
    assert!(
        !blob.is_empty(),
        "the encoded blob must be non-empty for a BlockUpdate packet",
    );

    assert!(
        rx.try_recv().is_err(),
        "dispatch_encode must send exactly one blob per tick (coalescing contract)",
    );
}

// ---------------------------------------------------------------------------
// e2e_aoi_surrounding_update
// ---------------------------------------------------------------------------

/// Two players in the same dimension whose positions are within tracking range
/// both end up in each other's `TrackedBy` set after the AoI tick pair runs.
///
/// Covers AOI-02 (surrounding-players set populated during transfer context).
/// Reuses the `make_aoi_app` / `drive_aoi_tick` / `spawn_player_in_dim` helpers
/// from the harness module, which are the same helpers the other AoI tests use.
#[test]
fn e2e_aoi_surrounding_update() {
    use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};
    use mcrs_engine::geometry::ColumnPos;
    use mcrs_engine::world::dimension::DimensionBundle;

    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();

    let player_a = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));
    let player_b = spawn_player_in_dim(&mut app, dim, DVec3::new(32.0, 64.0, 0.0));

    seed_columns(&mut app, dim, ColumnPos::new(0, 0), 16);
    seed_columns(&mut app, dim, ColumnPos::new(2, 0), 16);

    // Tick 1: AoI substrate wires observer subscriptions.
    drive_aoi_tick(&mut app);

    // Tick 2: nudge both transforms so both players' update_tracked_by
    // bodies fire (Changed<Transform> predicate) and read the populated
    // PlayerObservers from tick 1.
    nudge(&mut app, player_a);
    nudge(&mut app, player_b);
    drive_aoi_tick(&mut app);

    let world = app.world();
    let tracked_a = world
        .get::<TrackedBy>(player_a)
        .expect("player_a must have TrackedBy component");
    let tracked_b = world
        .get::<TrackedBy>(player_b)
        .expect("player_b must have TrackedBy component");

    assert!(
        tracked_a.0.contains(&player_b),
        "player_a's TrackedBy must include player_b after both are in view range; got {:?}",
        tracked_a.0.as_slice(),
    );
    assert!(
        tracked_b.0.contains(&player_a),
        "player_b's TrackedBy must include player_a after both are in view range; got {:?}",
        tracked_b.0.as_slice(),
    );
}

// ---------------------------------------------------------------------------
// e2e_join_releases_joining_world
// ---------------------------------------------------------------------------

/// Build a host `App` with the full join + delivery stack wired as systems
/// so a real `app.update()` pump exercises the entire production path.
///
/// Compared with `build_host_app` in `host_subapp_handoff.rs`, this variant
/// additionally registers `bridge_outbound` and `dispatch_encode` so that
/// outbound packets flow all the way to the mock socket.
fn build_join_host_app() -> App {
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
    {
        let state_count = 2usize;
        let emission = vec![0u8; state_count].into_boxed_slice();
        let dampening = vec![0u8; state_count].into_boxed_slice();
        let occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let flags = vec![0u8; state_count].into_boxed_slice();
        app.insert_resource(BlockStateLightTable { emission, dampening, occlusion, flags });
    }
    app.insert_resource(StaticRegistry::<Block>::new());
    app.insert_resource(StaticRegistry::<EnchantmentData>::default());
    app.insert_resource(TagRegistry::<Block>::default());
    app.insert_resource(RegistrySnapshot::<Biome>::default());

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

    app.add_systems(
        Update,
        (partition_main_inbound, bridge_player_attach, bridge_outbound, dispatch_encode),
    );
    app.add_plugins(LoginPlugin);
    app.add_systems(Update, emit_initial_player_spawn);

    app
}

/// Full production-topology regression for the "Joining world" release.
///
/// This is the test that would have caught the original failure: it exercises
/// a real connection crossing into a sub-app (`InboundPlayerSpawn` → in-dim
/// entity via `consume_inbound_player_spawn`) and verifies that the play-login
/// packet (`PacketPayload::PlayerLogin`) is encoded and delivered as a non-empty
/// blob to the joining player's host-resident mock socket connection, which is
/// what releases the client from the "Joining world" screen.
///
/// Asserts:
/// 1. `PlayerIndex.in_dim_entity` becomes `Some` (handoff bound).
/// 2. A non-empty blob reaches the mock socket channel (play-login delivered).
#[test]
fn e2e_join_releases_joining_world() {
    use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
    use mcrs_network::ConnectionState;
    use mcrs_network::InGameConnectionState;

    let mut app = build_join_host_app();

    // Spawn a connection entity with a mock socket so dispatch_encode can
    // coalesce and deliver blobs to it.
    let (raw, mut rx) = mock_connection::make_mock_raw_connection();
    let connection_entity = app
        .world_mut()
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            OutboundQueue::default(),
            InboundRateBucket::new(),
        ))
        .id();

    // Drive login: insert GameProfile + LoginState::Accepted so the
    // on_login_accepted observer creates the host-anchor + PlayerIndex entry.
    app.world_mut().entity_mut(connection_entity).insert((
        GameProfile {
            id: Uuid::new_v4(),
            username: "e2e_join_test".into(),
            properties: Vec::new(),
        },
        LoginState::Accepted,
    ));
    // Flush the on_login_accepted observer.
    app.update();

    let host_anchor = app
        .world()
        .entity(connection_entity)
        .get::<HostAnchorRef>()
        .copied()
        .expect("HostAnchorRef present after login")
        .0;

    // Make PlayerIndex.socket point at the mock connection entity so
    // bridge_outbound and dispatch_encode route blobs to it.
    {
        let mut index = app.world_mut().resource_mut::<PlayerIndex>();
        if let Some(loc) = index.get_mut(&host_anchor) {
            loc.socket = connection_entity;
        }
    }

    // Bring up the server and spawn a real sub-app.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
    drain_dim_spawn_queue(&mut app);

    // Transition to Game — mirrors what on_configuration_ack does in production.
    app.world_mut()
        .entity_mut(connection_entity)
        .insert((ConnectionState::Game, InGameConnectionState));

    // Tick 1: emit_initial_player_spawn fills PendingInboundLifecycle →
    //         extract shuttles spawn into sub-app Messages<InboundPlayerSpawn> →
    //         sub-app consume_inbound_player_spawn spawns the in-dim entity,
    //         writes OutboundPlayerAttached + emit_play_login OutboundPlayerPacket(s).
    app.update();

    // Tick 2: extract drains sub-app OutboundPlayerAttached → host
    //         Messages<OutboundPlayerAttached>; bridge_player_attach sets
    //         in_dim_entity; extract drains OutboundPlayerPacket(s) to host bus;
    //         bridge_outbound routes them to OutboundQueue on the mock connection;
    //         dispatch_encode encodes and sends the blob.
    app.update();

    // Tick 3: second dispatch window — catches any packets queued on tick 2.
    app.update();

    // Assertion 1: handoff completed — in_dim_entity bound.
    let location = app
        .world()
        .resource::<PlayerIndex>()
        .get(&host_anchor)
        .expect("PlayerLocation present");
    assert!(
        location.in_dim_entity.is_some(),
        "PlayerIndex.in_dim_entity must be Some after the full handoff round-trip",
    );

    // Assertion 2: play-login delivered — at least one non-empty blob on the socket.
    let blob = rx
        .try_recv()
        .expect("dispatch_encode must have sent at least one blob to the mock socket; \
                 play-login not delivered — 'Joining world' would hang");
    assert!(
        !blob.is_empty(),
        "the blob reaching the mock socket must be non-empty (play-login bytes)",
    );
}

// ---------------------------------------------------------------------------
// Shared test utilities
// ---------------------------------------------------------------------------

fn run_system<S, Marker>(world: &mut World, system: S)
where
    S: IntoSystem<(), (), Marker>,
{
    let mut sys = IntoSystem::into_system(system);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

fn seed_columns(app: &mut App, dim: Entity, centre: mcrs_engine::geometry::ColumnPos, radius: i32) {
    use mcrs_engine::aoi::PlayerObservers;
    use mcrs_engine::geometry::ColumnPos;
    use mcrs_engine::world::dimension::InDimension;
    use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};

    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let pos = ColumnPos::new(centre.x + dx, centre.z + dz);
            let exists = app
                .world()
                .get::<ColumnIndex>(dim)
                .map(|idx| idx.0.contains_key(&pos))
                .unwrap_or(false);
            if exists {
                continue;
            }
            let column = app
                .world_mut()
                .spawn((Column, PlayerObservers::default(), InDimension(dim)))
                .id();
            app.world_mut()
                .get_mut::<ColumnIndex>(dim)
                .expect("dim has ColumnIndex")
                .0
                .insert(pos, ColumnSlot { entity: column, section_count: 1 });
        }
    }
}

fn nudge(app: &mut App, entity: Entity) {
    use mcrs_engine::entity::physics::Transform;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .expect("entity has Transform")
        .translation
        .x += 0.001;
}
