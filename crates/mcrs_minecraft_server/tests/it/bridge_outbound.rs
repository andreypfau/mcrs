//! Integration tests for `bridge_outbound`: packet drain, `PacketTarget`
//! resolution against the sessions, and priority sub-deque ordering.
//!
//! All tests are pure ECS routing — no sockets, no network I/O.

use crate::mock_connection;

use bevy_ecs::entity::Entity;
use mcrs_minecraft_server::world::bridge::bridge_outbound;
use mcrs_minecraft_server::world::bridge_queue::OutboundQueue;
use mcrs_minecraft_server::world::bus::{PacketPriority, PacketTarget};
use smallvec::SmallVec;

use mcrs_minecraft_level::session::PlayerSession;
use mock_connection::{
    build_bridge_world, build_bridge_world_with_sessions, drain_queue, register_player,
    register_session, run_system, spawn_connection, write_packet, write_packet_broadcast,
};

/// Two players in one dimension and one in another; each target reaches
/// exactly the sockets it names, and a listed entity without a session is
/// skipped without a panic.
#[test]
fn a_packet_target_reaches_exactly_its_players() {
    #[derive(Debug)]
    enum Target {
        First,
        FirstDim,
        All,
        FirstLastAndAbsent,
    }

    for (target, expected) in [
        (Target::First, [1, 0, 0]),
        (Target::FirstDim, [1, 1, 0]),
        (Target::All, [1, 1, 1]),
        (Target::FirstLastAndAbsent, [1, 0, 1]),
    ] {
        let mut world = build_bridge_world();
        let dim_a = Entity::from_raw_u32(100).expect("nonzero");
        let dim_b = Entity::from_raw_u32(101).expect("nonzero");
        let sockets: Vec<Entity> = (0..3).map(|_| spawn_connection(&mut world)).collect();
        let players: Vec<(Entity, PlayerSession)> = sockets
            .iter()
            .zip([dim_a, dim_a, dim_b])
            .map(|(&socket, dim)| register_player(&mut world, socket, dim))
            .collect();

        let (packet_target, session) = match target {
            Target::First => (PacketTarget::SinglePlayer(players[0].0), players[0].1),
            Target::FirstDim => (PacketTarget::AllInDim(dim_a), PlayerSession(0)),
            Target::All => (PacketTarget::AllPlayers, PlayerSession(0)),
            Target::FirstLastAndAbsent => {
                let absent = Entity::from_raw_u32(999).expect("nonzero");
                let set: SmallVec<[Entity; 8]> =
                    SmallVec::from_slice(&[players[0].0, players[2].0, absent]);
                (PacketTarget::PlayerSet(set), PlayerSession(0))
            }
        };
        write_packet(
            &mut world,
            packet_target,
            session,
            0,
            PacketPriority::Normal,
            1,
        );
        run_system(&mut world, bridge_outbound);

        let delivered: Vec<usize> = sockets
            .iter()
            .map(|&socket| world.get::<OutboundQueue>(socket).unwrap().total_len())
            .collect();
        assert_eq!(delivered, expected, "{target:?}");
    }
}

// ---------------------------------------------------------------------------
// packet_target_missing_queue_counted
// ---------------------------------------------------------------------------

/// A target that resolves to an entity with no `OutboundQueue` increments
/// `outbound_no_queue_total` and is NOT silently dropped.
#[test]
fn packet_target_missing_queue_counted() {
    let mut world = build_bridge_world();

    let dim = Entity::from_raw_u32(400).expect("nonzero");

    // Spawn a socket entity WITHOUT an OutboundQueue.
    let socket_no_queue = world.spawn_empty().id();
    let (player, session) = register_player(&mut world, socket_no_queue, dim);

    let before = world
        .resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>()
        .outbound_no_queue_total;

    write_packet(
        &mut world,
        PacketTarget::SinglePlayer(player),
        session,
        0,
        PacketPriority::Normal,
        0,
    );

    run_system(&mut world, bridge_outbound);

    let after = world
        .resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>()
        .outbound_no_queue_total;

    assert_eq!(
        after - before,
        1,
        "missing OutboundQueue should increment outbound_no_queue_total"
    );
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
    let socket = spawn_connection(&mut world);
    let (player, session) = register_player(&mut world, socket, dim);

    // Write in reverse-priority order.
    write_packet(
        &mut world,
        PacketTarget::SinglePlayer(player),
        session,
        0,
        PacketPriority::Low,
        4,
    );
    write_packet(
        &mut world,
        PacketTarget::SinglePlayer(player),
        session,
        0,
        PacketPriority::Normal,
        3,
    );
    write_packet(
        &mut world,
        PacketTarget::SinglePlayer(player),
        session,
        0,
        PacketPriority::High,
        2,
    );
    write_packet(
        &mut world,
        PacketTarget::SinglePlayer(player),
        session,
        0,
        PacketPriority::Critical,
        1,
    );

    run_system(&mut world, bridge_outbound);

    let packets = drain_queue(&mut world, socket);
    assert_eq!(packets.len(), 4);

    let seqs: Vec<u32> = packets
        .iter()
        .map(|p| match &p.data {
            mcrs_minecraft_server::world::bus::PacketPayload::Test(t) => t.seq,
            _ => panic!("expected Test payload"),
        })
        .collect();

    assert_eq!(
        seqs,
        vec![1, 2, 3, 4],
        "drain order must be Critical(1) → High(2) → Normal(3) → Low(4)"
    );
}

// ---------------------------------------------------------------------------
// broadcast_delivered_to_post_transfer_session
// ---------------------------------------------------------------------------

/// Regression: broadcasts carry the default epoch 0 and are never re-stamped,
/// so they must NOT be epoch-filtered. A recipient whose session epoch has
/// advanced past 0 (after one or more dim transfers) must still receive
/// AllInDim and AllPlayers broadcasts. The epoch stale-drop applies only to
/// SinglePlayer packets that may be in flight across a transfer.
#[test]
fn broadcast_delivered_to_post_transfer_session() {
    let mut world = build_bridge_world_with_sessions();

    // register_session places the session in dim Entity::from_raw_u32(9998).
    let dim = Entity::from_raw_u32(9998).expect("nonzero");
    let session = PlayerSession(1);
    let socket = spawn_connection(&mut world);
    register_session(&mut world, session, socket, 2); // epoch 2 = two dim transfers

    // Both broadcasts carry the default epoch 0. Write both, then run
    // bridge_outbound once (a single MessageReader pass) so each message is
    // read exactly once. Under the old per-recipient epoch filter both would be
    // dropped (0 != 2); both must now be delivered.
    write_packet_broadcast(
        &mut world,
        PacketTarget::AllInDim(dim),
        PacketPriority::Normal,
        1,
    );
    write_packet_broadcast(
        &mut world,
        PacketTarget::AllPlayers,
        PacketPriority::High,
        2,
    );
    run_system(&mut world, bridge_outbound);
    assert_eq!(
        world.get::<OutboundQueue>(socket).unwrap().total_len(),
        2,
        "AllInDim + AllPlayers broadcasts must both reach a session at epoch 2 (broadcasts are not epoch-filtered)"
    );
}
