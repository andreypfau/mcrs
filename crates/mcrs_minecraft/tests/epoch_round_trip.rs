//! ROUT-05 gate: A→B→A round-trip stale-epoch drop and current-epoch delivery.
//!
//! Simulates the hazard where a clientbound packet from a player's first visit
//! to dimension A is still in transit when the player returns to A after
//! visiting B (i.e., the A→B→A round trip). The session epoch is bumped to 1
//! to represent the Nether (B) entry; the real dim-transfer path will perform
//! this bump — here we mutate `SessionRegistry` directly since the
//! move infrastructure does not exist yet.
//!
//! Assertions:
//!   - epoch-0 packet (from the first A visit) is dropped after bump to epoch 1
//!   - epoch-1 packet (from the return-to-A visit) is delivered

#[path = "common/mock_connection.rs"]
mod mock_connection;

use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_engine::session::{PlayerSession, SessionRegistry};
use mcrs_minecraft::world::bridge::bridge_outbound;
use mcrs_minecraft::world::bridge_queue::OutboundQueue;
use mcrs_minecraft::world::bus::{OutboundPlayerPacket, PacketPriority};

use mock_connection::{drain_queue, register_session, run_system, spawn_connection, write_packet_stamped};

/// Epoch-0 packets are dropped after the session epoch is bumped to 1, and
/// epoch-1 packets are delivered. Models the A→B→A stale-drop invariant.
#[test]
fn epoch_round_trip() {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<SessionRegistry>();

    // --- Connect a player: register session at epoch 0 (first Overworld visit) ---
    let session = PlayerSession(1);
    let socket = spawn_connection(&mut world);
    register_session(&mut world, session, socket, 0);

    // --- Emit an epoch-0 packet (in-flight clientbound from the first A visit) ---
    write_packet_stamped(&mut world, session, 0, PacketPriority::Normal, 1);

    // --- Bump the session epoch to 1 (simulating Nether entry / dim B transition).
    //
    // The real dim-transfer path will perform this bump when the player's
    // dimension transfer completes. We mutate SessionRegistry directly here
    // because the move infrastructure does not exist yet. ---
    world
        .resource_mut::<SessionRegistry>()
        .get_mut(&session)
        .expect("session must be registered")
        .epoch = 1;

    // Run bridge_outbound: the epoch-0 packet is now stale (session.epoch == 1)
    // and must be dropped by the strict-equality filter.
    run_system(&mut world, bridge_outbound);

    let queue = world.get::<OutboundQueue>(socket).expect("OutboundQueue present");
    assert_eq!(
        queue.total_len(),
        0,
        "epoch-0 packet must be dropped after session epoch bumped to 1 (A→B→A stale-drop)"
    );

    // --- Emit an epoch-1 packet (clientbound from the return-to-A visit) ---
    write_packet_stamped(&mut world, session, 1, PacketPriority::Normal, 2);
    run_system(&mut world, bridge_outbound);

    let packets = drain_queue(&mut world, socket);
    assert_eq!(
        packets.len(),
        1,
        "epoch-1 packet must be delivered (current epoch matches session epoch)"
    );
}
