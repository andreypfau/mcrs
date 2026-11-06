//! Integration tests for `dispatch_encode`: drop policy, kick thresholds,
//! single-blob coalescing and shared framing.
//!
//! All tests use a mock RawConnection backed by an `mpsc::channel` so no real
//! socket is needed. The receiver is kept by the test so it can assert exactly
//! how many blobs arrive per tick.

use crate::mock_connection;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockUpdate;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bytes::{Bytes, BytesMut};
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use mcrs_minecraft_engine::muxer::bridge::{DEPTH_DRAIN_TARGET, DEPTH_LIMIT, dispatch_encode};
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_network::OutboundReceiver;
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_network::SwitchMode;
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::packets::login::clientbound::LoginCompression;
use mcrs_minecraft_protocol::{CompressionThreshold, Decode, PacketDecoder};
use mcrs_minecraft_protocol_game::chunk::{ChunkData, LightData};
use mcrs_minecraft_protocol_game::clientbound::ClientboundLevelChunkWithLight;
use mcrs_voxel::ColumnPos;
use std::borrow::Cow;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal ECS world with the resources `dispatch_encode` needs.
fn build_dispatch_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<BridgeTelemetry>();
    world.init_resource::<mcrs_minecraft_engine::muxer::bridge::PendingConnections>();
    world
}

fn chunk_load(chunk_bytes: &[u8]) -> PacketPayload {
    let column = ColumnPos::new(0, 0);
    let mut packet = Vec::new();
    ClientboundLevelChunkWithLight {
        pos: column,
        chunk_data: ChunkData {
            heightmaps: Vec::new(),
            data: chunk_bytes,
            block_entities: Cow::Borrowed(&[]),
        },
        light_data: LightData::default(),
    }
    .encode_with_id(&mut packet)
    .unwrap();
    PacketPayload::ChunkLoad {
        column,
        packet: Bytes::from(packet),
    }
}

/// Spawn a connection entity backed by an `mpsc::Receiver` that the test holds
/// to observe blobs sent to the socket.
///
/// Returns `(entity, receiver)`.
fn spawn_mock_connection(world: &mut World) -> (Entity, OutboundReceiver) {
    let (raw, rx) = mock_connection::make_mock_raw_connection();
    let entity = world
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            OutboundQueue::default(),
        ))
        .id();
    (entity, rx)
}

/// Push `count` small packets at `priority` directly into `entity`'s
/// `OutboundQueue` without going through `bridge_outbound`.
fn enqueue(world: &mut World, entity: Entity, priority: PacketPriority, count: usize) {
    let mut q = world
        .get_mut::<OutboundQueue>(entity)
        .expect("OutboundQueue present");
    for i in 0..count {
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority,
            data: mock_connection::marker(i as u32),
            session: PlayerSession(0),
            epoch: 0,
        });
    }
}

fn run_dispatch(world: &mut World) {
    let mut sys = IntoSystem::into_system(dispatch_encode);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

// ---------------------------------------------------------------------------
// drop_oldest_on_overflow
// ---------------------------------------------------------------------------

/// When total queue depth exceeds `DEPTH_LIMIT`, oldest Normal packets are
/// dropped until depth <= DEPTH_DRAIN_TARGET.
#[test]
fn drop_oldest_on_overflow() {
    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    enqueue(&mut world, socket, PacketPriority::Critical, 2);
    enqueue(&mut world, socket, PacketPriority::Normal, DEPTH_LIMIT - 1);

    run_dispatch(&mut world);

    assert_eq!(
        world.resource::<BridgeTelemetry>().drop_normal_total,
        ((DEPTH_LIMIT + 1) - DEPTH_DRAIN_TARGET) as u64
    );
}

/// Once Normal is empty, the shedding stops: Critical is never dropped.
#[test]
fn shedding_stops_when_normal_is_empty() {
    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    enqueue(&mut world, socket, PacketPriority::Normal, 2);
    enqueue(
        &mut world,
        socket,
        PacketPriority::Critical,
        DEPTH_LIMIT + 8,
    );

    run_dispatch(&mut world);

    assert_eq!(world.resource::<BridgeTelemetry>().drop_normal_total, 2);
}

// ---------------------------------------------------------------------------
// deep_critical_queue_is_not_kicked
// ---------------------------------------------------------------------------

/// A tick's worth of critical packets is what the server produced, not what the
/// client failed to take, so no depth of critical backlog kicks the connection.
/// Columns are paced by the client's batch acknowledgements instead.
#[test]
fn deep_critical_queue_is_not_kicked() {
    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    let before_kick = world.resource::<BridgeTelemetry>().kick_overflow_total;

    for _ in 0..8 {
        enqueue(&mut world, socket, PacketPriority::Critical, 512);
        run_dispatch(&mut world);
    }

    assert_eq!(
        world.resource::<BridgeTelemetry>().kick_overflow_total,
        before_kick,
        "a deep critical queue must not count as an overflow kick"
    );
    assert!(
        world.get::<ServerSideConnection>(socket).is_some(),
        "the connection must survive a deep critical queue"
    );
}

// ---------------------------------------------------------------------------
// coalesce_single_write_per_tick
// ---------------------------------------------------------------------------

/// N queued packets produce exactly one hand-over to the writer per
/// socket per tick. The receiver side sees exactly one blob arrive.
#[test]
fn coalesce_single_write_per_tick() {
    use mcrs_minecraft_protocol_game::block::BlockStateNumber;
    use mcrs_voxel::BlockPos;

    let mut world = build_dispatch_world();
    let (socket, mut rx) = spawn_mock_connection(&mut world);

    // Enqueue several BlockUpdate packets — these are MAPPED variants that
    // produce real encoded bytes, so the blob is non-empty.
    {
        let mut q = world
            .get_mut::<OutboundQueue>(socket)
            .expect("OutboundQueue");
        for i in 0..5u32 {
            q.push(OutboundPlayerPacket {
                target: PacketTarget::AllPlayers,
                priority: PacketPriority::Normal,
                data: PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                    block_pos: BlockPos::new(i as i32, 64, 0),
                    block_state_id: BlockStateNumber(i as u16),
                }),
                session: PlayerSession(0),
                epoch: 0,
            });
        }
    }

    run_dispatch(&mut world);

    // Exactly one blob should arrive in the mpsc channel.
    let blob1 = rx.try_recv().expect("one blob should be sent");
    assert!(!blob1.is_empty(), "blob must not be empty");
    assert!(
        rx.try_recv().is_err(),
        "exactly one blob per tick, not more"
    );
}

// ---------------------------------------------------------------------------
// per-variant encode tests
// ---------------------------------------------------------------------------

/// ChunkLoad encodes to a real ClientboundLevelChunkWithLight (non-empty blob).
#[test]
fn chunk_load_encodes() {
    let mut world = build_dispatch_world();
    let (socket, mut rx) = spawn_mock_connection(&mut world);

    {
        let mut q = world
            .get_mut::<OutboundQueue>(socket)
            .expect("OutboundQueue");
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Critical,
            data: chunk_load(&[0x80u8; 2000]),
            session: PlayerSession(0),
            epoch: 0,
        });
    }

    run_dispatch(&mut world);

    let blob = rx
        .try_recv()
        .expect("dispatch must produce a blob for ChunkLoad");
    assert!(!blob.is_empty(), "blob must be non-empty");
}

/// A column that three connections receive at two compression thresholds is framed once for
/// each threshold, and each connection reads it back at its own threshold.
#[test]
fn a_shared_column_is_framed_once_for_each_threshold() {
    let mut world = build_dispatch_world();
    let thresholds = [None, Some(256), Some(256)];
    let mut sockets = Vec::new();
    for threshold in thresholds {
        let (mut raw, rx) = mock_connection::make_mock_raw_connection();
        raw = raw.with_switch_mode(SwitchMode::Immediate);
        if let Some(threshold) = threshold {
            raw.announce_compression(CompressionThreshold(threshold));
        }
        let entity = world
            .spawn((
                ServerSideConnection { raw: Box::new(raw) },
                OutboundQueue::default(),
            ))
            .id();
        sockets.push((entity, rx, threshold));
    }
    let column = chunk_load(&[0x80u8; 2000]);
    let PacketPayload::ChunkLoad { packet, .. } = &column else {
        unreachable!("chunk_load makes a ChunkLoad");
    };
    let mut expected = &packet[..];
    let expected_id = VarInt::decode(&mut expected).unwrap().0;
    let expected_body = expected.to_vec();
    for (socket, _, _) in &sockets {
        world
            .get_mut::<OutboundQueue>(*socket)
            .unwrap()
            .push(OutboundPlayerPacket {
                target: PacketTarget::AllPlayers,
                priority: PacketPriority::Critical,
                data: column.clone(),
                session: PlayerSession(0),
                epoch: 0,
            });
    }

    run_dispatch(&mut world);

    let telemetry = *world.resource::<BridgeTelemetry>();
    assert_eq!(
        telemetry.shared_frames_total, 2,
        "one frame for each threshold"
    );
    assert_eq!(telemetry.framed_appends_total, 3);
    for (_, rx, threshold) in &mut sockets {
        let mut decoder = PacketDecoder::new();
        decoder.queue_bytes(BytesMut::from(&rx.try_recv().unwrap()[..]));
        if let Some(threshold) = threshold {
            let announce = decoder.try_next_packet().unwrap().unwrap();
            assert_eq!(announce.id, LoginCompression::ID);
            decoder.set_compression(CompressionThreshold(*threshold));
        }
        let frame = decoder.try_next_packet().unwrap().unwrap();
        assert_eq!(frame.id, expected_id, "threshold {threshold:?}");
        assert_eq!(
            &frame.body[..],
            &expected_body[..],
            "threshold {threshold:?}"
        );
        assert!(decoder.try_next_packet().unwrap().is_none());
    }
}

#[test]
fn a_shared_packet_that_one_connection_sheds_is_not_framed_for_the_other() {
    let mut world = build_dispatch_world();
    let (sending, mut rx) = spawn_mock_connection(&mut world);
    let (shedding, _shedding_rx) = spawn_mock_connection(&mut world);
    let PacketPayload::ChunkLoad { packet, .. } = chunk_load(&[0x80u8; 2000]) else {
        unreachable!("a column")
    };
    // A column is never shed; the command tree is, as a shared packet that no prediction needs.
    let shared = OutboundPlayerPacket {
        target: PacketTarget::AllPlayers,
        priority: PacketPriority::Normal,
        data: PacketPayload::Commands(packet),
        session: PlayerSession(0),
        epoch: 0,
    };
    for socket in [sending, shedding] {
        world
            .get_mut::<OutboundQueue>(socket)
            .unwrap()
            .push(shared.clone());
    }
    enqueue(&mut world, shedding, PacketPriority::Normal, DEPTH_LIMIT);

    run_dispatch(&mut world);

    assert_eq!(
        world.resource::<BridgeTelemetry>().shared_frames_total,
        0,
        "the one connection that sends the packet leaves the framing to its writer"
    );
    assert!(rx.try_recv().is_ok());
}

// ---------------------------------------------------------------------------
// wire order with shedding
// ---------------------------------------------------------------------------

/// What one packet on the wire is, for an order test.
#[derive(Clone, Debug, PartialEq)]
enum Wire {
    Update(i32),
    Event(i32),
    Position,
    Probe(i32),
}

fn payload_of(wire: &Wire) -> (PacketPriority, PacketPayload) {
    use mcrs_minecraft_protocol_game::block::BlockStateNumber;
    use mcrs_minecraft_protocol_game::clientbound::{
        ClientboundBlockEvent, ClientboundPlayerPosition,
    };
    use mcrs_voxel::BlockPos;

    match *wire {
        Wire::Update(x) => (
            PacketPriority::Normal,
            PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                block_pos: BlockPos::new(x, 64, 0),
                block_state_id: BlockStateNumber(1),
            }),
        ),
        Wire::Event(x) => (
            PacketPriority::Normal,
            PacketPayload::BlockEvent(ClientboundBlockEvent {
                pos: BlockPos::new(x, 64, 0),
                action: 0,
                param: 0,
                block: VarInt(1),
            }),
        ),
        Wire::Position => (
            PacketPriority::Critical,
            PacketPayload::PlayerPosition(ClientboundPlayerPosition {
                teleport_id: VarInt(1),
                position: bevy_math::DVec3::ZERO,
                velocity: bevy_math::DVec3::ZERO,
                look: mcrs_minecraft_core::Look::default(),
                flags: mcrs_minecraft_protocol_game::PositionFlags::default(),
            }),
        ),
        Wire::Probe(_) => unreachable!("the test writes no probe"),
    }
}

/// The packets of `blob`, as the client reads them.
fn wire_of(blob: &[u8]) -> Vec<Wire> {
    use mcrs_minecraft_protocol_game::clientbound::{
        ClientboundBlockEvent, ClientboundPing, ClientboundPlayerPosition,
    };

    let mut decoder = PacketDecoder::new();
    decoder.queue_bytes(BytesMut::from(blob));
    let mut wire = Vec::new();
    while let Some(frame) = decoder.try_next_packet().unwrap() {
        let mut body = &frame.body[..];
        wire.push(match frame.id {
            ClientboundBlockUpdate::ID => Wire::Update(
                ClientboundBlockUpdate::decode(&mut body)
                    .unwrap()
                    .block_pos
                    .x,
            ),
            ClientboundBlockEvent::ID => {
                Wire::Event(ClientboundBlockEvent::decode(&mut body).unwrap().pos.x)
            }
            ClientboundPlayerPosition::ID => Wire::Position,
            ClientboundPing::ID => Wire::Probe(ClientboundPing::decode(&mut body).unwrap().id),
            id => panic!("an unexpected packet {id}"),
        });
    }
    wire
}

/// MVC-LAT-001, MVC-TICK-002: a queue past its depth sheds only packets that the client may
/// miss. Every block update and the teleport keep their place in the order of the writes, and
/// the probes bracket the predicted changes of the flush on the wire.
#[test]
fn a_shedding_queue_keeps_the_wire_order_of_the_predicted_changes_between_probes() {
    use mcrs_minecraft_engine::muxer::probes::Probes;

    let mut world = build_dispatch_world();
    let (socket, mut rx) = spawn_mock_connection(&mut world);
    world.entity_mut(socket).insert(Probes::default());
    let written: Vec<Wire> = (0..300)
        .map(|x| match x {
            150 => Wire::Position,
            x if x % 10 == 5 => Wire::Update(x),
            x => Wire::Event(x),
        })
        .collect();
    {
        let mut queue = world.get_mut::<OutboundQueue>(socket).unwrap();
        for wire in &written {
            let (priority, data) = payload_of(wire);
            queue.push(OutboundPlayerPacket {
                target: PacketTarget::AllPlayers,
                priority,
                data,
                session: PlayerSession(0),
                epoch: 0,
            });
        }
    }

    run_dispatch(&mut world);

    let shed = written.len() - DEPTH_DRAIN_TARGET;
    let mut events_to_shed = shed;
    let mut expected: Vec<Wire> = written
        .iter()
        .filter(|wire| {
            let drop = matches!(wire, Wire::Event(_)) && events_to_shed > 0;
            events_to_shed -= usize::from(drop);
            !drop
        })
        .cloned()
        .collect();
    let first = expected
        .iter()
        .position(|wire| matches!(wire, Wire::Update(_)))
        .unwrap();
    let last = expected
        .iter()
        .rposition(|wire| matches!(wire, Wire::Update(_)))
        .unwrap();
    expected.insert(last + 1, Wire::Probe(1));
    expected.insert(first, Wire::Probe(0));

    let wire = wire_of(&rx.try_recv().unwrap());
    assert_eq!(
        world.resource::<BridgeTelemetry>().drop_normal_total,
        shed as u64
    );
    assert_eq!(wire, expected);
}
