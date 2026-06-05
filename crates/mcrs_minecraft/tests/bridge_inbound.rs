//! Integration tests for `bridge_inbound` (per-connection rate limiting,
//! routing into PendingInboundPartition / inbound_pending) and
//! `disconnect_clears_pending` (teardown leak check).

#[path = "common/mock_connection.rs"]
mod mock_connection;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, RunSystemOnce, System};
use bevy_ecs::world::World;
use mcrs_minecraft::world::bridge::bridge_inbound;
use mcrs_minecraft::world::bridge_queue::{
    InboundRateBucket, OutboundQueue, INBOUND_BUCKET_CAP, INBOUND_KICK_OVERFLOW_TICKS,
};
use mcrs_minecraft::world::bus::{
    InboundPlayerPacket, OutboundPlayerPacket, PendingInboundPartition, TestInboundPayload,
};
use mcrs_minecraft::world::player_index::{HostAnchorRef, PlayerIndex, PlayerLocation};
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
// bridge_inbound_routes_to_dim
// ---------------------------------------------------------------------------

/// A packet received on an in-game connection whose player is assigned to
/// dim D is written into PendingInboundPartition.per_dim[D] and into no other
/// dim's bucket.
#[test]
fn bridge_inbound_routes_to_dim() {
    let mut world = build_inbound_world();

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
    attach_anchor(&mut world, socket_a, player_a);
    attach_anchor(&mut world, socket_b, player_b);

    // Inject one packet into each connection.
    tx_a.try_send(make_received_packet(1)).unwrap();
    tx_b.try_send(make_received_packet(2)).unwrap();

    // Give the mpsc a moment to be readable (same-thread runtime, no delay needed).
    run_inbound(&mut world);

    let partition = world.resource::<PendingInboundPartition>();
    let bucket_a = partition.per_dim.get(&dim_a).map(|v| v.len()).unwrap_or(0);
    let bucket_b = partition.per_dim.get(&dim_b).map(|v| v.len()).unwrap_or(0);

    assert_eq!(bucket_a, 1, "packet for player_a must land in dim_a bucket only");
    assert_eq!(bucket_b, 1, "packet for player_b must land in dim_b bucket only");

    // Ensure dim_a packet is not in dim_b's bucket and vice-versa.
    assert!(
        partition.per_dim.get(&dim_b).map_or(true, |v| {
            v.iter().all(|p| p.player != player_a)
        }),
        "player_a packet must not appear in dim_b bucket"
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
// in_transit_buffering
// ---------------------------------------------------------------------------

/// A packet received for a player whose in_dim_entity is None (mid-transfer)
/// must be appended to PlayerLocation.inbound_pending, not a partition bucket.
#[test]
fn in_transit_buffering() {
    let mut world = build_inbound_world();

    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let player = Entity::from_raw_u32(20).expect("nonzero");

    let (socket, tx) = spawn_ingame_connection(&mut world);
    // in_dim_entity = None → player is mid-transit
    register_player(&mut world, player, socket, dim, None);
    attach_anchor(&mut world, socket, player);

    tx.try_send(make_received_packet(42)).unwrap();

    run_inbound(&mut world);

    let partition = world.resource::<PendingInboundPartition>();
    assert!(
        partition.per_dim.is_empty(),
        "in-transit packet must not land in a partition bucket"
    );

    let index = world.resource::<PlayerIndex>();
    let loc = index.get(&player).expect("player still present");
    assert_eq!(
        loc.inbound_pending.len(),
        1,
        "in-transit packet must be buffered in inbound_pending"
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
                    for seq in 0..3u32 {
                        v.push(InboundPlayerPacket {
                            player,
                            packet: TestInboundPayload { seq },
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
