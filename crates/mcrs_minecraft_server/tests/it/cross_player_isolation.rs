use crate::mock_connection;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageRegistry;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::bus::{InboundPlayerDespawn, OutboundPlayerPacket};
use mcrs_minecraft_engine::despawn_inbound_player::despawn_inbound_player;
use mcrs_minecraft_engine::muxer::bridge::bridge_outbound;
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{DimPlayerIndex, Owner, PlayerSession, Session};

use mock_connection::{
    drain_queue, register_session, run_system, spawn_connection, write_packet_stamped,
};

#[test]
fn cross_player_isolation() {
    let mut world = World::new();
    MessageRegistry::register_message::<OutboundPlayerPacket>(&mut world);
    world.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
    world.init_resource::<mcrs_minecraft_engine::muxer::bridge::PendingConnections>();

    let session_a = PlayerSession(1);
    let socket_a = spawn_connection(&mut world);
    register_session(&mut world, session_a, socket_a, 0);

    let anchor_a = world
        .query::<(Entity, &Session)>()
        .iter(&world)
        .find(|(_, session)| session.0 == session_a)
        .map(|(anchor, _)| anchor)
        .expect("session_a registered");
    world.despawn(anchor_a);

    let raw_index = socket_a.index();
    world.despawn(socket_a);

    let session_b = PlayerSession(2);
    let socket_b = spawn_connection(&mut world);
    register_session(&mut world, session_b, socket_b, 0);

    write_packet_stamped(&mut world, session_a, 0, 1);
    run_system(&mut world, bridge_outbound);

    let queue_b = world
        .get::<OutboundQueue>(socket_b)
        .expect("OutboundQueue on socket_b");
    assert_eq!(
        queue_b.total_len(),
        0,
        "packet stamped with removed session_a must not reach session_b's queue"
    );

    write_packet_stamped(&mut world, session_b, 0, 2);
    run_system(&mut world, bridge_outbound);

    let packets_b = drain_queue(&mut world, socket_b);
    assert_eq!(packets_b.len(), 1, "session_b must receive its own packets");

    let _ = raw_index;
}

#[test]
fn stale_despawn_does_not_kill_fresh_player() {
    let mut world = World::new();
    MessageRegistry::register_message::<InboundPlayerDespawn>(&mut world);
    world.init_resource::<DimPlayerIndex>();

    let entity_b = world.spawn((Player, Owner(PlayerSession(2)))).id();

    world
        .resource_mut::<Messages<InboundPlayerDespawn>>()
        .write(InboundPlayerDespawn {
            session: PlayerSession(1),
        });

    run_system(&mut world, despawn_inbound_player);

    assert!(
        world.get_entity(entity_b).is_ok(),
        "entity_b must survive a stale despawn for a different session"
    );

    let index = world.resource::<DimPlayerIndex>();
    assert!(
        index.0.contains_key(&PlayerSession(2)),
        "DimPlayerIndex must still contain session_b after stale despawn"
    );
}
