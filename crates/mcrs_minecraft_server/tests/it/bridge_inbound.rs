//! Integration tests for `bridge_inbound` (per-connection rate limiting,
//! ReceivedPacketEvent emission) and `disconnect_clears_pending` (teardown
//! leak check).

use crate::mock_connection;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{IntoSystem, RunSystemOnce, System};
use bevy_ecs::world::World;
use mcrs_minecraft_level::session::{Place, PlayerSessionCounter, Session, SessionPlacement};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::event::run_event_loop;
use mcrs_minecraft_network::metrics::{BridgeTelemetry, GameDecodeCounts, PreGameDecodeCounts};
use mcrs_minecraft_network::{ConnectionState, ReceivedPacket, ServerSideConnection};
use mcrs_minecraft_protocol::packets::table::{game_serverbound, row_of};
use mcrs_minecraft_server::world::bridge::bridge_inbound;
use mcrs_minecraft_server::world::bridge_queue::{
    INBOUND_BUCKET_CAP, INBOUND_KICK_OVERFLOW_PACKETS, InboundRateBucket, OutboundQueue,
};
use mcrs_minecraft_server::world::bus::{InboundPlayerPacket, OutboundPlayerPacket};
use mcrs_minecraft_server::world::channel_types::DimChannelsResource;
use mcrs_minecraft_server::world::session::{HostAnchorRef, SessionBundle};

use std::time::Instant;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_inbound_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<Messages<InboundPlayerPacket>>();
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<DimChannelsResource>();
    world.init_resource::<BridgeTelemetry>();
    world.init_resource::<GameDecodeCounts>();
    world
}

/// Spawn a connection entity in the game state with InboundRateBucket +
/// OutboundQueue and a mock RawConnection that we can inject packets into.
///
/// Returns `(socket_entity, inbound_tx)`. Send `ReceivedPacket` values into
/// `inbound_tx` to simulate packets arriving from the client.
fn spawn_ingame_connection(world: &mut World) -> (Entity, mpsc::Sender<ReceivedPacket>) {
    let (raw, _outgoing_rx, inbound_tx) = mock_connection::make_mock_raw_connection_full();
    let entity = world
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            ConnectionState::Game,
            InboundRateBucket::new(),
            OutboundQueue::default(),
        ))
        .id();
    (entity, inbound_tx)
}

/// Log a player in, attached to `dim` or, without an in-dim entity yet, still
/// joining it. Returns the host anchor carrying the session.
fn register_player(world: &mut World, dim: Entity, in_dim_entity: Option<Entity>) -> Entity {
    let session = world.resource_mut::<PlayerSessionCounter>().next();
    let place = match in_dim_entity {
        Some(_) => Place::InDim(dim),
        None => Place::Joining(dim),
    };
    world
        .spawn(SessionBundle::placed(
            session,
            SessionPlacement::new(place, 0),
        ))
        .id()
}

/// Attach a `HostAnchorRef` to the given socket entity pointing at `player`.
fn attach_anchor(world: &mut World, socket: Entity, player: Entity) {
    world.entity_mut(socket).insert(HostAnchorRef(player));
}

fn make_received_packet(seq: u32) -> ReceivedPacket {
    ReceivedPacket {
        timestamp: Instant::now(),
        id: seq as i32,
        payload: bytes::Bytes::new(),
    }
}

fn run_inbound(world: &mut World) {
    let mut sys = IntoSystem::into_system(bridge_inbound);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

// ---------------------------------------------------------------------------
// Counter resource used to verify ReceivedPacketEvent triggers
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
struct EventCounter {
    count: usize,
}

// ---------------------------------------------------------------------------
// bridge_inbound_emits_received_packet_event
//
// Replaces the old routing test: bridge_inbound no longer routes into
// PendingInboundPartition directly. Instead it re-emits ReceivedPacketEvent
// for each drained in-game packet so host-side observers (keepalive, movement,
// chat) fire. This test verifies that exactly one event fires per drained
// packet across two independent connections.
// ---------------------------------------------------------------------------

#[test]
fn bridge_inbound_emits_received_packet_event() {
    let mut world = build_inbound_world();
    world.init_resource::<EventCounter>();

    // Register an observer that counts ReceivedPacketEvent triggers.
    world.add_observer(
        |_ev: On<ReceivedPacketEvent>, mut counter: bevy_ecs::system::ResMut<EventCounter>| {
            counter.count += 1;
        },
    );

    let dim_a = Entity::from_raw_u32(10).expect("nonzero");
    let dim_b = Entity::from_raw_u32(11).expect("nonzero");
    let in_dim_a = Entity::from_raw_u32(30).expect("nonzero");
    let in_dim_b = Entity::from_raw_u32(31).expect("nonzero");

    let (_socket_a, tx_a) = spawn_ingame_connection(&mut world);
    let (_socket_b, tx_b) = spawn_ingame_connection(&mut world);

    register_player(&mut world, dim_a, Some(in_dim_a));
    register_player(&mut world, dim_b, Some(in_dim_b));

    // Inject one packet into each connection.
    tx_a.try_send(make_received_packet(1)).unwrap();
    tx_b.try_send(make_received_packet(2)).unwrap();

    run_inbound(&mut world);

    let counter = world.resource::<EventCounter>();
    assert_eq!(
        counter.count, 2,
        "bridge_inbound must emit one ReceivedPacketEvent per drained packet"
    );

    // bridge_inbound no longer routes to PendingInboundPartition;
    // it only emits ReceivedPacketEvent. The count above is the full
    // verification needed.
}

// ---------------------------------------------------------------------------
// inbound_rate_kick
// ---------------------------------------------------------------------------

/// A burst past INBOUND_BUCKET_CAP by INBOUND_KICK_OVERFLOW_PACKETS kicks the connection (ServerSideConnection
/// removed) and increments kick_flood_total.
/// Packets received within the budget are NOT dropped.
#[test]
fn inbound_rate_kick() {
    let mut world = build_inbound_world();

    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let in_dim = Entity::from_raw_u32(30).expect("nonzero");

    let (socket, tx) = spawn_ingame_connection(&mut world);
    let player = register_player(&mut world, dim, Some(in_dim));
    attach_anchor(&mut world, socket, player);

    let before = world.resource::<BridgeTelemetry>().kick_flood_total;

    // Run enough ticks flooding packets to trigger the kick.
    // Each tick sends INBOUND_BUCKET_CAP + 1 packets to ensure bucket empties.
    for _ in 0..INBOUND_KICK_OVERFLOW_PACKETS + 1 {
        if world.get::<ServerSideConnection>(socket).is_none() {
            break;
        }
        for seq in 0..INBOUND_BUCKET_CAP + 10 {
            let _ = tx.try_send(make_received_packet(seq));
        }
        run_inbound(&mut world);
    }

    let after = world.resource::<BridgeTelemetry>().kick_flood_total;
    assert!(
        after > before,
        "kick_flood_total must increment on flood kick"
    );
    assert!(
        world.get::<ServerSideConnection>(socket).is_none(),
        "ServerSideConnection must be removed after flood kick"
    );
}

// ---------------------------------------------------------------------------
// no_unattached_outbound_queue_after_fixed_preupdate
// ---------------------------------------------------------------------------

/// After `attach_outbound_queue` runs (FixedPreUpdate), every
/// `ServerSideConnection` entity must also carry an `OutboundQueue`.
/// This asserts the spawn→attach ordering window is closed.
///
/// Test creates a connection entity WITHOUT an OutboundQueue, runs
/// `attach_outbound_queue`, and verifies the gap is closed.
#[test]
fn no_unattached_outbound_queue_after_fixed_preupdate() {
    use bevy_ecs::query::{With, Without};
    use mcrs_minecraft_server::world::bridge::attach_outbound_queue;

    let mut world = World::new();
    // Spawn a bare ServerSideConnection without OutboundQueue (simulates
    // a freshly spawned connection from spawn_new_raw_connections).
    let (raw, _rx) = mock_connection::make_mock_raw_connection();
    let socket = world
        .spawn(ServerSideConnection { raw: Box::new(raw) })
        .id();

    // Verify the gap exists before running.
    let gap_before = world
        .query_filtered::<Entity, (With<ServerSideConnection>, Without<OutboundQueue>)>()
        .iter(&world)
        .count();
    assert_eq!(
        gap_before, 1,
        "should have one unattached connection before attach"
    );

    // Run attach_outbound_queue (simulating FixedPreUpdate).
    let mut sys = IntoSystem::into_system(attach_outbound_queue);
    sys.initialize(&mut world);
    let _ = sys.run((), &mut world);
    sys.apply_deferred(&mut world);

    // After attach, no ServerSideConnection entity should lack OutboundQueue.
    let gap_after = world
        .query_filtered::<Entity, (With<ServerSideConnection>, Without<OutboundQueue>)>()
        .iter(&world)
        .count();
    assert_eq!(
        gap_after, 0,
        "no ServerSideConnection should lack OutboundQueue after attach_outbound_queue"
    );
    let _ = socket;
}

// ---------------------------------------------------------------------------
// disconnect_clears_pending
// ---------------------------------------------------------------------------

/// After `process_disconnect` runs, the session's host anchor is despawned
/// and `OutboundQueue` is removed from the socket entity. Neither leaks past
/// the disconnect tick.
#[test]
fn disconnect_clears_pending() {
    use mcrs_minecraft_server::disconnect::{LeavingSessions, process_disconnect};

    let mut world = World::new();
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<DimChannelsResource>();

    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let in_dim = Entity::from_raw_u32(30).expect("nonzero");

    let (socket, _tx) = spawn_ingame_connection(&mut world);
    let player = register_player(&mut world, dim, Some(in_dim));
    attach_anchor(&mut world, socket, player);

    assert!(
        world.get::<Session>(player).is_some(),
        "session should exist before disconnect"
    );

    // Verify OutboundQueue exists on socket.
    assert!(
        world.get::<OutboundQueue>(socket).is_some(),
        "OutboundQueue should exist before disconnect"
    );

    // Run process_disconnect (simulating the observer path).
    world
        .run_system_once(
            move |mut commands: bevy_ecs::prelude::Commands,
                  mut sessions: LeavingSessions,
                  dim_channels: bevy_ecs::system::Res<DimChannelsResource>| {
                process_disconnect(
                    player,
                    &mut sessions,
                    &dim_channels,
                    &mut mcrs_minecraft_level::world::sub_app::DimDespawnQueue::default(),
                    &mut commands,
                );
            },
        )
        .expect("process_disconnect system ran");

    assert!(
        world.get_entity(player).is_err(),
        "the session must be gone after disconnect"
    );

    // OutboundQueue must be removed from the socket entity.
    assert!(
        world.get::<OutboundQueue>(socket).is_none(),
        "OutboundQueue must be removed from socket entity after disconnect"
    );
}

// ---------------------------------------------------------------------------
// Decode outcomes of inbound frames
// ---------------------------------------------------------------------------

fn frame(id: i32, payload: &[u8]) -> ReceivedPacket {
    ReceivedPacket {
        timestamp: Instant::now(),
        id,
        payload: bytes::Bytes::copy_from_slice(payload),
    }
}

fn keep_alive_row() -> i32 {
    row_of(game_serverbound::NAMES, "keep_alive")
}

fn row_without_structure() -> i32 {
    game_serverbound::NAMES
        .iter()
        .position(|name| !game_serverbound::TYPED.contains(name))
        .expect("a serverbound game row without a structure") as i32
}

fn game_counts(world: &World) -> &mcrs_minecraft_network::metrics::TableCounts {
    world.resource::<GameDecodeCounts>().counts()
}

#[derive(Resource, Default)]
struct DeliveredIds(Vec<i32>);

#[test]
fn an_unknown_id_is_counted_and_the_connection_survives() {
    let mut world = build_inbound_world();
    let (socket, tx) = spawn_ingame_connection(&mut world);
    let past_the_table = game_serverbound::NAMES.len() as i32;
    tx.try_send(frame(past_the_table, &[1, 2, 3])).unwrap();
    tx.try_send(frame(-1, &[])).unwrap();

    run_inbound(&mut world);

    assert_eq!(game_counts(&world).unknown(), 2);
    assert_eq!(game_counts(&world).failures_total(), 0);
    assert!(world.entity(socket).contains::<ServerSideConnection>());
}

#[test]
fn a_failed_decode_is_counted_under_its_packet() {
    let mut world = build_inbound_world();
    let (_socket, tx) = spawn_ingame_connection(&mut world);
    tx.try_send(frame(keep_alive_row(), &[0, 0, 0])).unwrap();
    tx.try_send(frame(keep_alive_row(), &[0; 9])).unwrap();

    run_inbound(&mut world);

    let counts = game_counts(&world);
    assert_eq!(counts.failures(keep_alive_row() as usize), 2);
    assert_eq!(counts.failures_total(), 2);
    assert_eq!(counts.unknown(), 0);
}

#[test]
fn a_row_without_a_structure_counts_nothing() {
    let mut world = build_inbound_world();
    let (_socket, tx) = spawn_ingame_connection(&mut world);
    tx.try_send(frame(row_without_structure(), &[5; 12]))
        .unwrap();
    tx.try_send(frame(keep_alive_row(), &7i64.to_be_bytes()))
        .unwrap();

    run_inbound(&mut world);

    assert_eq!(game_counts(&world).failures_total(), 0);
    assert_eq!(game_counts(&world).unknown(), 0);
}

#[test]
fn each_loop_counts_only_its_own_connections() {
    let mut world = build_inbound_world();
    world.init_resource::<PreGameDecodeCounts>();
    let (_game, game_tx) = spawn_ingame_connection(&mut world);
    let (raw, _outgoing_rx, login_tx) = mock_connection::make_mock_raw_connection_full();
    world.spawn((
        ServerSideConnection { raw: Box::new(raw) },
        ConnectionState::Login,
    ));
    let unknown_everywhere = 10_000;
    game_tx.try_send(frame(unknown_everywhere, &[])).unwrap();
    login_tx.try_send(frame(unknown_everywhere, &[])).unwrap();

    run_inbound(&mut world);
    assert_eq!(game_counts(&world).unknown(), 1);
    let login = |world: &World| {
        world
            .resource::<PreGameDecodeCounts>()
            .counts(ConnectionState::Login)
            .unwrap()
            .unknown()
    };
    assert_eq!(login(&world), 0);

    let mut sys = IntoSystem::into_system(run_event_loop);
    sys.initialize(&mut world);
    let _ = sys.run((), &mut world);
    sys.apply_deferred(&mut world);
    assert_eq!(login(&world), 1);
    assert_eq!(game_counts(&world).unknown(), 1);
    let configuration = world
        .resource::<PreGameDecodeCounts>()
        .counts(ConnectionState::Configuration)
        .unwrap()
        .unknown();
    assert_eq!(configuration, 0);
}

#[test]
fn a_valid_frame_after_an_unknown_id_is_still_delivered() {
    let mut world = build_inbound_world();
    world.init_resource::<DeliveredIds>();
    world.add_observer(
        |event: On<ReceivedPacketEvent>, mut delivered: bevy_ecs::system::ResMut<DeliveredIds>| {
            delivered.0.push(event.id);
        },
    );
    let (socket, tx) = spawn_ingame_connection(&mut world);
    tx.try_send(frame(game_serverbound::NAMES.len() as i32, &[]))
        .unwrap();
    tx.try_send(frame(keep_alive_row(), &7i64.to_be_bytes()))
        .unwrap();

    run_inbound(&mut world);

    assert_eq!(
        world.resource::<DeliveredIds>().0.last(),
        Some(&keep_alive_row())
    );
    assert!(world.entity(socket).contains::<ServerSideConnection>());
    assert_eq!(game_counts(&world).unknown(), 1);
}
