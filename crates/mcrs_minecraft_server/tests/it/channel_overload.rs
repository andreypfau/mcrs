use crate::mock_connection;
use mcrs_minecraft_server::world::bus::{InboundPlayerDespawn, InboundPlayerSpawn};

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bytes::Bytes;
use mcrs_minecraft_level::session::{Place, PlayerSession, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::channels::{
    FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_server::world::bridge::bridge_inbound_to_channel;
use mcrs_minecraft_server::world::bus::InboundPlayerPacket;
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_minecraft_server::world::session::{HostAnchorRef, SessionBundle};

fn build_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<InboundPlayerPacket>>();
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<DimChannelsResource>();
    world
}

fn make_channels(
    world: &mut World,
    dim: Entity,
) -> (
    flume::Receiver<ToDim>,
    flume::Receiver<ToDim>,
    flume::Sender<FromDim>,
) {
    let (srv_tx, srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    world
        .resource_mut::<DimChannelsResource>()
        .insert(dim, srv_tx, ctl_tx, from_rx);
    (srv_rx, ctl_rx, from_tx)
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

#[test]
fn serverbound_full_disconnects_session() {
    let mut world = build_world();

    let dim_a = world.spawn_empty().id();
    let dim_b = world.spawn_empty().id();

    let (srv_rx_a, _ctl_rx_a, _from_tx_a) = make_channels(&mut world, dim_a);
    let (_srv_rx_b, _ctl_rx_b, _from_tx_b) = make_channels(&mut world, dim_b);

    let in_dim_a = world.spawn_empty().id();
    let in_dim_b = world.spawn_empty().id();

    let conn_a = spawn_connection(&mut world);
    let conn_b = spawn_connection(&mut world);

    let anchor_a = world.spawn_empty().id();
    let anchor_b = world.spawn_empty().id();

    register_session(&mut world, conn_a, anchor_a, dim_a, Some(in_dim_a));
    register_session(&mut world, conn_b, anchor_b, dim_b, Some(in_dim_b));

    // Fill dim_a's serverbound channel to capacity.
    {
        let channels = world.resource::<DimChannelsResource>();
        let entry = channels.get(dim_a).expect("dim_a channel present");
        for i in 0..TO_DIM_CAPACITY {
            entry
                .serverbound_sender
                .try_send(ToDim::Serverbound(InboundPlayerPacket {
                    player: anchor_a,
                    id: i as i32,
                    data: Bytes::new(),
                    timestamp: std::time::Instant::now(),
                }))
                .expect("fill channel");
        }
    }

    // Route one more packet for the offending session.
    world
        .resource_mut::<Messages<InboundPlayerPacket>>()
        .write(InboundPlayerPacket {
            player: anchor_a,
            id: 0xBAD,
            data: Bytes::new(),
            timestamp: std::time::Instant::now(),
        });
    run_bridge(&mut world);

    // The offending session's connection must be disconnected.
    assert!(
        world.get::<ServerSideConnection>(conn_a).is_none(),
        "offending session connection_entity must lose ServerSideConnection on channel full"
    );

    // The unrelated session on dim_b must be unaffected.
    assert!(
        world.get::<ServerSideConnection>(conn_b).is_some(),
        "unrelated session on a different dim must remain connected (bounded blast radius)"
    );

    // Verify the channel was indeed full and we did not accidentally drain it.
    let drained: Vec<_> = srv_rx_a.try_iter().collect();
    assert_eq!(
        drained.len(),
        TO_DIM_CAPACITY,
        "channel held exactly TO_DIM_CAPACITY messages"
    );
}

fn transfer_snapshot() -> mcrs_minecraft_server::world::bus::PlayerTransferSnapshot {
    mcrs_minecraft_server::world::bus::PlayerTransferSnapshot {
        uuid: mcrs_minecraft_protocol::uuid::Uuid::nil(),
        username: "test".into(),
        position: bevy_math::DVec3::ZERO,
        rotation: bevy_math::Vec2::ZERO,
        view_distance: 12,
    }
}

#[test]
fn control_full_enqueues_dim_teardown() {
    use mcrs_minecraft_level::world::sub_app::DimDespawnQueue;
    use mcrs_minecraft_server::dim::send_control_or_teardown;

    let mut world = World::new();
    let dim = world.spawn_empty().id();
    let snapshot = transfer_snapshot();

    // A control channel with headroom: the send is delivered and no teardown is
    // scheduled.
    let (ok_tx, ok_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let ok_sender = ok_tx;
    let mut queue = DimDespawnQueue::default();
    send_control_or_teardown(
        &ok_sender,
        dim,
        ToDim::Despawn(InboundPlayerDespawn {
            host_anchor: dim,
            session: PlayerSession(1),
        }),
        &mut queue,
    );
    assert!(
        queue.0.is_empty(),
        "a control channel with room must not schedule teardown"
    );
    assert!(
        matches!(ok_rx.try_recv(), Ok(ToDim::Despawn(..))),
        "the control message must be delivered when the channel has capacity"
    );

    // A saturated control channel (hard overload): the dim is scheduled for
    // teardown instead of silently dropping the lifecycle message. The receiver
    // is kept alive so the send reports Full, not Disconnected.
    let (full_tx, _full_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let full_sender = full_tx;
    for i in 0..TO_DIM_CONTROL_CAPACITY {
        full_sender
            .try_send(ToDim::Spawn(InboundPlayerSpawn {
                host_anchor: dim,
                session: PlayerSession(i as u64 + 1),
                snapshot: snapshot.clone(),
                dimensions: Vec::new().into(),
            }))
            .expect("fill control channel");
    }
    let mut queue = DimDespawnQueue::default();
    send_control_or_teardown(
        &full_sender,
        dim,
        ToDim::Despawn(InboundPlayerDespawn {
            host_anchor: dim,
            session: PlayerSession(1),
        }),
        &mut queue,
    );
    assert_eq!(
        queue.0,
        vec![dim],
        "a saturated control channel must schedule the dim for teardown"
    );

    // A second failed send for the same dim must not double-enqueue.
    send_control_or_teardown(
        &full_sender,
        dim,
        ToDim::Despawn(InboundPlayerDespawn {
            host_anchor: dim,
            session: PlayerSession(1),
        }),
        &mut queue,
    );
    assert_eq!(
        queue.0,
        vec![dim],
        "teardown scheduling must be deduplicated per dim"
    );
}
