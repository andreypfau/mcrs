#[path = "common/mock_connection.rs"]
mod mock_connection;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bytes::Bytes;
use mcrs_engine::session::{PlayerSession, PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_engine::world::channels::{DimSender, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY};
use mcrs_minecraft::world::bridge::bridge_inbound_to_channel;
use mcrs_minecraft::world::bus::InboundPlayerPacket;
use mcrs_minecraft::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_minecraft::world::player_index::PendingInboundBuffer;
use mcrs_network::ServerSideConnection;

fn build_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<InboundPlayerPacket>>();
    world.init_resource::<SessionRegistry>();
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<PendingInboundBuffer>();
    world.init_resource::<DimChannelsResource>();
    world
}

fn make_channels(world: &mut World, dim: Entity) -> (flume::Receiver<ToDim>, flume::Receiver<ToDim>, flume::Sender<FromDim>) {
    let (srv_tx, srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    world
        .resource_mut::<DimChannelsResource>()
        .insert(dim, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
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
    world.resource_mut::<SessionRegistry>().insert(
        session,
        SessionEntry {
            connection_entity,
            host_anchor,
            dim,
            previous_dim: None,
            in_dim_entity,
            epoch: 0,
        },
    );
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
                .try_send(ToDim::Serverbound {
                    player: anchor_a,
                    id: i as i32,
                    data: Bytes::new(),
                    timestamp: std::time::Instant::now(),
                })
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
    assert_eq!(drained.len(), TO_DIM_CAPACITY, "channel held exactly TO_DIM_CAPACITY messages");
}

#[test]
fn spawn_succeeds_when_serverbound_full() {
    let mut world = build_world();

    let dim = world.spawn_empty().id();
    let (_srv_rx, ctl_rx, _from_tx) = make_channels(&mut world, dim);

    let in_dim = world.spawn_empty().id();
    let conn = spawn_connection(&mut world);
    let anchor = world.spawn_empty().id();
    register_session(&mut world, conn, anchor, dim, Some(in_dim));

    // Fill the serverbound channel completely.
    {
        let channels = world.resource::<DimChannelsResource>();
        let entry = channels.get(dim).expect("channel present");
        for i in 0..TO_DIM_CAPACITY {
            entry
                .serverbound_sender
                .try_send(ToDim::Serverbound {
                    player: anchor,
                    id: i as i32,
                    data: Bytes::new(),
                    timestamp: std::time::Instant::now(),
                })
                .expect("fill serverbound channel");
        }
    }

    // A Spawn via the control channel must still succeed because it uses the
    // separate non-sheddable control channel.
    {
        let channels = world.resource::<DimChannelsResource>();
        let entry = channels.get(dim).expect("channel present");
        let result = entry.control_sender.try_send(ToDim::Spawn {
            host_anchor: anchor,
            session: PlayerSession(1),
            snapshot: mcrs_minecraft::world::bus::PlayerTransferSnapshot {
                uuid: mcrs_protocol::uuid::Uuid::nil(),
                username: "test".into(),
                position: bevy_math::DVec3::ZERO,
                rotation: bevy_math::Vec2::ZERO,
            },
        });
        assert!(result.is_ok(), "Spawn must succeed even when serverbound channel is full (control reserve)");
    }

    // The Spawn message arrived on the control channel.
    let msg = ctl_rx.try_recv().expect("Spawn present in control channel");
    assert!(matches!(msg, ToDim::Spawn { .. }), "control channel received a Spawn");

    // The connection is still up; serverbound-full does not tear down the dim.
    assert!(
        world.get::<ServerSideConnection>(conn).is_some(),
        "dim must not be torn down when only serverbound is full"
    );
}

#[test]
fn dim_teardown_only_on_control_reserve_exhausted() {
    let mut world = build_world();

    let dim = world.spawn_empty().id();
    let (_srv_rx, _ctl_rx, _from_tx) = make_channels(&mut world, dim);

    let in_dim = world.spawn_empty().id();
    let conn = spawn_connection(&mut world);
    let anchor = world.spawn_empty().id();
    register_session(&mut world, conn, anchor, dim, Some(in_dim));

    // In spawn_succeeds_when_serverbound_full we proved that serverbound-full
    // alone does not exhaust the control reserve. Here we exhaust both channels.

    // Fill serverbound channel.
    {
        let channels = world.resource::<DimChannelsResource>();
        let entry = channels.get(dim).expect("channel present");
        for i in 0..TO_DIM_CAPACITY {
            entry
                .serverbound_sender
                .try_send(ToDim::Serverbound {
                    player: anchor,
                    id: i as i32,
                    data: Bytes::new(),
                    timestamp: std::time::Instant::now(),
                })
                .expect("fill serverbound");
        }
    }

    // Fill control channel.
    {
        let channels = world.resource::<DimChannelsResource>();
        let entry = channels.get(dim).expect("channel present");
        let snapshot = mcrs_minecraft::world::bus::PlayerTransferSnapshot {
            uuid: mcrs_protocol::uuid::Uuid::nil(),
            username: "test".into(),
            position: bevy_math::DVec3::ZERO,
            rotation: bevy_math::Vec2::ZERO,
        };
        for i in 0..TO_DIM_CONTROL_CAPACITY {
            entry
                .control_sender
                .try_send(ToDim::Spawn {
                    host_anchor: anchor,
                    session: PlayerSession(i as u64 + 1),
                    snapshot: snapshot.clone(),
                })
                .expect("fill control channel");
        }
    }

    // One more control message must fail — this is genuine hard-overload.
    let result = {
        let channels = world.resource::<DimChannelsResource>();
        let entry = channels.get(dim).expect("channel present");
        entry.control_sender.try_send(ToDim::Despawn { host_anchor: anchor })
    };
    assert!(
        result.is_err(),
        "control channel must reject sends when at capacity (hard-overload)"
    );

    // In spawn_succeeds_when_serverbound_full the control channel was NOT full,
    // so Spawn succeeded. Here both channels are full — the hard-overload
    // condition. The caller (bridge / dim manager) is responsible for the
    // teardown action when it observes this error. This test confirms the
    // structural condition that triggers teardown is distinct from ordinary
    // serverbound saturation.
    assert!(
        world.get::<ServerSideConnection>(conn).is_some(),
        "teardown decision belongs to the caller; this test only verifies the structural condition"
    );
}

fn transfer_snapshot() -> mcrs_minecraft::world::bus::PlayerTransferSnapshot {
    mcrs_minecraft::world::bus::PlayerTransferSnapshot {
        uuid: mcrs_protocol::uuid::Uuid::nil(),
        username: "test".into(),
        position: bevy_math::DVec3::ZERO,
        rotation: bevy_math::Vec2::ZERO,
    }
}

#[test]
fn control_full_enqueues_dim_teardown() {
    use mcrs_engine::world::sub_app::DimDespawnQueue;
    use mcrs_minecraft::world::channel_types::send_control_or_teardown;

    let mut world = World::new();
    let dim = world.spawn_empty().id();
    let snapshot = transfer_snapshot();

    // A control channel with headroom: the send is delivered and no teardown is
    // scheduled.
    let (ok_tx, ok_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let ok_sender = DimSender::new(ok_tx);
    let mut queue = DimDespawnQueue::default();
    send_control_or_teardown(&ok_sender, dim, ToDim::Despawn { host_anchor: dim }, &mut queue);
    assert!(queue.0.is_empty(), "a control channel with room must not schedule teardown");
    assert!(
        matches!(ok_rx.try_recv(), Ok(ToDim::Despawn { .. })),
        "the control message must be delivered when the channel has capacity"
    );

    // A saturated control channel (hard overload): the dim is scheduled for
    // teardown instead of silently dropping the lifecycle message. The receiver
    // is kept alive so the send reports Full, not Disconnected.
    let (full_tx, _full_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let full_sender = DimSender::new(full_tx);
    for i in 0..TO_DIM_CONTROL_CAPACITY {
        full_sender
            .try_send(ToDim::Spawn {
                host_anchor: dim,
                session: PlayerSession(i as u64 + 1),
                snapshot: snapshot.clone(),
            })
            .expect("fill control channel");
    }
    let mut queue = DimDespawnQueue::default();
    send_control_or_teardown(&full_sender, dim, ToDim::Despawn { host_anchor: dim }, &mut queue);
    assert_eq!(queue.0, vec![dim], "a saturated control channel must schedule the dim for teardown");

    // A second failed send for the same dim must not double-enqueue.
    send_control_or_teardown(&full_sender, dim, ToDim::Despawn { host_anchor: dim }, &mut queue);
    assert_eq!(queue.0, vec![dim], "teardown scheduling must be deduplicated per dim");
}

#[test]
fn bridge_transfer_full_control_schedules_teardown() {
    use mcrs_engine::world::sub_app::DimDespawnQueue;
    use mcrs_minecraft::world::bridge::bridge_player_transfer;
    use mcrs_minecraft::world::bus::OutboundPlayerTransfer;
    use mcrs_minecraft::world::sub_app_builder::DimSubAppHandle;

    let mut world = build_world();
    world.init_resource::<Messages<OutboundPlayerTransfer>>();
    world.init_resource::<DimDespawnQueue>();

    let old_dim = world.spawn(DimSubAppHandle).id();
    let dest_dim = world.spawn(DimSubAppHandle).id();

    // Keep both control receivers alive so a full sender reports Full (overload),
    // not Disconnected (dim already gone).
    let (_srv_old, _ctl_old_rx, _from_old) = make_channels(&mut world, old_dim);
    let (_srv_dest, _ctl_dest_rx, _from_dest) = make_channels(&mut world, dest_dim);

    let conn = spawn_connection(&mut world);
    let anchor = world.spawn_empty().id();
    let in_dim = world.spawn_empty().id();
    register_session(&mut world, conn, anchor, old_dim, Some(in_dim));

    let snapshot = transfer_snapshot();

    // Saturate the SOURCE dim's control channel: the transfer's Despawn to it
    // cannot land, which is the hard-overload teardown trigger.
    {
        let channels = world.resource::<DimChannelsResource>();
        let entry = channels.get(old_dim).expect("old_dim channels present");
        for i in 0..TO_DIM_CONTROL_CAPACITY {
            entry
                .control_sender
                .try_send(ToDim::Spawn {
                    host_anchor: anchor,
                    session: PlayerSession(i as u64 + 1),
                    snapshot: snapshot.clone(),
                })
                .expect("fill old_dim control channel");
        }
    }

    world
        .resource_mut::<Messages<OutboundPlayerTransfer>>()
        .write(OutboundPlayerTransfer {
            host_anchor: anchor,
            dest_dim,
            snapshot: snapshot.clone(),
        });

    let mut sys = IntoSystem::into_system(bridge_player_transfer);
    sys.initialize(&mut world);
    let _ = sys.run((), &mut world);
    sys.apply_deferred(&mut world);

    let queue = world.resource::<DimDespawnQueue>();
    assert!(
        queue.0.contains(&old_dim),
        "a real transfer caller must schedule teardown when the source control channel is saturated"
    );
    assert!(
        !queue.0.contains(&dest_dim),
        "the destination dim has control headroom and must not be torn down"
    );
}
