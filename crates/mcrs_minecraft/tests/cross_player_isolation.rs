//! ROUT-04 gate: cross-player isolation under disconnect/reconnect id churn.
//!
//! Scenario A: a packet stamped with a disconnected session's id never reaches
//! a newly-connected session that happens to reuse the same socket entity
//! generation. Verifies the `bridge_outbound` registry-miss drop.
//!
//! Scenario B: a stale `InboundPlayerDespawn` carrying `PlayerSession(1)` does
//! not despawn a dim-world entity owned by `PlayerSession(2)`. Verifies that
//! `despawn_inbound_player` keys the entity despawn on `host_anchor`, not the
//! session of the despawn message, so a freshly-connected player cannot be
//! accidentally removed by a slow dim processing an old message.

#[path = "common/mock_connection.rs"]
mod mock_connection;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_engine::entity::player::Player;
use mcrs_engine::session::{DimPlayerIndex, Owner, PlayerSession, SessionRegistry};
use mcrs_minecraft::world::bridge::bridge_outbound;
use mcrs_minecraft::world::bridge_queue::OutboundQueue;
use mcrs_minecraft::world::bus::{InboundPlayerDespawn, OutboundPlayerPacket, PacketPriority};
use mcrs_minecraft::world::entity::player::{despawn_inbound_player, HostAnchor};

use mock_connection::{drain_queue, register_session, run_system, spawn_connection, write_packet_stamped};

// ---------------------------------------------------------------------------
// Scenario A — no cross-session delivery under id churn
// ---------------------------------------------------------------------------

/// Under disconnect/reconnect id churn, a packet stamped with a disconnected
/// session's id must never reach a newly-connected session's queue even when
/// the session reuses the same connection entity slot.
#[test]
fn cross_player_isolation() {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<SessionRegistry>();

    // --- Connect player A ---
    let session_a = PlayerSession(1);
    let socket_a = spawn_connection(&mut world);
    register_session(&mut world, session_a, socket_a, 0);

    // --- Disconnect A: remove the session entry ---
    world.resource_mut::<SessionRegistry>().remove(&session_a);

    // Despawn the socket so Bevy can potentially reuse the slot (simulating
    // the entity generation churn that happens under real disconnect/reconnect).
    let raw_index = socket_a.index();
    world.despawn(socket_a);

    // --- Connect player B: new session, re-spawned socket ---
    // Spawn a new entity. Bevy may or may not reuse the same index with a
    // higher generation, which is exactly the churn risk under test.
    let session_b = PlayerSession(2);
    let socket_b = spawn_connection(&mut world);
    register_session(&mut world, session_b, socket_b, 0);

    // --- Emit a packet stamped with A's removed session ---
    write_packet_stamped(&mut world, session_a, 0, PacketPriority::Normal, 1);
    run_system(&mut world, bridge_outbound);

    // B's queue must be empty: A's session is gone from the registry so
    // bridge_outbound takes the registry-miss path and drops the packet.
    let queue_b = world.get::<OutboundQueue>(socket_b).expect("OutboundQueue on socket_b");
    assert_eq!(
        queue_b.total_len(),
        0,
        "packet stamped with removed session_a must not reach session_b's queue"
    );

    // Sanity: B still receives its own packets.
    write_packet_stamped(&mut world, session_b, 0, PacketPriority::Normal, 2);
    run_system(&mut world, bridge_outbound);

    let packets_b = drain_queue(&mut world, socket_b);
    assert_eq!(packets_b.len(), 1, "session_b must receive its own packets");

    let _ = raw_index; // suppress unused-variable warning
}

// ---------------------------------------------------------------------------
// Scenario B — stale despawn does not kill a freshly-connected player
// ---------------------------------------------------------------------------

/// A slow dim processing a stale `InboundPlayerDespawn { session: PlayerSession(1) }`
/// must not despawn an entity owned by `PlayerSession(2)`. The session-keyed
/// `DimPlayerIndex` remove is safe (session 1 is absent), and the entity
/// query matches on `host_anchor`, which differs between the two players.
#[test]
fn stale_despawn_does_not_kill_fresh_player() {
    let mut world = World::new();
    world.init_resource::<Messages<InboundPlayerDespawn>>();
    world.init_resource::<DimPlayerIndex>();

    // --- Spawn an entity for player B in the dim world ---
    let host_anchor_b = Entity::from_raw_u32(200).expect("nonzero");
    let entity_b = world
        .spawn((
            Player,
            Owner(PlayerSession(2)),
            HostAnchor(host_anchor_b),
        ))
        .id();

    // Register B in DimPlayerIndex.
    world
        .resource_mut::<DimPlayerIndex>()
        .0
        .insert(PlayerSession(2), entity_b);

    // --- Emit a stale despawn for player A (host_anchor different from B) ---
    let host_anchor_a = Entity::from_raw_u32(100).expect("nonzero");
    world
        .resource_mut::<Messages<InboundPlayerDespawn>>()
        .write(InboundPlayerDespawn {
            host_anchor: host_anchor_a,
            session: PlayerSession(1),
        });

    run_system(&mut world, despawn_inbound_player);

    // entity_b must still exist: the despawn message's host_anchor (100) does
    // not match entity_b's HostAnchor (200), so the query skip protects B.
    assert!(
        world.get_entity(entity_b).is_ok(),
        "entity_b must survive a stale despawn for a different host_anchor"
    );

    // DimPlayerIndex must still contain session_b: session_a (1) was absent
    // so the remove was a no-op; session_b (2) is untouched.
    let index = world.resource::<DimPlayerIndex>();
    assert!(
        index.0.contains_key(&PlayerSession(2)),
        "DimPlayerIndex must still contain session_b after stale despawn"
    );
}
