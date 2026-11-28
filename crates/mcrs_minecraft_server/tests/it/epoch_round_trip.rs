use crate::mock_connection;

use bevy_ecs::message::MessageRegistry;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::muxer::bridge::bridge_outbound;
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine_world::session::{PlayerSession, SessionPlacement};

use mock_connection::{
    drain_queue, register_session, run_system, spawn_connection, write_packet_stamped,
};

#[test]
fn epoch_round_trip() {
    let mut world = World::new();
    MessageRegistry::register_message::<OutboundPlayerPacket>(&mut world);
    world.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
    world.init_resource::<mcrs_minecraft_engine::muxer::bridge::PendingConnections>();

    let session = PlayerSession(1);
    let socket = spawn_connection(&mut world);
    register_session(&mut world, session, socket, 0);

    write_packet_stamped(&mut world, session, 0, 1);

    let mut placements = world.query::<&mut SessionPlacement>();
    let mut placement = placements
        .single_mut(&mut world)
        .expect("session must be registered");
    *placement = SessionPlacement::new(placement.place(), 1);

    run_system(&mut world, bridge_outbound);

    let queue = world
        .get::<OutboundQueue>(socket)
        .expect("OutboundQueue present");
    assert_eq!(
        queue.total_len(),
        0,
        "epoch-0 packet must be dropped after session epoch bumped to 1 (A→B→A stale-drop)"
    );

    write_packet_stamped(&mut world, session, 1, 2);
    run_system(&mut world, bridge_outbound);

    let packets = drain_queue(&mut world, socket);
    assert_eq!(
        packets.len(),
        1,
        "epoch-1 packet must be delivered (current epoch matches session epoch)"
    );
}
