//! CI-testable subset of the BRIDGE-09 E2E gate.
//!
//! Four in-process integration tests exercise the steady-state path without a
//! real TCP socket:
//!
//! - `e2e_login_handshake_completes`: synthetic login → a session + `HostAnchorRef`.
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

use crate::mock_connection;

use crate::harness;

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_assets::snapshot::RegistrySnapshot;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::Block;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_level::session::{Place, PlayerSession, Session, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registry::static_registry::StaticRegistry;
use mcrs_minecraft_server::configuration::emit_initial_player_spawn;
use mcrs_minecraft_server::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft_server::runner::pump_channels;
use mcrs_minecraft_server::world::aoi::TrackedBy;
use mcrs_minecraft_server::world::bridge::{
    bridge_inbound_to_channel, bridge_outbound, bridge_player_attach, dispatch_encode,
    forward_pending_inbound,
};
use mcrs_minecraft_server::world::bridge_queue::{InboundRateBucket, OutboundQueue};
use mcrs_minecraft_server::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use mcrs_minecraft_server::world::session::{HostAnchorRef, SessionBundle, SessionConnection};
use mcrs_minecraft_server::world::sub_app_builder::drain_dim_spawn_queue;

use crate::support;

// ---------------------------------------------------------------------------
// e2e_login_handshake_completes
// ---------------------------------------------------------------------------

/// A synthetic login drives the `LoginPlugin` observer chain so a connection
/// entity reaches the in-game state: one session exists, on the host anchor
/// the connection's `HostAnchorRef` points at.
///
/// Exercises BRIDGE-01/02/06/07 steady-state setup: the login path is the
/// prerequisite for any bridge packet routing.
#[test]
fn e2e_login_handshake_completes() {
    let mut app = App::new();
    app.add_plugins(LoginPlugin);
    app.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
    app.init_resource::<mcrs_minecraft_level::session::PlayerSessionCounter>();
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

    let session_count = app
        .world_mut()
        .query::<&Session>()
        .iter(app.world())
        .count();
    assert_eq!(
        session_count, 1,
        "one session must exist after accepted login"
    );

    let world = app.world();
    let host_anchor_ref = world
        .entity(connection_entity)
        .get::<HostAnchorRef>()
        .copied()
        .expect("connection entity must carry HostAnchorRef after login");

    let anchor = world
        .get_entity(host_anchor_ref.0)
        .expect("host-anchor entity must exist in the world");

    assert_eq!(
        anchor
            .get::<SessionConnection>()
            .expect("the host anchor must know its connection")
            .entity(),
        connection_entity,
        "the session's connection must be the connection entity",
    );
    assert_eq!(
        anchor
            .get::<SessionPlacement>()
            .map(SessionPlacement::place),
        Some(Place::Unplaced),
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
    use mcrs_minecraft_core::BlockPos;
    use mcrs_minecraft_registry::BlockStateId;

    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();

    let dim = Entity::from_raw_u32(2).expect("nonzero");

    let (raw, mut rx) = mock_connection::make_mock_raw_connection();
    let socket = world
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            OutboundQueue::default(),
            InboundRateBucket::new(),
        ))
        .id();

    let session = PlayerSession(1);
    let host_anchor = world
        .spawn(SessionBundle::placed(
            session,
            SessionPlacement::new(Place::InDim(dim), 0),
        ))
        .id();
    world.entity_mut(socket).insert(HostAnchorRef(host_anchor));

    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host_anchor),
            priority: PacketPriority::Normal,
            data: PacketPayload::BlockUpdate {
                position: BlockPos::new(0, 64, 0),
                new_state: BlockStateId(1),
            },
            session,
            epoch: 0,
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
    use mcrs_minecraft_core::ColumnPos;
    use mcrs_minecraft_level::world::dimension::{
        DimensionBundle, DimensionId, DimensionTypeConfig,
    };

    let mut app = make_aoi_app();
    let dim = app
        .world_mut()
        .spawn(DimensionBundle::new(
            DimensionId::new("minecraft:overworld"),
            DimensionTypeConfig::new(-64, 384),
        ))
        .id();

    let player_a = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));
    let player_b = spawn_player_in_dim(&mut app, dim, DVec3::new(32.0, 64.0, 0.0));

    seed_columns(&mut app, dim, ColumnPos::new(0, 0), 16);
    seed_columns(&mut app, dim, ColumnPos::new(2, 0), 16);

    // Tick 1: the mirror lists both players on the columns they hold.
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
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(StaticRegistry::<EnchantmentData>::default());
    app.insert_resource(DynTagRegistry::<Block>::default());
    app.insert_resource(RegistrySnapshot::<Biome>::default());
    support::insert_corpus(&mut app);

    app.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
    app.init_resource::<mcrs_minecraft_level::session::PlayerSessionCounter>();
    app.init_resource::<mcrs_minecraft_server::world::channel_types::DimChannelsResource>();
    app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app.add_systems(
        Update,
        (
            bridge_inbound_to_channel,
            bridge_player_attach,
            forward_pending_inbound,
            bridge_outbound,
            dispatch_encode,
        )
            .chain(),
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
/// 1. The session is attached to its dimension (handoff bound).
/// 2. A non-empty blob reaches the mock socket channel (play-login delivered).
#[test]
fn e2e_join_releases_joining_world() {
    use mcrs_minecraft_level::world::dimension::{DimensionId, DimensionTypeConfig};
    use mcrs_minecraft_network::ConnectionState;

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
    // on_login_accepted observer creates the host-anchor and its session.
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
            type_config: DimensionTypeConfig::new(-64, 384),
            has_sky: true,
        });
    drain_dim_spawn_queue(&mut app);

    // Transition to Game — mirrors what on_configuration_ack does in production.
    app.world_mut()
        .entity_mut(connection_entity)
        .insert(ConnectionState::Game);

    // Tick 1: emit_initial_player_spawn sends ToDim::Spawn on the control channel.
    //         Sub-app extract runs (no output yet), then sub-app FixedPreUpdate
    //         drain_to_dim_inbox routes Spawn → Messages<InboundPlayerSpawn>.
    //         FixedLast flush_from_dim_outbox runs before Update (empty outbox).
    //         Update consume_inbound_player_spawn spawns the in-dim entity,
    //         writes OutboundPlayerAttached + play-login OutboundPlayerPacket(s).
    //         pump_channels: from_dim channel empty (flush ran before spawn wrote).
    app.update();
    pump_channels(&mut app);

    // Tick 2: extract drains sub-app Messages<OutboundPlayerAttached> (written tick 1)
    //         → host Messages<OutboundPlayerAttached>. Sub-app FixedLast
    //         flush_from_dim_outbox now drains the play-login packet(s) into the
    //         FromDim channel. pump_channels drains the channel into host
    //         Messages<OutboundPlayerPacket> with correct session stamp.
    app.update();
    pump_channels(&mut app);

    // Tick 3: main First swaps host Messages; bridge_player_attach sees
    //         OutboundPlayerAttached → attaches the session; bridge_outbound sees
    //         play-login packets → routes to OutboundQueue; dispatch_encode encodes
    //         and sends the blob.
    app.update();
    pump_channels(&mut app);

    // Assertion 1: handoff completed — the session is attached.
    let place = app
        .world()
        .get::<SessionPlacement>(host_anchor)
        .expect("session present")
        .place();
    assert!(
        matches!(place, Place::InDim(_)),
        "the session must be attached to its dim after the full handoff round-trip, got {place:?}",
    );

    // Assertion 2: play-login delivered — at least one non-empty blob on the socket.
    let blob = rx.try_recv().expect(
        "dispatch_encode must have sent at least one blob to the mock socket; \
                 play-login not delivered — 'Joining world' would hang",
    );
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

fn seed_columns(app: &mut App, dim: Entity, centre: mcrs_minecraft_core::ColumnPos, radius: i32) {
    use mcrs_minecraft_core::ColumnPos;
    use mcrs_minecraft_level::aoi::PlayerObservers;
    use mcrs_minecraft_level::world::dimension::InDimension;
    use mcrs_minecraft_level::world::storage::column::{Column, ColumnIndex, ColumnSlot};

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
                .insert(
                    pos,
                    ColumnSlot {
                        entity: column,
                        section_count: 1,
                    },
                );
        }
    }
}

fn nudge(app: &mut App, entity: Entity) {
    use mcrs_minecraft_level::entity::physics::Transform;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .expect("entity has Transform")
        .translation
        .x += 0.001;
}
