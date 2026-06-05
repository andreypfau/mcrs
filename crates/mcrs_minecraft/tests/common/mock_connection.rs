//! Minimal ECS world helpers for bridge routing tests.
//!
//! These tests exercise `bridge_outbound` (packet routing only, no sockets).
//! The world carries `OutboundQueue` + `PlayerIndex` but no real network
//! transport — socket I/O belongs to separate dispatch tests.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use mcrs_minecraft::world::bridge_queue::OutboundQueue;
use mcrs_minecraft::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, TestPayload};
use mcrs_minecraft::world::player_index::{PlayerIndex, PlayerLocation};
use smallvec::SmallVec;

/// Build a bare world with the resources needed for `bridge_outbound` tests.
pub fn build_bridge_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<PlayerIndex>();
    world
}

/// Spawn a connection entity with an `OutboundQueue` and return its entity id.
pub fn spawn_connection(world: &mut World) -> Entity {
    world.spawn(OutboundQueue::default()).id()
}

/// Register a player in `PlayerIndex` pointing at `socket_entity`.
pub fn register_player(
    world: &mut World,
    player: Entity,
    socket: Entity,
    dim: Entity,
) {
    world.resource_mut::<PlayerIndex>().insert(
        player,
        PlayerLocation {
            socket,
            current_dim: dim,
            previous_dim: None,
            in_dim_entity: Some(socket),
            inbound_pending: SmallVec::new(),
        },
    );
}

/// Write a test packet addressed to `target` into the world's
/// `Messages<OutboundPlayerPacket>`.
pub fn write_packet(world: &mut World, target: PacketTarget, priority: PacketPriority, seq: u32) {
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target,
            priority,
            data: PacketPayload::Test(TestPayload { seq }),
        });
}

/// Run a single system on `world` (handles initialization and deferred commands).
pub fn run_system<S, Marker>(world: &mut World, system: S)
where
    S: IntoSystem<(), (), Marker>,
{
    let mut sys = IntoSystem::into_system(system);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

/// Collect all packets from a connection entity's `OutboundQueue` in priority
/// drain order (Critical → High → Normal → Low) into a flat `Vec`.
pub fn drain_queue(world: &mut World, socket: Entity) -> Vec<OutboundPlayerPacket> {
    let mut q = world.get_mut::<OutboundQueue>(socket).expect("OutboundQueue present");
    let mut out = Vec::new();
    out.extend(q.critical.drain(..));
    out.extend(q.high.drain(..));
    out.extend(q.normal.drain(..));
    out.extend(q.low.drain(..));
    out
}
