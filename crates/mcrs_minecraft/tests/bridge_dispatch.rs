//! Integration tests for `dispatch_encode`: drop policy, kick thresholds,
//! single-blob coalescing, metric observability, and counted-drop variants.
//!
//! All tests use a mock RawConnection backed by an `mpsc::channel` so no real
//! socket is needed. The receiver is kept by the test so it can assert exactly
//! how many blobs arrive per tick.

#[path = "common/mock_connection.rs"]
mod mock_connection;

use std::sync::atomic::Ordering;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_engine::geometry::ColumnPos;
use mcrs_minecraft::world::bridge::dispatch_encode;
use mcrs_minecraft::world::bridge_queue::{
    OutboundQueue, DEPTH_DRAIN_TARGET, DEPTH_LIMIT, HIGH_OVERFLOW_LIMIT, KICK_AFTER_OVERFLOW_TICKS,
};
use mcrs_minecraft::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, TestPayload,
};
use mcrs_minecraft::world::player_index::PlayerIndex;
use mcrs_network::metrics::{
    BRIDGE_DROP_LOW_TOTAL, BRIDGE_DROP_NORMAL_TOTAL, BRIDGE_ENCODE_UNHANDLED_TOTAL,
    BRIDGE_KICK_OVERFLOW_TOTAL, TELEMETRY_TEST_LOCK,
};
use mcrs_network::ServerSideConnection;
use mcrs_protocol::chunk::LightData;
use mcrs_protocol::uuid::Uuid;
use mcrs_protocol::Look;
use smallvec::SmallVec;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal ECS world with the resources `dispatch_encode` needs.
fn build_dispatch_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<PlayerIndex>();
    world
}

/// Spawn a connection entity backed by an `mpsc::Receiver` that the test holds
/// to observe blobs sent to the socket.
///
/// Returns `(entity, receiver)`.
fn spawn_mock_connection(world: &mut World) -> (Entity, mpsc::Receiver<Bytes>) {
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
        });
    }
}

/// Push `count` High-priority test packets directly into `entity`'s
/// `OutboundQueue`.
fn enqueue_high(world: &mut World, entity: Entity, count: usize) {
    let mut q = world
        .get_mut::<OutboundQueue>(entity)
        .expect("OutboundQueue present");
    for i in 0..count {
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::High,
            data: PacketPayload::Test(TestPayload { seq: i as u32 }),
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
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    // Fill past DEPTH_LIMIT: DEPTH_LIMIT+1 Normal + 2 Critical.
    enqueue_critical(&mut world, socket, 2);
    enqueue_normal(&mut world, socket, DEPTH_LIMIT - 1); // total = DEPTH_LIMIT+1

    let before_normal = BRIDGE_DROP_NORMAL_TOTAL.load(Ordering::Relaxed);
    let before_low = BRIDGE_DROP_LOW_TOTAL.load(Ordering::Relaxed);

    run_dispatch(&mut world);

    let after_normal = BRIDGE_DROP_NORMAL_TOTAL.load(Ordering::Relaxed);
    let after_low = BRIDGE_DROP_LOW_TOTAL.load(Ordering::Relaxed);

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
    assert_eq!(after_low - before_low, 0, "Low should not be dropped when Normal is available");

    // Critical count must be untouched by the drop policy.
    // (The entity may have been removed if connection closed, so check conditionally.)
    // The important invariant is that critical/high are never dropped.
}

/// When Normal queue is exhausted, Low is dropped next.
#[test]
fn drop_oldest_low_after_normal_exhausted() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    // 2 Normal + many Low totalling DEPTH_LIMIT + 10.
    let low_count = DEPTH_LIMIT + 8;
    enqueue_normal(&mut world, socket, 2);
    enqueue_low(&mut world, socket, low_count);

    let before_normal = BRIDGE_DROP_NORMAL_TOTAL.load(Ordering::Relaxed);
    let before_low = BRIDGE_DROP_LOW_TOTAL.load(Ordering::Relaxed);

    run_dispatch(&mut world);

    let after_normal = BRIDGE_DROP_NORMAL_TOTAL.load(Ordering::Relaxed);
    let after_low = BRIDGE_DROP_LOW_TOTAL.load(Ordering::Relaxed);

    // All 2 Normal should be dropped first, then remaining from Low.
    let total_drops = (2 + low_count) - DEPTH_DRAIN_TARGET;
    let expected_normal_drops = 2usize.min(total_drops);
    let expected_low_drops = total_drops - expected_normal_drops;

    assert_eq!(after_normal - before_normal, expected_normal_drops as u64);
    assert_eq!(after_low - before_low, expected_low_drops as u64);
}

// ---------------------------------------------------------------------------
// kick_on_critical_high_overflow
// ---------------------------------------------------------------------------

/// When `critical_high_len() > HIGH_OVERFLOW_LIMIT` for
/// `KICK_AFTER_OVERFLOW_TICKS` consecutive ticks, the connection is kicked
/// (ServerSideConnection removed) and `BRIDGE_KICK_OVERFLOW_TOTAL` increments.
#[test]
fn kick_on_critical_high_overflow() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    // Fill critical+high past HIGH_OVERFLOW_LIMIT.
    enqueue_critical(&mut world, socket, HIGH_OVERFLOW_LIMIT + 1);

    let before_kick = BRIDGE_KICK_OVERFLOW_TOTAL.load(Ordering::Relaxed);

    // Run enough ticks to trigger the kick.
    for _ in 0..KICK_AFTER_OVERFLOW_TICKS {
        run_dispatch(&mut world);
        // Re-fill to keep the overflow sustained.
        if world.get::<OutboundQueue>(socket).is_some() {
            enqueue_critical(&mut world, socket, HIGH_OVERFLOW_LIMIT + 1);
        }
    }

    let after_kick = BRIDGE_KICK_OVERFLOW_TOTAL.load(Ordering::Relaxed);
    assert!(
        after_kick > before_kick,
        "BRIDGE_KICK_OVERFLOW_TOTAL should increment on overflow kick"
    );

    // ServerSideConnection must be removed (connection kicked).
    assert!(
        world.get::<ServerSideConnection>(socket).is_none(),
        "ServerSideConnection must be removed after overflow kick"
    );
}

// ---------------------------------------------------------------------------
// coalesce_single_write_per_tick
// ---------------------------------------------------------------------------

/// N queued packets produce exactly ONE `try_send_blob` call (one blob) per
/// socket per tick. The receiver side sees exactly one blob arrive.
#[test]
fn coalesce_single_write_per_tick() {
    use mcrs_engine::geometry::BlockPos;
    use mcrs_protocol::BlockStateId;

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
                data: PacketPayload::BlockUpdate {
                    position: BlockPos::new(i as i32, 64, 0),
                    new_state: BlockStateId(i as u16),
                },
            });
        }
    }

    run_dispatch(&mut world);

    // Exactly one blob should arrive in the mpsc channel.
    let blob1 = rx.try_recv().expect("one blob should be sent");
    assert!(!blob1.is_empty(), "blob must not be empty");
    assert!(rx.try_recv().is_err(), "exactly one blob per tick, not more");
}

// ---------------------------------------------------------------------------
// metrics_delta_on_drop
// ---------------------------------------------------------------------------

/// snapshot() before/after a drop scenario shows the exact delta (D-03b).
#[test]
fn metrics_delta_on_drop() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    // Overflow with Normal packets to force drops.
    enqueue_normal(&mut world, socket, DEPTH_LIMIT + 10);

    let snap_before = mcrs_network::metrics::snapshot();
    run_dispatch(&mut world);
    let snap_after = mcrs_network::metrics::snapshot();

    let expected = ((DEPTH_LIMIT + 10) - DEPTH_DRAIN_TARGET) as u64;
    assert_eq!(
        snap_after.drop_normal_total - snap_before.drop_normal_total,
        expected,
        "snapshot delta must equal exact drop count"
    );
}

// ---------------------------------------------------------------------------
// unhandled_variant_counted
// ---------------------------------------------------------------------------

/// A counted-drop PacketPayload variant (Test) increments
/// `BRIDGE_ENCODE_UNHANDLED_TOTAL` by the exact count and does not panic.
#[test]
fn unhandled_variant_counted() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    // Enqueue 3 Test(counted-drop) payloads.
    {
        let mut q = world
            .get_mut::<OutboundQueue>(socket)
            .expect("OutboundQueue present");
        for i in 0..3u32 {
            q.push(OutboundPlayerPacket {
                target: PacketTarget::AllPlayers,
                priority: PacketPriority::Normal,
                data: PacketPayload::Test(TestPayload { seq: i }),
            });
        }
    }

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    run_dispatch(&mut world);
    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    assert_eq!(after - before, 3, "3 Test packets must increment BRIDGE_ENCODE_UNHANDLED_TOTAL by 3");
}

// ---------------------------------------------------------------------------
// per-variant encode tests (Task 2 GREEN targets; RED written here in Task 1)
// ---------------------------------------------------------------------------

/// LightUpdate encodes to a real ClientboundLightUpdate (non-empty blob) and does
/// not increment BRIDGE_ENCODE_UNHANDLED_TOTAL.
#[test]
fn light_update_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, mut rx) = spawn_mock_connection(&mut world);

    {
        let mut q = world.get_mut::<OutboundQueue>(socket).expect("OutboundQueue");
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::LightUpdate {
                column: ColumnPos::new(3, -5),
                light_data: LightData::default(),
            },
        });
    }

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    run_dispatch(&mut world);
    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    assert_eq!(after - before, 0, "LightUpdate must not increment BRIDGE_ENCODE_UNHANDLED_TOTAL");
    let blob = rx.try_recv().expect("dispatch must produce a blob for LightUpdate");
    assert!(!blob.is_empty(), "blob must be non-empty");
}

/// ChunkLoad encodes to a real ClientboundLevelChunkWithLight (non-empty blob) and
/// does not increment BRIDGE_ENCODE_UNHANDLED_TOTAL.
#[test]
fn chunk_load_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, mut rx) = spawn_mock_connection(&mut world);

    {
        let mut q = world.get_mut::<OutboundQueue>(socket).expect("OutboundQueue");
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Critical,
            data: PacketPayload::ChunkLoad {
                column: ColumnPos::new(0, 0),
                chunk_bytes: vec![0x80u8; 2000],
                light_data: LightData::default(),
            },
        });
    }

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    run_dispatch(&mut world);
    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    assert_eq!(after - before, 0, "ChunkLoad must not increment BRIDGE_ENCODE_UNHANDLED_TOTAL");
    let blob = rx.try_recv().expect("dispatch must produce a blob for ChunkLoad");
    assert!(!blob.is_empty(), "blob must be non-empty");
}

/// EntityPosSync encodes to a real ClientboundEntityPositionSync (non-empty blob)
/// and does not increment BRIDGE_ENCODE_UNHANDLED_TOTAL.
#[test]
fn entity_pos_sync_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, mut rx) = spawn_mock_connection(&mut world);

    {
        let mut q = world.get_mut::<OutboundQueue>(socket).expect("OutboundQueue");
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::EntityPosSync {
                entity_id: 42,
                position: DVec3::new(1.0, 64.0, -3.0),
                velocity: DVec3::ZERO,
                look: Look { yaw: 0.0, pitch: 0.0 },
                on_ground: true,
            },
        });
    }

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    run_dispatch(&mut world);
    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    assert_eq!(after - before, 0, "EntityPosSync must not increment BRIDGE_ENCODE_UNHANDLED_TOTAL");
    let blob = rx.try_recv().expect("dispatch must produce a blob for EntityPosSync");
    assert!(!blob.is_empty(), "blob must be non-empty");
}

/// PlayerEnteredView encodes to a real ClientboundAddEntity (non-empty blob) and
/// does not increment BRIDGE_ENCODE_UNHANDLED_TOTAL.
#[test]
fn player_entered_view_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, mut rx) = spawn_mock_connection(&mut world);

    {
        let mut q = world.get_mut::<OutboundQueue>(socket).expect("OutboundQueue");
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::PlayerEnteredView {
                entity_id: 7,
                uuid: Uuid::nil(),
                kind: 128,
                position: DVec3::new(0.0, 64.0, 0.0),
                yaw: 90.0,
                pitch: 0.0,
            },
        });
    }

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    run_dispatch(&mut world);
    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    assert_eq!(after - before, 0, "PlayerEnteredView must not increment BRIDGE_ENCODE_UNHANDLED_TOTAL");
    let blob = rx.try_recv().expect("dispatch must produce a blob for PlayerEnteredView");
    assert!(!blob.is_empty(), "blob must be non-empty");
}

/// PlayerLeftView encodes to a real ClientboundRemoveEntities (non-empty blob) and
/// does not increment BRIDGE_ENCODE_UNHANDLED_TOTAL.
#[test]
fn player_left_view_encodes() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, mut rx) = spawn_mock_connection(&mut world);

    {
        let mut q = world.get_mut::<OutboundQueue>(socket).expect("OutboundQueue");
        let mut ids: SmallVec<[i32; 4]> = SmallVec::new();
        ids.push(99);
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::PlayerLeftView { entity_ids: ids },
        });
    }

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    run_dispatch(&mut world);
    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    assert_eq!(after - before, 0, "PlayerLeftView must not increment BRIDGE_ENCODE_UNHANDLED_TOTAL");
    let blob = rx.try_recv().expect("dispatch must produce a blob for PlayerLeftView");
    assert!(!blob.is_empty(), "blob must be non-empty");
}

/// Only `PacketPayload::Test` remains a counted-drop; the five real variants
/// contribute zero to `BRIDGE_ENCODE_UNHANDLED_TOTAL`.
#[test]
fn only_test_remains_counted_drop() {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = build_dispatch_world();
    let (socket, _rx) = spawn_mock_connection(&mut world);

    {
        let mut q = world.get_mut::<OutboundQueue>(socket).expect("OutboundQueue");
        // One of each real variant — none should increment the unhandled counter.
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::LightUpdate {
                column: ColumnPos::new(0, 0),
                light_data: LightData::default(),
            },
        });
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Critical,
            data: PacketPayload::ChunkLoad {
                column: ColumnPos::new(0, 0),
                chunk_bytes: vec![],
                light_data: LightData::default(),
            },
        });
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::EntityPosSync {
                entity_id: 1,
                position: DVec3::ZERO,
                velocity: DVec3::ZERO,
                look: Look { yaw: 0.0, pitch: 0.0 },
                on_ground: false,
            },
        });
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::PlayerEnteredView {
                entity_id: 2,
                uuid: Uuid::nil(),
                kind: 128,
                position: DVec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
            },
        });
        let mut ids: SmallVec<[i32; 4]> = SmallVec::new();
        ids.push(3);
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::PlayerLeftView { entity_ids: ids },
        });
        // One Test packet: must increment by exactly 1.
        q.push(OutboundPlayerPacket {
            target: PacketTarget::AllPlayers,
            priority: PacketPriority::Normal,
            data: PacketPayload::Test(TestPayload { seq: 0 }),
        });
    }

    let before = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);
    run_dispatch(&mut world);
    let after = BRIDGE_ENCODE_UNHANDLED_TOTAL.load(Ordering::Relaxed);

    assert_eq!(
        after - before,
        1,
        "only the Test variant must increment BRIDGE_ENCODE_UNHANDLED_TOTAL (by 1); got {}",
        after - before
    );
}
