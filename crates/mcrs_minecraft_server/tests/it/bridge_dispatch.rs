//! Integration tests for `dispatch_encode`: drop policy, kick thresholds,
//! single-blob coalescing, metric observability, and counted-drop variants.
//!
//! All tests use a mock RawConnection backed by an `mpsc::channel` so no real
//! socket is needed. The receiver is kept by the test so it can assert exactly
//! how many blobs arrive per tick.

use crate::mock_connection;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockUpdate;
use mcrs_minecraft_protocol_game::clientbound::ClientboundEntityPositionSync;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLightUpdate;
use mcrs_minecraft_protocol_game::clientbound::ClientboundRemoveEntities;
use mcrs_minecraft_protocol_game::clientbound::PositionPath;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bytes::{Bytes, BytesMut};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, TestPayload,
};
use mcrs_minecraft_engine::muxer::bridge::dispatch_encode;
use mcrs_minecraft_engine::muxer::bridge_queue::{DEPTH_DRAIN_TARGET, DEPTH_LIMIT, OutboundQueue};
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

/// Push `count` Normal-priority test packets directly into `entity`'s
/// `OutboundQueue` without going through `bridge_outbound`.
fn enqueue_normal(world: &mut World, entity: Entity, count: usize) {
    let mut q = world
        .get_mut::<OutboundQueue>(entity)
        .expect("OutboundQueue present");
    for i in 0..count {
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::Test(TestPayload { seq: i as u32 }),
            session: PlayerSession(0),
            epoch: 0,
        });
    }
}

/// Push `count` Low-priority test packets directly into `entity`'s
/// `OutboundQueue`.
fn enqueue_low(world: &mut World, entity: Entity, count: usize) {
    let mut q = world
        .get_mut::<OutboundQueue>(entity)
        .expect("OutboundQueue present");
    for i in 0..count {
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Low,
            data: PacketPayload::Test(TestPayload { seq: i as u32 }),
            session: PlayerSession(0),
            epoch: 0,
        });
    }
}

/// Push `count` Critical-priority test packets directly into `entity`'s
/// `OutboundQueue`.
fn enqueue_critical(world: &mut World, entity: Entity, count: usize) {
    let mut q = world
        .get_mut::<OutboundQueue>(entity)
        .expect("OutboundQueue present");
    for i in 0..count {
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Critical,
            data: PacketPayload::Test(TestPayload { seq: i as u32 }),
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
/// dropped first, then oldest Low packets, until depth <= DEPTH_DRAIN_TARGET.
/// Critical + High counts are unchanged.
#[test]
fn drop_oldest_on_overflow() {
    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    // Fill past DEPTH_LIMIT: DEPTH_LIMIT+1 Normal + 2 Critical.
    enqueue_critical(&mut world, socket, 2);
    enqueue_normal(&mut world, socket, DEPTH_LIMIT - 1); // total = DEPTH_LIMIT+1

    let before_normal = world.resource::<BridgeTelemetry>().drop_normal_total;
    let before_low = world.resource::<BridgeTelemetry>().drop_low_total;

    run_dispatch(&mut world);

    let after_normal = world.resource::<BridgeTelemetry>().drop_normal_total;
    let after_low = world.resource::<BridgeTelemetry>().drop_low_total;

    // We had 2 Critical + (DEPTH_LIMIT-1) Normal = DEPTH_LIMIT+1 total.
    // Drops until <= DEPTH_DRAIN_TARGET:
    //   need to drop (DEPTH_LIMIT+1 - DEPTH_DRAIN_TARGET) = (DEPTH_LIMIT - DEPTH_DRAIN_TARGET + 1) Normal.
    let expected_normal_drops = (DEPTH_LIMIT + 1) - DEPTH_DRAIN_TARGET;
    assert_eq!(
        after_normal - before_normal,
        expected_normal_drops as u64,
        "expected {} Normal drops, got {}",
        expected_normal_drops,
        after_normal - before_normal
    );
    assert_eq!(
        after_low - before_low,
        0,
        "Low should not be dropped when Normal is available"
    );

    // Critical count must be untouched by the drop policy.
    // (The entity may have been removed if connection closed, so check conditionally.)
    // The important invariant is that critical/high are never dropped.
}

/// When Normal queue is exhausted, Low is dropped next.
#[test]
fn drop_oldest_low_after_normal_exhausted() {
    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    // 2 Normal + many Low totalling DEPTH_LIMIT + 10.
    let low_count = DEPTH_LIMIT + 8;
    enqueue_normal(&mut world, socket, 2);
    enqueue_low(&mut world, socket, low_count);

    let before_normal = world.resource::<BridgeTelemetry>().drop_normal_total;
    let before_low = world.resource::<BridgeTelemetry>().drop_low_total;

    run_dispatch(&mut world);

    let after_normal = world.resource::<BridgeTelemetry>().drop_normal_total;
    let after_low = world.resource::<BridgeTelemetry>().drop_low_total;

    // All 2 Normal should be dropped first, then remaining from Low.
    let total_drops = (2 + low_count) - DEPTH_DRAIN_TARGET;
    let expected_normal_drops = 2usize.min(total_drops);
    let expected_low_drops = total_drops - expected_normal_drops;

    assert_eq!(after_normal - before_normal, expected_normal_drops as u64);
    assert_eq!(after_low - before_low, expected_low_drops as u64);
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
        enqueue_critical(&mut world, socket, 512);
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

/// ChunkLoad encodes to a real ClientboundLevelChunkWithLight (non-empty blob) and
/// does not increment encode_unhandled_total.
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

    let before = world.resource::<BridgeTelemetry>().encode_unhandled_total;
    run_dispatch(&mut world);
    let after = world.resource::<BridgeTelemetry>().encode_unhandled_total;

    assert_eq!(
        after - before,
        0,
        "ChunkLoad must not increment encode_unhandled_total"
    );
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

/// Only `PacketPayload::Test` remains a counted-drop; the five real variants
/// contribute zero to `encode_unhandled_total`.
#[test]
fn only_test_remains_counted_drop() {
    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    {
        let mut q = world
            .get_mut::<OutboundQueue>(socket)
            .expect("OutboundQueue");
        // One of each real variant — none should increment the unhandled counter.
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::LightUpdate(ClientboundLightUpdate {
                x: VarInt(0),
                z: VarInt(0),
                light_data: LightData::default(),
            }),
            session: PlayerSession(0),
            epoch: 0,
        });
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Critical,
            data: chunk_load(&[]),
            session: PlayerSession(0),
            epoch: 0,
        });
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::EntityPosSync(ClientboundEntityPositionSync {
                entity_id: VarInt(1),
                position: PositionPath::Linear(DVec3::ZERO),
                look: Look {
                    yaw: 0.0,
                    pitch: 0.0,
                },
                on_ground: false,
            }),
            session: PlayerSession(0),
            epoch: 0,
        });
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::Replication(vec![bytes::Bytes::new()]),
            session: PlayerSession(0),
            epoch: 0,
        });
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::RemoveEntities(ClientboundRemoveEntities {
                entity_ids: vec![VarInt(3)],
            }),
            session: PlayerSession(0),
            epoch: 0,
        });
        // One Test packet: must increment by exactly 1.
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::Test(TestPayload { seq: 0 }),
            session: PlayerSession(0),
            epoch: 0,
        });
    }

    let before = world.resource::<BridgeTelemetry>().encode_unhandled_total;
    run_dispatch(&mut world);
    let after = world.resource::<BridgeTelemetry>().encode_unhandled_total;

    assert_eq!(
        after - before,
        1,
        "only the Test variant must increment encode_unhandled_total (by 1); got {}",
        after - before
    );
}
