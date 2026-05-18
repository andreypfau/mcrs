//! End-to-end bus integration test running against the production
//! `spawn_dim_subapp` builder. Validates the spike's tick-ordering
//! invariants (0-tick inbound, 1-tick outbound) hold when the bus is
//! wired with the real `WorldPlugin`-style host registrations and the
//! merged extract closure from `sub_app_builder.rs`.

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::sub_app::{DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::world::bridge::partition_main_inbound;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer, PacketPayload,
    PacketPriority, PacketTarget, PendingInboundPartition, TestInboundPayload, TestPayload,
};
use mcrs_minecraft::world::player_index::{PlayerIndex, PlayerLocation};
use mcrs_minecraft::world::sub_app_builder::{
    drain_dim_spawn_queue, DimSubAppHandle,
};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;
use smallvec::SmallVec;

#[derive(Resource, Default)]
struct OutboundLog(Vec<u32>);

#[derive(Resource, Default)]
struct InboundLog(Vec<u32>);

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
    static SET_ASSET_ROOT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    SET_ASSET_ROOT.get_or_init(|| {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("CARGO_MANIFEST_DIR must have two ancestors (workspace root)");
        // SAFETY: set_var is safe before any thread reads the value;
        // the OnceLock guard guarantees a single mutation.
        unsafe {
            std::env::set_var("BEVY_ASSET_ROOT", workspace_root);
        }
    });

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

    // Host-side bus substrate (mirrors what `WorldPlugin::build` installs).
    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundPartition>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.add_systems(Update, partition_main_inbound);

    app
}

fn drive_to_playing_and_spawn_subapps(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    // `enqueue_dim_spawns_from_preset` requires `LoadedWorldPreset`. The
    // tests here use an explicit `DimSpawnRequest` instead — push it
    // directly, then drain.
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
        log.0.push(msg.packet.seq);
    }
}

#[test]
fn outbound_latency_is_one_host_tick_in_production_app() {
    let mut app = build_app();

    // Host-side consumer that records outbound seq values. Must be in
    // Update so it runs each `app.update()`.
    app.init_resource::<OutboundLog>();
    app.add_systems(Update, record_host_outbound);

    enqueue_overworld(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    let label_entity = first_label_entity(&mut app);

    // Inject one outbound packet into the sub-app's Messages buffer.
    app.sub_app_mut(DimAppLabel(label_entity))
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(Entity::PLACEHOLDER),
            priority: PacketPriority::Normal,
            data: PacketPayload::Test(TestPayload { seq: 0xDEAD }),
        });

    // Tick 1: main runs first (host log still empty), then sub extract
    // drains the sub-app's Messages<Outbound> into main.Messages<Outbound>.
    app.update();
    let log = app.world().resource::<OutboundLog>().0.clone();
    assert!(
        log.is_empty(),
        "outbound should NOT yet be visible to host after tick 1; log = {log:?}"
    );

    // Tick 2: main runs again; record_host_outbound now drains the
    // message that was extracted at the end of tick 1.
    app.update();
    let log = app.world().resource::<OutboundLog>().0.clone();
    assert_eq!(
        log,
        vec![0xDEAD],
        "outbound visible to host after tick 2 (1 host-tick latency)"
    );
}

#[test]
fn inbound_latency_is_zero_host_ticks_via_player_index() {
    let mut app = build_app();

    enqueue_overworld(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    let label_entity = first_label_entity(&mut app);

    // Add a sub-side recorder in the sub-app's Update schedule.
    {
        let sub = app.sub_app_mut(DimAppLabel(label_entity));
        sub.world_mut().init_resource::<InboundLog>();
        sub.add_systems(Update, record_sub_inbound);
    }

    // Place the player in PlayerIndex with the sub-app's label_entity as
    // its current_dim and a non-None in_dim_entity so partition_main_inbound
    // routes to PendingInboundPartition.per_dim (not inbound_pending).
    let player = Entity::from_raw_u32(42).expect("nonzero");
    let in_dim = Entity::from_raw_u32(99).expect("nonzero");
    app.world_mut().resource_mut::<PlayerIndex>().insert(
        player,
        PlayerLocation {
            socket: Entity::PLACEHOLDER,
            current_dim: label_entity,
            in_dim_entity: Some(in_dim),
            inbound_pending: SmallVec::new(),
        },
    );

    // Inject inbound packet on main.
    app.world_mut()
        .resource_mut::<Messages<InboundPlayerPacket>>()
        .write(InboundPlayerPacket {
            player,
            packet: TestInboundPayload { seq: 0xCAFE },
        });

    // Tick 1: main partition_main_inbound routes to the per_dim bucket,
    // sub extract drains the bucket into sub.Messages<Inbound>, sub.update
    // runs record_sub_inbound which logs the packet.
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
