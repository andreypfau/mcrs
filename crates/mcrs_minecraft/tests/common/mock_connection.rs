//! Minimal ECS world helpers for bridge routing tests.
//!
//! These tests exercise `bridge_outbound` (packet routing only, no sockets).
//! The world carries `OutboundQueue` + `SessionRegistry` but no real network
//! transport — socket I/O belongs to separate dispatch tests.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use bytes::Bytes;
use mcrs_engine::session::{PlayerSession, PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_minecraft::world::bridge_queue::OutboundQueue;
use mcrs_minecraft::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, TestPayload};
use mcrs_minecraft::world::player_index::PlayerIndex;
use mcrs_network::RawConnection;
use tokio::sync::mpsc;

/// Build a bare world with the resources needed for `bridge_outbound` tests.
pub fn build_bridge_world() -> World {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<SessionRegistry>();
    world.init_resource::<PlayerIndex>();
    world
}

/// Spawn a connection entity with an `OutboundQueue` and return its entity id.
pub fn spawn_connection(world: &mut World) -> Entity {
    world.spawn(OutboundQueue::default()).id()
}

/// Register a player in `SessionRegistry`. `player` is treated as the
/// host_anchor; `socket` is the connection entity that packets route to.
/// Returns the allocated `PlayerSession`.
pub fn register_player(
    world: &mut World,
    player: Entity,
    socket: Entity,
    dim: Entity,
) -> PlayerSession {
    if !world.contains_resource::<PlayerSessionCounter>() {
        world.init_resource::<PlayerSessionCounter>();
    }
    let session = world.resource_mut::<PlayerSessionCounter>().next();
    world.resource_mut::<SessionRegistry>().insert(
        session,
        SessionEntry {
            connection_entity: socket,
            host_anchor: player,
            dim,
            previous_dim: None,
            in_dim_entity: Some(socket),
            epoch: 0,
        },
    );
    session
}

/// Write a test packet addressed to `target` into the world's
/// `Messages<OutboundPlayerPacket>`. Pass `session` + `epoch` to simulate
/// the stamp the extract closure applies; use `PlayerSession(0)` / `0` for
/// fan-out targets that are epoch-checked per-recipient in bridge_outbound.
pub fn write_packet(
    world: &mut World,
    target: PacketTarget,
    session: PlayerSession,
    epoch: u32,
    priority: PacketPriority,
    seq: u32,
) {
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target,
            priority,
            data: PacketPayload::Test(TestPayload { seq }),
            session,
            epoch,
        });
}

/// Convenience wrapper: write a packet with `PlayerSession(0)` / epoch 0,
/// suitable for fan-out targets (`AllInDim`, `AllPlayers`, `PlayerSet`) when
/// all sessions in the registry are also at epoch 0.
pub fn write_packet_broadcast(
    world: &mut World,
    target: PacketTarget,
    priority: PacketPriority,
    seq: u32,
) {
    write_packet(world, target, PlayerSession(0), 0, priority, seq);
}

/// Write a stamped test packet for the epoch-filter tests.
pub fn write_packet_stamped(
    world: &mut World,
    session: PlayerSession,
    epoch: u32,
    priority: PacketPriority,
    seq: u32,
) {
    // Construct a dummy dim entity for the target (irrelevant — bridge_outbound
    // uses msg.session for SinglePlayer, not the entity inside PacketTarget).
    let dummy = Entity::from_raw_u32(9999).expect("nonzero");
    write_packet(world, PacketTarget::SinglePlayer(dummy), session, epoch, priority, seq);
}

/// Register a session directly (bypassing the PlayerSessionCounter).
/// Used by epoch-filter tests that need precise control over session ids.
pub fn register_session(
    world: &mut World,
    session: PlayerSession,
    socket: Entity,
    epoch: u32,
) {
    let anchor = Entity::from_raw_u32(9997).expect("nonzero");
    let dim = Entity::from_raw_u32(9998).expect("nonzero");
    world.resource_mut::<SessionRegistry>().insert(
        session,
        SessionEntry {
            connection_entity: socket,
            host_anchor: anchor,
            dim,
            previous_dim: None,
            in_dim_entity: None,
            epoch,
        },
    );
}

/// Build a world identical to `build_bridge_world` but with no `PlayerIndex`
/// dependency — suitable for epoch-filter tests that only need `SessionRegistry`.
pub fn build_bridge_world_with_sessions() -> World {
    build_bridge_world()
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

/// Create a mock `RawConnection` backed by an in-memory mpsc channel.
///
/// Returns the `(RawConnection, Receiver<Bytes>)` pair so tests can observe
/// every blob that would be sent to a real socket.
///
/// The dummy JoinHandle tasks are spawned onto a process-global single-thread
/// runtime that lives for the entire test binary lifetime. Abort on
/// `RawConnection` drop cleans up the handle slots; the runtime itself never
/// shuts down.
pub fn make_mock_raw_connection() -> (RawConnection, mpsc::Receiver<Bytes>) {
    use std::sync::OnceLock;
    use tokio::runtime::Runtime;

    static TEST_RT: OnceLock<Runtime> = OnceLock::new();
    let rt = TEST_RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("test tokio runtime")
    });

    let (outgoing_tx, outgoing_rx) = mpsc::channel::<Bytes>(16);
    let raw = rt.block_on(async { RawConnection::new_for_test(outgoing_tx) });
    (raw, outgoing_rx)
}

/// Create a full mock `RawConnection` with controllable inbound packets.
///
/// Returns `(RawConnection, outgoing_rx, inbound_tx)` so the test can both
/// observe outgoing blobs and inject incoming `ReceivedPacket` values.
pub fn make_mock_raw_connection_full() -> (
    RawConnection,
    mpsc::Receiver<Bytes>,
    mpsc::Sender<mcrs_network::ReceivedPacket>,
) {
    use std::sync::OnceLock;
    use tokio::runtime::Runtime;

    static TEST_RT: OnceLock<Runtime> = OnceLock::new();
    let rt = TEST_RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("test tokio runtime")
    });

    rt.block_on(async { RawConnection::new_for_test_full(16) })
}
