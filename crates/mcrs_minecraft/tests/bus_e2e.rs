use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::snapshot::RegistrySnapshot;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::sub_app::{DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_engine::session::PlayerSession;
use mcrs_minecraft::world::bridge::bridge_inbound_to_channel;
use bytes::Bytes;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, PacketPayload,
    PacketPriority, PacketTarget, TestPayload,
};
use mcrs_minecraft::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_engine::session::{PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_minecraft::world::player_index::{PendingInboundBuffer, PlayerIndex};
use mcrs_minecraft::world::sub_app_builder::{
    drain_dim_spawn_queue, DimSubAppHandle,
};
use mcrs_minecraft::runner::pump_channels;
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;

#[derive(Resource, Default)]
struct OutboundLog(Vec<u32>);

#[derive(Resource, Default)]
struct InboundLog(Vec<i32>);

fn make_stub_block_light_table() -> BlockStateLightTable {
    let state_count = 2usize;
    let emission = vec![0u8; state_count].into_boxed_slice();
    let dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let flags = vec![0u8; state_count].into_boxed_slice();
    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(make_stub_block_light_table());
    app.insert_resource(StaticRegistry::<Block>::new());
    app.insert_resource(StaticRegistry::<EnchantmentData>::default());
    app.insert_resource(TagRegistry::<Block>::default());
    app.insert_resource(RegistrySnapshot::<Biome>::default());

    app.init_resource::<PlayerIndex>();
    app.init_resource::<SessionRegistry>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<PendingInboundBuffer>();
    app.init_resource::<DimChannelsResource>();
    app.init_resource::<mcrs_engine::world::in_flight::InFlightMoves>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.add_systems(Update, bridge_inbound_to_channel);

    app
}

fn drive_to_playing_and_spawn_subapps(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    drain_dim_spawn_queue(app);
}

fn enqueue_overworld(app: &mut App) {
    use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
}

fn first_label_entity(app: &mut App) -> Entity {
    let mut q = app
        .world_mut()
        .query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
    assert_eq!(
        handles.len(),
        1,
        "expected exactly one DimSubAppHandle entity"
    );
    handles[0]
}

fn record_host_outbound(
    mut msgs: ResMut<Messages<OutboundPlayerPacket>>,
    mut log: ResMut<OutboundLog>,
) {
    for msg in msgs.drain() {
        let seq = match msg.data {
            PacketPayload::Test(TestPayload { seq }) => seq,
            _ => continue,
        };
        log.0.push(seq);
    }
}

fn record_sub_inbound(
    mut msgs: ResMut<Messages<InboundPlayerPacket>>,
    mut log: ResMut<InboundLog>,
) {
    for msg in msgs.drain() {
        log.0.push(msg.id);
    }
}

#[test]
fn outbound_latency_is_one_host_tick() {
    let mut app = build_app();

    app.init_resource::<OutboundLog>();
    app.add_systems(Update, record_host_outbound);

    enqueue_overworld(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    let label_entity = first_label_entity(&mut app);

    app.sub_app_mut(DimAppLabel(label_entity))
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(Entity::PLACEHOLDER),
            priority: PacketPriority::Normal,
            data: PacketPayload::Test(TestPayload { seq: 0xDEAD }),
            session: PlayerSession(0),
            epoch: 0,
        });

    // Tick 1: flush_from_dim_outbox sends to channel; pump_channels drains it.
    // record_host_outbound runs in Update which precedes the pump, so the packet
    // is not yet in the host Messages buffer when Update executes.
    app.update();
    pump_channels(&mut app);
    let log = app.world().resource::<OutboundLog>().0.clone();
    assert!(
        log.is_empty(),
        "outbound should NOT yet be visible to host after tick 1; log = {log:?}"
    );

    // Tick 2: host Messages buffers swap; record_host_outbound drains the packet
    // that pump_channels wrote during tick 1.
    app.update();
    pump_channels(&mut app);
    let log = app.world().resource::<OutboundLog>().0.clone();
    assert_eq!(
        log,
        vec![0xDEAD],
        "outbound visible to host after tick 2 (1 host-tick latency)"
    );
}

#[test]
fn inbound_latency_is_zero_host_ticks() {
    let mut app = build_app();

    enqueue_overworld(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    let label_entity = first_label_entity(&mut app);

    {
        let sub = app.sub_app_mut(DimAppLabel(label_entity));
        sub.world_mut().init_resource::<InboundLog>();
        sub.add_systems(Update, record_sub_inbound);
    }

    let host_anchor = Entity::from_raw_u32(42).expect("nonzero");
    let player = host_anchor;
    let in_dim = Entity::from_raw_u32(99).expect("nonzero");
    let session = app.world_mut().resource_mut::<PlayerSessionCounter>().next();
    app.world_mut().resource_mut::<SessionRegistry>().insert(
        session,
        SessionEntry {
            connection_entity: Entity::PLACEHOLDER,
            host_anchor,
            dim: label_entity,
            previous_dim: None,
            in_dim_entity: Some(in_dim),
            epoch: 0,
        },
    );

    app.world_mut()
        .resource_mut::<Messages<InboundPlayerPacket>>()
        .write(InboundPlayerPacket {
            player,
            id: 0xCAFE,
            data: Bytes::new(),
            timestamp: std::time::Instant::now(),
        });

    // bridge_inbound_to_channel (Update) routes to the dim's serverbound channel;
    // drain_to_dim_inbox (FixedPreUpdate) reads it into sub Messages;
    // record_sub_inbound (Update) drains it — all within one app.update().
    app.update();

    let log = app
        .sub_app(DimAppLabel(label_entity))
        .world()
        .resource::<InboundLog>()
        .0
        .clone();
    assert_eq!(
        log,
        vec![0xCAFE],
        "inbound visible inside sub-app after tick 1 (0 host-tick latency)"
    );
}

#[test]
fn channel_pair_created_on_dim_spawn() {
    let mut app = build_app();

    enqueue_overworld(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    let label_entity = first_label_entity(&mut app);

    // The channel entry keyed by the dim's label_entity must exist.
    let channels = app.world().resource::<DimChannelsResource>();
    assert!(
        channels.get(label_entity).is_some(),
        "DimChannels must have an entry for the spawned dim's label_entity (one bounded pair per dim)"
    );
}

#[test]
fn fifo_ordering_preserved() {
    let mut app = build_app();

    enqueue_overworld(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    let label_entity = first_label_entity(&mut app);

    {
        let sub = app.sub_app_mut(DimAppLabel(label_entity));
        sub.world_mut().init_resource::<InboundLog>();
        sub.add_systems(Update, record_sub_inbound);
    }

    let host_anchor = Entity::from_raw_u32(7).expect("nonzero");
    let in_dim = Entity::from_raw_u32(8).expect("nonzero");
    let session = app.world_mut().resource_mut::<PlayerSessionCounter>().next();
    app.world_mut().resource_mut::<SessionRegistry>().insert(
        session,
        SessionEntry {
            connection_entity: Entity::PLACEHOLDER,
            host_anchor,
            dim: label_entity,
            previous_dim: None,
            in_dim_entity: Some(in_dim),
            epoch: 0,
        },
    );

    // Send N messages in a known order via the host-side channel sender directly.
    let send_order: Vec<i32> = (100..105).collect();
    {
        let channels = app.world().resource::<DimChannelsResource>();
        let entry = channels.get(label_entity).expect("channel entry exists");
        for &id in &send_order {
            entry
                .serverbound_sender
                .try_send(ToDim::Serverbound {
                    player: host_anchor,
                    id,
                    data: Bytes::new(),
                    timestamp: std::time::Instant::now(),
                })
                .expect("send succeeds");
        }
    }

    app.update();

    let log = app
        .sub_app(DimAppLabel(label_entity))
        .world()
        .resource::<InboundLog>()
        .0
        .clone();
    assert_eq!(
        log, send_order,
        "per-channel FIFO: messages must be delivered in send order"
    );
}
