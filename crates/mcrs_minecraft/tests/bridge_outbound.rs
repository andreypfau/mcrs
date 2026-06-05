//! Integration tests for `bridge_outbound`: packet drain, `PacketTarget`
//! resolution against `PlayerIndex`, and priority sub-deque ordering.
//!
//! All tests are pure ECS routing — no sockets, no network I/O.

#[path = "common/mock_connection.rs"]
mod mock_connection;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use mcrs_minecraft::world::bridge::bridge_outbound;
use mcrs_minecraft::world::bridge_queue::OutboundQueue;
use mcrs_minecraft::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use smallvec::SmallVec;

use mock_connection::{
    build_bridge_world, drain_queue, register_player, run_system, spawn_connection, write_packet,
};

// ---------------------------------------------------------------------------
// bridge_outbound_drains
// ---------------------------------------------------------------------------

/// A message written to `Messages<OutboundPlayerPacket>` on the main world is
/// drained by `bridge_outbound` and pushed to the resolved player's
/// `OutboundQueue`. The system uses `MessageReader` (cursor semantics) rather
/// than `Messages::drain()`, which is the contract for single-owner reads.
#[test]
fn bridge_outbound_drains() {
    let mut world = build_bridge_world();

    let player = Entity::from_raw_u32(1).expect("nonzero");
    let dim = Entity::from_raw_u32(2).expect("nonzero");
    let socket = spawn_connection(&mut world);
    register_player(&mut world, player, socket, dim);

    write_packet(&mut world, PacketTarget::SinglePlayer(player), PacketPriority::Normal, 42);

    run_system(&mut world, bridge_outbound);

    // The queue on `socket` should have received the packet.
    let queue = world.get::<OutboundQueue>(socket).expect("OutboundQueue present");
    assert_eq!(queue.total_len(), 1, "packet was not pushed to OutboundQueue");

    // No other side-effects: exactly one message produced exactly one push.
    assert_eq!(queue.normal.len(), 1, "Normal-priority packet must land in normal sub-deque");
    assert_eq!(queue.critical.len(), 0);
    assert_eq!(queue.high.len(), 0);
    assert_eq!(queue.low.len(), 0);
}

// ---------------------------------------------------------------------------
// packet_target_single_player
// ---------------------------------------------------------------------------

/// `SinglePlayer(e)` resolves `e` → `player_index.get(&e).socket` →
/// pushes to exactly that socket's `OutboundQueue`, no other.
#[test]
fn packet_target_single_player() {
    let mut world = build_bridge_world();

    let player_a = Entity::from_raw_u32(10).expect("nonzero");
    let player_b = Entity::from_raw_u32(11).expect("nonzero");
    let dim = Entity::from_raw_u32(2).expect("nonzero");
    let socket_a = spawn_connection(&mut world);
    let socket_b = spawn_connection(&mut world);
    register_player(&mut world, player_a, socket_a, dim);
    register_player(&mut world, player_b, socket_b, dim);

    write_packet(
        &mut world,
        PacketTarget::SinglePlayer(player_a),
        PacketPriority::Normal,
        1,
    );

    run_system(&mut world, bridge_outbound);

    let qa = world.get::<OutboundQueue>(socket_a).unwrap();
    let qb = world.get::<OutboundQueue>(socket_b).unwrap();
    assert_eq!(qa.total_len(), 1, "packet_a not in socket_a queue");
    assert_eq!(qb.total_len(), 0, "packet_a leaked to socket_b");
}

// ---------------------------------------------------------------------------
// packet_target_all_in_dim
// ---------------------------------------------------------------------------

/// `AllInDim(dim)` pushes to every player whose `current_dim == dim` and to
/// no players in other dimensions.
#[test]
fn packet_target_all_in_dim() {
    let mut world = build_bridge_world();

    let dim_a = Entity::from_raw_u32(100).expect("nonzero");
    let dim_b = Entity::from_raw_u32(101).expect("nonzero");

    let player_a1 = Entity::from_raw_u32(20).expect("nonzero");
    let player_a2 = Entity::from_raw_u32(21).expect("nonzero");
    let player_b = Entity::from_raw_u32(22).expect("nonzero");

    let socket_a1 = spawn_connection(&mut world);
    let socket_a2 = spawn_connection(&mut world);
    let socket_b = spawn_connection(&mut world);

    register_player(&mut world, player_a1, socket_a1, dim_a);
    register_player(&mut world, player_a2, socket_a2, dim_a);
    register_player(&mut world, player_b, socket_b, dim_b);

    write_packet(&mut world, PacketTarget::AllInDim(dim_a), PacketPriority::Normal, 5);

    run_system(&mut world, bridge_outbound);

    assert_eq!(world.get::<OutboundQueue>(socket_a1).unwrap().total_len(), 1);
    assert_eq!(world.get::<OutboundQueue>(socket_a2).unwrap().total_len(), 1);
    assert_eq!(world.get::<OutboundQueue>(socket_b).unwrap().total_len(), 0);
}

// ---------------------------------------------------------------------------
// packet_target_all_players
// ---------------------------------------------------------------------------

/// `AllPlayers` pushes to every player in `PlayerIndex`, regardless of dim.
#[test]
fn packet_target_all_players() {
    let mut world = build_bridge_world();

    let dim = Entity::from_raw_u32(200).expect("nonzero");

    let player_x = Entity::from_raw_u32(30).expect("nonzero");
    let player_y = Entity::from_raw_u32(31).expect("nonzero");
    let player_z = Entity::from_raw_u32(32).expect("nonzero");

    let socket_x = spawn_connection(&mut world);
    let socket_y = spawn_connection(&mut world);
    let socket_z = spawn_connection(&mut world);

    register_player(&mut world, player_x, socket_x, dim);
    register_player(&mut world, player_y, socket_y, dim);
    register_player(&mut world, player_z, socket_z, dim);

    write_packet(&mut world, PacketTarget::AllPlayers, PacketPriority::High, 7);

    run_system(&mut world, bridge_outbound);

    assert_eq!(world.get::<OutboundQueue>(socket_x).unwrap().total_len(), 1);
    assert_eq!(world.get::<OutboundQueue>(socket_y).unwrap().total_len(), 1);
    assert_eq!(world.get::<OutboundQueue>(socket_z).unwrap().total_len(), 1);
}

// ---------------------------------------------------------------------------
// packet_target_player_set
// ---------------------------------------------------------------------------

/// `PlayerSet` pushes to exactly the listed entities present in `PlayerIndex`;
/// entities absent from the index are skipped without panic.
#[test]
fn packet_target_player_set() {
    let mut world = build_bridge_world();

    let dim = Entity::from_raw_u32(300).expect("nonzero");

    let player_p = Entity::from_raw_u32(40).expect("nonzero");
    let player_q = Entity::from_raw_u32(41).expect("nonzero");
    let absent = Entity::from_raw_u32(999).expect("nonzero");

    let socket_p = spawn_connection(&mut world);
    let socket_q = spawn_connection(&mut world);

    register_player(&mut world, player_p, socket_p, dim);
    register_player(&mut world, player_q, socket_q, dim);
    // `absent` is NOT in PlayerIndex

    let mut set: SmallVec<[Entity; 8]> = SmallVec::new();
    set.push(player_p);
    set.push(player_q);
    set.push(absent);

    write_packet(&mut world, PacketTarget::PlayerSet(set), PacketPriority::Normal, 9);

    // Must not panic even though `absent` is not in PlayerIndex.
    run_system(&mut world, bridge_outbound);

    assert_eq!(world.get::<OutboundQueue>(socket_p).unwrap().total_len(), 1);
    assert_eq!(world.get::<OutboundQueue>(socket_q).unwrap().total_len(), 1);
}

// ---------------------------------------------------------------------------
// packet_target_missing_queue_counted
// ---------------------------------------------------------------------------

/// A target that resolves to an entity with no `OutboundQueue` increments
/// `BRIDGE_OUTBOUND_NO_QUEUE_TOTAL` and is NOT silently dropped.
#[test]
fn packet_target_missing_queue_counted() {
    let _lock = mcrs_network::metrics::TELEMETRY_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let mut world = build_bridge_world();

    let dim = Entity::from_raw_u32(400).expect("nonzero");
    let player = Entity::from_raw_u32(50).expect("nonzero");

    // Spawn a socket entity WITHOUT an OutboundQueue.
    let socket_no_queue = world.spawn_empty().id();
    register_player(&mut world, player, socket_no_queue, dim);

    let before = mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL
        .load(std::sync::atomic::Ordering::Relaxed);

    write_packet(
        &mut world,
        PacketTarget::SinglePlayer(player),
        PacketPriority::Normal,
        0,
    );

    run_system(&mut world, bridge_outbound);

    let after = mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL
        .load(std::sync::atomic::Ordering::Relaxed);

    assert_eq!(after - before, 1, "missing OutboundQueue should increment BRIDGE_OUTBOUND_NO_QUEUE_TOTAL");

    // Reset counter so parallel tests don't see stale increments.
    mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL
        .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// priority_drain_order
// ---------------------------------------------------------------------------

/// Pushing Low → Normal → High → Critical then draining in priority order
/// yields Critical, High, Normal, Low.
#[test]
fn priority_drain_order() {
    let mut world = build_bridge_world();

    let dim = Entity::from_raw_u32(500).expect("nonzero");
    let player = Entity::from_raw_u32(60).expect("nonzero");
    let socket = spawn_connection(&mut world);
    register_player(&mut world, player, socket, dim);

    // Write in reverse-priority order.
    write_packet(&mut world, PacketTarget::SinglePlayer(player), PacketPriority::Low, 4);
    write_packet(&mut world, PacketTarget::SinglePlayer(player), PacketPriority::Normal, 3);
    write_packet(&mut world, PacketTarget::SinglePlayer(player), PacketPriority::High, 2);
    write_packet(&mut world, PacketTarget::SinglePlayer(player), PacketPriority::Critical, 1);

    run_system(&mut world, bridge_outbound);

    let packets = drain_queue(&mut world, socket);
    assert_eq!(packets.len(), 4);

    let seqs: Vec<u32> = packets
        .iter()
        .map(|p| match &p.data {
            mcrs_minecraft::world::bus::PacketPayload::Test(t) => t.seq,
            _ => panic!("expected Test payload"),
        })
        .collect();

    assert_eq!(seqs, vec![1, 2, 3, 4], "drain order must be Critical(1) → High(2) → Normal(3) → Low(4)");
}
