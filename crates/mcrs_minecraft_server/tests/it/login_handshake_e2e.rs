//! Full production-topology join without a real TCP socket: the host emits
//! `InboundPlayerSpawn` on the Game transition, the sub-app materializes the
//! in-dim entity, `emit_play_login` writes `PacketPayload::PlayerLogin`, and
//! `bridge_outbound` + `dispatch_encode` coalesce a non-empty blob to the mock
//! socket. This is the regression test that would have caught the original
//! "Joining world" hang: it exercises a real connection crossing into a sub-app
//! and verifies the play-login blob reaches the host-resident connection, not
//! just the in-process bus.

use crate::mock_connection;


use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_level::session::{Place, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::configuration::emit_initial_player_spawn;
use mcrs_minecraft_server::dim::pump_channels;
use mcrs_minecraft_server::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft_server::world::bridge::{
    bridge_inbound_to_channel, bridge_outbound, bridge_player_attach, dispatch_encode,
    forward_pending_inbound,
};
use mcrs_minecraft_server::world::bridge_queue::{InboundRateBucket, OutboundQueue};
use mcrs_minecraft_server::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket,
};
use mcrs_minecraft_server::world::session::HostAnchorRef;
use mcrs_minecraft_server::world::sub_app_builder::drain_dim_spawn_queue;

use crate::support;

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
    support::insert_registries(&mut app);

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

