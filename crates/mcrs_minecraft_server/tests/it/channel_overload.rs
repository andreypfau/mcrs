use crate::mock_connection;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bytes::Bytes;
use mcrs_minecraft_engine::bridge::{bridge_inbound_to_channel, forward_pending_inbound};
use mcrs_minecraft_engine::bus::InboundPlayerPacket;
use mcrs_minecraft_engine::dim_inputs::DimInputs;
use mcrs_minecraft_engine::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, PlayerSessionCounter, SessionAnchors, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::channels::{MAX_HELD_SERVERBOUND, TO_DIM_CAPACITY};
use mcrs_minecraft_network::ServerSideConnection;

fn build_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<InboundPlayerPacket>>();
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<DimInputs>();
    world.init_resource::<SessionAnchors>();
    world
}

fn take_input(world: &mut World, dim: Entity) {
    world.resource_mut::<DimInputs>().insert(dim);
}

/// What the next tick of `dim` takes in, which the tick then empties.
fn tick_takes(world: &mut World, dim: Entity) -> Vec<i32> {
    let mut inputs = world.resource_mut::<DimInputs>();
    let ids = inputs
        .serverbound(dim)
        .iter()
        .map(|packet| packet.id)
        .collect();
    inputs.remove(dim);
    inputs.insert(dim);
    ids
}

fn spawn_connection(world: &mut World) -> Entity {
    let (raw, _outgoing_rx) = mock_connection::make_mock_raw_connection();
    world
        .spawn(ServerSideConnection { raw: Box::new(raw) })
        .id()
}

fn register_session(
    world: &mut World,
    connection_entity: Entity,
    host_anchor: Entity,
    dim: Entity,
    in_dim_entity: Option<Entity>,
) -> PlayerSession {
    let session = world.resource_mut::<PlayerSessionCounter>().next();
    let place = match in_dim_entity {
        Some(_) => Place::InDim(dim),
        None => Place::Joining(dim),
    };
    world.entity_mut(host_anchor).insert(SessionBundle::placed(
        session,
        SessionPlacement::new(place, 0),
    ));
    world
        .entity_mut(connection_entity)
        .insert(HostAnchorRef(host_anchor));
    session
}

fn run_bridge(world: &mut World) {
    let mut sys = IntoSystem::into_system(bridge_inbound_to_channel);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

fn packet(session: PlayerSession, id: i32) -> InboundPlayerPacket {
    InboundPlayerPacket {
        session,
        id,
        data: Bytes::new(),
        trigger: None,
    }
}

fn fill(world: &mut World, dim: Entity, player: PlayerSession) {
    let mut inputs = world.resource_mut::<DimInputs>();
    for i in 0..TO_DIM_CAPACITY {
        inputs
            .try_send_serverbound(dim, packet(player, i as i32))
            .expect("the next tick has room");
    }
}

fn run_forward(world: &mut World) {
    let mut sys = IntoSystem::into_system(forward_pending_inbound);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

/// The input of a tick is shared by every player of the dimension, so finding it full says
/// nothing about the player whose packet came last.
#[test]
fn a_full_serverbound_input_holds_the_packet_until_the_dimension_ticks() {
    let mut world = build_world();
    let dim = world.spawn_empty().id();
    let other = PlayerSession(999);
    take_input(&mut world, dim);
    let conn = spawn_connection(&mut world);
    let anchor = world.spawn_empty().id();
    let in_dim = world.spawn_empty().id();
    let session = register_session(&mut world, conn, anchor, dim, Some(in_dim));
    fill(&mut world, dim, other);

    world
        .resource_mut::<Messages<InboundPlayerPacket>>()
        .write_batch([packet(session, 0xA), packet(session, 0xB)]);
    run_bridge(&mut world);
    run_forward(&mut world);

    assert!(
        world.get::<ServerSideConnection>(conn).is_some(),
        "a full input holds the packet rather than costing its session the connection"
    );
    assert_eq!(tick_takes(&mut world, dim).len(), TO_DIM_CAPACITY);

    run_forward(&mut world);
    assert_eq!(
        tick_takes(&mut world, dim),
        vec![0xA, 0xB],
        "the held packets reach the dimension once it ticks, in order"
    );
}

#[test]
fn only_the_session_over_its_held_bound_is_disconnected() {
    let mut world = build_world();
    let dim = world.spawn_empty().id();
    let other = PlayerSession(999);
    take_input(&mut world, dim);
    let flooding_conn = spawn_connection(&mut world);
    let flooding = world.spawn_empty().id();
    let quiet_conn = spawn_connection(&mut world);
    let quiet = world.spawn_empty().id();
    let in_dim = world.spawn_empty().id();
    let flooding = register_session(&mut world, flooding_conn, flooding, dim, Some(in_dim));
    let quiet = register_session(&mut world, quiet_conn, quiet, dim, Some(in_dim));
    fill(&mut world, dim, other);

    let mut messages = world.resource_mut::<Messages<InboundPlayerPacket>>();
    messages.write_batch((0..=MAX_HELD_SERVERBOUND as i32).map(|id| packet(flooding, id)));
    messages.write(packet(quiet, 0));
    run_bridge(&mut world);

    assert!(
        world.get::<ServerSideConnection>(flooding_conn).is_none(),
        "a session holding more than its bound is disconnected"
    );
    assert!(
        world.get::<ServerSideConnection>(quiet_conn).is_some(),
        "a session within its bound stays, however full the shared input is"
    );
}
