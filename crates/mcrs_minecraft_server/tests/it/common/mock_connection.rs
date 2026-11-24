use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageRegistry;
use bevy_ecs::message::Messages;
use bevy_ecs::system::{IntoSystem, System};
use bevy_ecs::world::World;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_engine::muxer::bridge_queue::{OutboundQueue, QueuedPacket};
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, PlayerSessionCounter, SessionPlacement,
};
use mcrs_minecraft_network::OutboundReceiver;
use mcrs_minecraft_network::RawConnection;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockEvent;
use mcrs_voxel::BlockPos;
use tokio::sync::mpsc;

pub fn build_bridge_world() -> World {
    let mut world = World::new();
    MessageRegistry::register_message::<OutboundPlayerPacket>(&mut world);
    world.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
    world.init_resource::<mcrs_minecraft_engine::muxer::bridge::PendingConnections>();
    world.init_resource::<mcrs_minecraft_engine_world::session::SessionAnchors>();
    world
}

pub fn spawn_connection(world: &mut World) -> Entity {
    world.spawn(OutboundQueue::default()).id()
}

pub fn register_player(world: &mut World, socket: Entity, dim: Entity) -> (Entity, PlayerSession) {
    if !world.contains_resource::<PlayerSessionCounter>() {
        world.init_resource::<PlayerSessionCounter>();
    }
    let session = world.resource_mut::<PlayerSessionCounter>().next();
    let anchor = world
        .spawn(SessionBundle::placed(
            session,
            SessionPlacement::new(Place::InDim(dim), 0),
        ))
        .id();
    world.entity_mut(socket).insert(HostAnchorRef(anchor));
    (anchor, session)
}

pub fn write_packet(
    world: &mut World,
    target: PacketTarget,
    session: PlayerSession,
    epoch: u32,
    seq: u32,
) {
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target,
            data: marker(seq),
            session,
            epoch,
        });
}

pub fn write_packet_broadcast(world: &mut World, target: PacketTarget, seq: u32) {
    write_packet(world, target, PlayerSession(0), 0, seq);
}

pub fn write_packet_stamped(world: &mut World, session: PlayerSession, epoch: u32, seq: u32) {
    write_packet(
        world,
        PacketTarget::SinglePlayer(session),
        session,
        epoch,
        seq,
    );
}

pub fn register_session(world: &mut World, session: PlayerSession, socket: Entity, epoch: u32) {
    let dim = Entity::from_raw_u32(9998).expect("nonzero");
    let anchor = world
        .spawn(SessionBundle::placed(
            session,
            SessionPlacement::new(Place::InDim(dim), epoch),
        ))
        .id();
    world.entity_mut(socket).insert(HostAnchorRef(anchor));
}

pub fn build_bridge_world_with_sessions() -> World {
    build_bridge_world()
}

pub fn run_system<S, Marker>(world: &mut World, system: S)
where
    S: IntoSystem<(), (), Marker>,
{
    let mut sys = IntoSystem::into_system(system);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

pub fn drain_queue(world: &mut World, socket: Entity) -> Vec<QueuedPacket> {
    let mut q = world
        .get_mut::<OutboundQueue>(socket)
        .expect("OutboundQueue present");
    q.drain().collect()
}

pub fn marker(seq: u32) -> PacketPayload {
    PacketPayload::BlockEvent(ClientboundBlockEvent {
        pos: BlockPos::new(0, 0, 0),
        action: 0,
        param: 0,
        block: VarInt(seq as i32),
    })
}

pub fn marker_seq(payload: &PacketPayload) -> Option<u32> {
    match payload {
        PacketPayload::BlockEvent(packet) => Some(packet.block.0 as u32),
        _ => None,
    }
}

pub fn make_mock_raw_connection() -> (RawConnection, OutboundReceiver) {
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

    let (raw, outgoing_rx, inbound_tx) =
        rt.block_on(async { RawConnection::new_for_test_full(16) });
    rt.spawn(async move {
        let _keep = inbound_tx;
        std::future::pending::<()>().await;
    });
    (raw, outgoing_rx)
}

pub fn make_mock_raw_connection_full() -> (
    RawConnection,
    OutboundReceiver,
    mpsc::Sender<mcrs_minecraft_network::ReceivedPacket>,
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
