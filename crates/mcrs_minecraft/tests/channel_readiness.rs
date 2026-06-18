use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use bytes::Bytes;
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::snapshot::RegistrySnapshot;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::session::PlayerSession;
use mcrs_engine::world::sub_app::{DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerDisconnect,
    OutboundPlayerPacket, OutboundPlayerTransfer,
};
use mcrs_minecraft::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft::world::player_index::{PendingInboundBuffer, PlayerIndex};
use mcrs_minecraft::world::sub_app_builder::{drain_dim_spawn_queue, DimSubAppHandle};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;

#[derive(Resource, Default)]
struct InboundLog(Vec<i32>);

fn make_stub_block_light_table() -> BlockStateLightTable {
    let state_count = 2usize;
    let emission = vec![0u8; state_count].into_boxed_slice();
    let dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let flags = vec![0u8; state_count].into_boxed_slice();
    BlockStateLightTable { emission, dampening, occlusion, flags }
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
    app.init_resource::<mcrs_engine::session::SessionRegistry>();
    app.init_resource::<mcrs_engine::session::PlayerSessionCounter>();
    app.init_resource::<PendingInboundBuffer>();
    app.init_resource::<DimChannelsResource>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app
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
fn messages_buffered_before_dim_boots() {
    let mut app = build_app();

    // Transition to Playing so sub-apps can be spawned.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();

    // Enqueue and spawn the dim. spawn_dim_subapp creates the channel pair
    // before the sub-app's schedule first runs.
    use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:readiness"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
    drain_dim_spawn_queue(&mut app);

    // Resolve the label entity for the newly spawned dim.
    let label_entity = {
        let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
        let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
        assert_eq!(handles.len(), 1, "expected exactly one DimSubAppHandle entity");
        handles[0]
    };

    // Install the recorder in the sub-app BEFORE its first tick.
    {
        let sub = app.sub_app_mut(DimAppLabel(label_entity));
        sub.world_mut().init_resource::<InboundLog>();
        sub.add_systems(Update, record_sub_inbound);
    }

    let anchor = Entity::from_raw_u32(5).expect("nonzero");

    // Send messages into the channel BEFORE app.update() runs the sub-app.
    // All sends must return Ok without blocking — the bounded channel buffer
    // is the pending queue (structural readiness).
    let send_results: Vec<Result<(), _>> = {
        let channels = app.world().resource::<DimChannelsResource>();
        let entry = channels.get(label_entity).expect("channel entry exists before first tick");

        let spawn_result = entry.control_sender.try_send(ToDim::Spawn {
            host_anchor: anchor,
            session: PlayerSession(1),
            snapshot: mcrs_minecraft::world::bus::PlayerTransferSnapshot {
                uuid: mcrs_protocol::uuid::Uuid::nil(),
                username: "readiness_player".into(),
                position: bevy_math::DVec3::ZERO,
                rotation: bevy_math::Vec2::ZERO,
            },
        });

        let serverbound_results: Vec<_> = (200i32..203)
            .map(|id| {
                entry.serverbound_sender.try_send(ToDim::Serverbound {
                    player: anchor,
                    id,
                    data: Bytes::new(),
                    timestamp: std::time::Instant::now(),
                })
            })
            .collect();

        std::iter::once(spawn_result)
            .chain(serverbound_results)
            .collect()
    };

    for (i, result) in send_results.iter().enumerate() {
        assert!(
            result.is_ok(),
            "send {i} must return Ok without blocking before the dim's first tick"
        );
    }

    // First app.update() — sub-app runs for the first time:
    // FixedPreUpdate drain_to_dim_inbox drains the channel in FIFO order,
    // control (Spawn) before serverbound; Update record_sub_inbound logs
    // the serverbound packets (Spawn routes to InboundPlayerSpawn, not
    // InboundPlayerPacket, so it does not appear in InboundLog).
    app.update();

    let log = app
        .sub_app(DimAppLabel(label_entity))
        .world()
        .resource::<InboundLog>()
        .0
        .clone();

    // The three serverbound ids must arrive in send order.
    assert_eq!(
        log,
        vec![200, 201, 202],
        "buffered serverbound messages must be delivered FIFO on the first dim tick"
    );
}
