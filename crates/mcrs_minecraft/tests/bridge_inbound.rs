//! Integration tests for `bridge_inbound` (per-connection rate limiting,
//! ReceivedPacketEvent emission) and `disconnect_clears_pending` (teardown
//! leak check).

#[path = "common/mock_connection.rs"]
mod mock_connection;

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::Commands;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{IntoSystem, RunSystemOnce, System};
use bevy_ecs::world::World;
use mcrs_minecraft::world::bridge::bridge_inbound;
use mcrs_minecraft::world::bridge_queue::{
    InboundRateBucket, OutboundQueue, INBOUND_BUCKET_CAP, INBOUND_KICK_OVERFLOW_TICKS,
};
use bytes::Bytes;
use mcrs_minecraft::world::bus::{
    InboundPlayerPacket, OutboundPlayerPacket, PendingInboundPartition,
};
use mcrs_minecraft::world::player_index::{HostAnchorRef, PlayerIndex, PlayerLocation};
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::metrics::{BRIDGE_KICK_FLOOD_TOTAL, TELEMETRY_TEST_LOCK};
use mcrs_network::{InGameConnectionState, ReceivedPacket, ServerSideConnection};
use smallvec::SmallVec;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_inbound_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<Messages<InboundPlayerPacket>>();
    world.init_resource::<PlayerIndex>();
    world.init_resource::<PendingInboundPartition>();
    world
}

/// Spawn a connection entity with InGameConnectionState + InboundRateBucket +
/// OutboundQueue and a mock RawConnection that we can inject packets into.
///
/// Returns `(socket_entity, inbound_tx)`. Send `ReceivedPacket` values into
/// `inbound_tx` to simulate packets arriving from the client.
fn spawn_ingame_connection(
    world: &mut World,
) -> (Entity, mpsc::Sender<ReceivedPacket>) {
    let (raw, _outgoing_rx, inbound_tx) = mock_connection::make_mock_raw_connection_full();
    let entity = world
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            InGameConnectionState,
            InboundRateBucket::new(),
            OutboundQueue::default(),
        ))
        .id();
    (entity, inbound_tx)
}

/// Register a player in PlayerIndex pointing at `socket` with the given dim
/// and `in_dim_entity`.
fn register_player(
    world: &mut World,
    player: Entity,
    socket: Entity,
    dim: Entity,
    in_dim_entity: Option<Entity>,
) {
    world.resource_mut::<PlayerIndex>().insert(
        player,
        PlayerLocation {
            socket,
            current_dim: dim,
            previous_dim: None,
            in_dim_entity,
            inbound_pending: SmallVec::new(),
        },
    );
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

#[derive(Component, Default)]
struct EntityEventCount(usize);

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
    world.add_observer(|_ev: On<ReceivedPacketEvent>, mut counter: bevy_ecs::system::ResMut<EventCounter>| {
        counter.count += 1;
    });

    let dim_a = Entity::from_raw_u32(10).expect("nonzero");
    let dim_b = Entity::from_raw_u32(11).expect("nonzero");
    let player_a = Entity::from_raw_u32(20).expect("nonzero");
    let player_b = Entity::from_raw_u32(21).expect("nonzero");
    let in_dim_a = Entity::from_raw_u32(30).expect("nonzero");
    let in_dim_b = Entity::from_raw_u32(31).expect("nonzero");

    let (socket_a, tx_a) = spawn_ingame_connection(&mut world);
    let (socket_b, tx_b) = spawn_ingame_connection(&mut world);

    register_player(&mut world, player_a, socket_a, dim_a, Some(in_dim_a));
    register_player(&mut world, player_b, socket_b, dim_b, Some(in_dim_b));

    // Inject one packet into each connection.
    tx_a.try_send(make_received_packet(1)).unwrap();
    tx_b.try_send(make_received_packet(2)).unwrap();

    run_inbound(&mut world);

    let counter = world.resource::<EventCounter>();
    assert_eq!(
        counter.count, 2,
        "bridge_inbound must emit one ReceivedPacketEvent per drained packet"
    );

    // PendingInboundPartition must remain empty — bridge_inbound no longer
    // routes there directly.
    let partition = world.resource::<PendingInboundPartition>();
    assert!(
        partition.per_dim.is_empty(),
        "bridge_inbound must not write into PendingInboundPartition directly"
    );
}

// ---------------------------------------------------------------------------
// bridge_inbound_emits_event_regardless_of_transit_state
//
// Replaces in_transit_buffering: bridge_inbound no longer buffers in
// inbound_pending. It emits ReceivedPacketEvent unconditionally (rate-
// permitting). Buffering for mid-transit players is handled by the consumer
// observer (e.g. keepalive / movement). This test verifies the event fires
// even when in_dim_entity is None.
// ---------------------------------------------------------------------------

#[test]
fn bridge_inbound_emits_event_regardless_of_transit_state() {
    let mut world = build_inbound_world();
    world.init_resource::<EventCounter>();

    world.add_observer(|_ev: On<ReceivedPacketEvent>, mut counter: bevy_ecs::system::ResMut<EventCounter>| {
        counter.count += 1;
    });

    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let player = Entity::from_raw_u32(20).expect("nonzero");

    let (socket, tx) = spawn_ingame_connection(&mut world);
    // in_dim_entity = None → player is mid-transit
    register_player(&mut world, player, socket, dim, None);

    tx.try_send(make_received_packet(42)).unwrap();

    run_inbound(&mut world);

    let counter = world.resource::<EventCounter>();
    assert_eq!(
        counter.count, 1,
        "bridge_inbound must emit ReceivedPacketEvent even when player is mid-transit"
    );

    // PendingInboundPartition must remain empty.
    let partition = world.resource::<PendingInboundPartition>();
    assert!(
        partition.per_dim.is_empty(),
        "bridge_inbound must not write into PendingInboundPartition"
    );
}

// ---------------------------------------------------------------------------
// inbound_rate_kick
// ---------------------------------------------------------------------------

/// Sustained packet flood exceeding INBOUND_BUCKET_CAP for
/// INBOUND_KICK_OVERFLOW_TICKS kicks the connection (ServerSideConnection
/// removed) and increments BRIDGE_KICK_FLOOD_TOTAL.
/// Packets received within the budget are NOT dropped.
#[test]
fn inbound_rate_kick() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_inbound_world();

    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let player = Entity::from_raw_u32(20).expect("nonzero");
    let in_dim = Entity::from_raw_u32(30).expect("nonzero");

    let (socket, tx) = spawn_ingame_connection(&mut world);
    register_player(&mut world, player, socket, dim, Some(in_dim));
    attach_anchor(&mut world, socket, player);

    let before = BRIDGE_KICK_FLOOD_TOTAL.load(Ordering::Relaxed);

    // Run enough ticks flooding packets to trigger the kick.
    // Each tick sends INBOUND_BUCKET_CAP + 1 packets to ensure bucket empties.
    for _ in 0..INBOUND_KICK_OVERFLOW_TICKS + 1 {
        if world.get::<ServerSideConnection>(socket).is_none() {
            break;
        }
        for seq in 0..INBOUND_BUCKET_CAP + 10 {
            let _ = tx.try_send(make_received_packet(seq as u32));
        }
        run_inbound(&mut world);
    }

    let after = BRIDGE_KICK_FLOOD_TOTAL.load(Ordering::Relaxed);
    assert!(
        after > before,
        "BRIDGE_KICK_FLOOD_TOTAL must increment on flood kick"
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
    use mcrs_minecraft::world::bridge::attach_outbound_queue;

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
    assert_eq!(gap_before, 1, "should have one unattached connection before attach");

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

/// After `process_disconnect` runs, the player's `PlayerIndex` entry is
/// removed (which clears `inbound_pending`) and `OutboundQueue` is removed
/// from the socket entity. Neither leaks past the disconnect tick.
#[test]
fn disconnect_clears_pending() {
    use mcrs_minecraft::disconnect::process_disconnect;
    use mcrs_minecraft::world::bus::PendingInboundLifecycle;

    let mut world = World::new();
    world.init_resource::<PlayerIndex>();
    world.init_resource::<PendingInboundLifecycle>();

    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let player = Entity::from_raw_u32(20).expect("nonzero");
    let in_dim = Entity::from_raw_u32(30).expect("nonzero");

    let (socket, _tx) = spawn_ingame_connection(&mut world);
    // Populate inbound_pending with a few packets.
    {
        let mut index = world.resource_mut::<PlayerIndex>();
        index.insert(
            player,
            PlayerLocation {
                socket,
                current_dim: dim,
                previous_dim: None,
                in_dim_entity: Some(in_dim),
                inbound_pending: {
                    let mut v = smallvec::SmallVec::new();
                    for seq in 0..3i32 {
                        v.push(InboundPlayerPacket {
                            player,
                            id: seq,
                            data: Bytes::new(),
                            timestamp: std::time::Instant::now(),
                        });
                    }
                    v
                },
            },
        );
    }

    // Verify inbound_pending was populated.
    {
        let index = world.resource::<PlayerIndex>();
        let loc = index.get(&player).expect("player present before disconnect");
        assert_eq!(loc.inbound_pending.len(), 3, "inbound_pending should have 3 packets before disconnect");
    }

    // Verify OutboundQueue exists on socket.
    assert!(
        world.get::<OutboundQueue>(socket).is_some(),
        "OutboundQueue should exist before disconnect"
    );

    // Run process_disconnect (simulating the observer path).
    world.run_system_once(move |mut commands: bevy_ecs::prelude::Commands,
                                mut player_index: bevy_ecs::system::ResMut<PlayerIndex>,
                                mut lifecycle: bevy_ecs::system::ResMut<PendingInboundLifecycle>| {
        process_disconnect(player, &mut player_index, &mut lifecycle, &mut commands);
    }).expect("process_disconnect system ran");

    // inbound_pending: PlayerIndex entry is gone → effectively cleared.
    let index = world.resource::<PlayerIndex>();
    assert!(
        index.get(&player).is_none(),
        "PlayerIndex entry must be removed after disconnect (inbound_pending cleared with it)"
    );

    // OutboundQueue must be removed from the socket entity.
    assert!(
        world.get::<OutboundQueue>(socket).is_none(),
        "OutboundQueue must be removed from socket entity after disconnect"
    );
}
