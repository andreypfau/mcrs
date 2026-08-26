//! The main app owns clock advancement; every dimension sub-world sees the
//! same tick in the same update and cannot run a clock of its own.

use bevy_app::{App, FixedUpdate, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_core::registry::access::RegistryAccess;
use mcrs_minecraft_core::registry::snapshot::RegistrySnapshot;
use mcrs_minecraft_core::registry::static_registry::StaticRegistry;
use mcrs_minecraft_core::tag::registry::DynTagRegistry;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket,
};
use mcrs_minecraft::world::channel_types::DimChannelsResource;
use mcrs_minecraft::world::player_index::{PendingInboundBuffer, PlayerIndex};
use mcrs_minecraft::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;
use mcrs_vanilla::world_clock::{ClockState, WorldClockPlugin, WorldClocks};
use mcrs_voxel_light::table::BlockStateLightTable;
use mcrs_voxel_math::voxel_shape::VoxelShape;
use mcrs_voxel_world::world::dimension::{DimensionId, DimensionTypeConfig};
use mcrs_voxel_world::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};

mod support;

const OVERWORLD: &str = "minecraft:overworld";

fn make_stub_block_light_table() -> BlockStateLightTable {
    let state_count = 2usize;
    BlockStateLightTable {
        emission: vec![0u8; state_count].into_boxed_slice(),
        dampening: vec![0u8; state_count].into_boxed_slice(),
        occlusion: vec![VoxelShape::empty(); state_count].into_boxed_slice(),
        flags: vec![0u8; state_count].into_boxed_slice(),
    }
}

fn build_host_app() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(make_stub_block_light_table());
    app.insert_resource(StaticRegistry::<EnchantmentData>::default());
    app.insert_resource(DynTagRegistry::<Block>::default());
    app.insert_resource(RegistrySnapshot::<Biome>::default());
    app.insert_resource(support::corpus(&app));
    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundBuffer>();
    app.init_resource::<DimChannelsResource>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app.add_plugins(WorldClockPlugin);
    let mut clocks = WorldClocks::default();
    clocks.reconcile_with_registry([mcrs_minecraft_core::ResourceLocation::parse(OVERWORLD).unwrap()]);
    app.insert_resource(clocks);

    app
}

fn spawn_dim(app: &mut App, id: &str) -> Entity {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new(id),
            type_config: DimensionTypeConfig::new(-64, 384),
            has_sky: true,
        });
    drain_dim_spawn_queue(app);

    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    q.iter(app.world())
        .map(|(e, _)| e)
        .next()
        .expect("one DimSubAppHandle")
}

fn sub_app_clock(app: &mut App, dim_label: Entity) -> ClockState {
    *app.sub_app_mut(DimAppLabel(dim_label))
        .world()
        .resource::<WorldClocks>()
        .get(OVERWORLD)
        .expect("the extract carried the overworld clock into the sub-world")
}

fn main_clock(app: &App) -> ClockState {
    *app.world()
        .resource::<WorldClocks>()
        .get(OVERWORLD)
        .unwrap()
}

#[test]
fn a_dimension_sub_app_observes_the_main_app_tick() {
    let mut app = build_host_app();
    let dim_label = spawn_dim(&mut app, "test:overworld");

    let before = main_clock(&app).total_ticks;
    for _ in 0..7 {
        app.world_mut().run_schedule(FixedUpdate);
    }
    app.update();

    assert!(main_clock(&app).total_ticks >= before + 7);
    assert_eq!(sub_app_clock(&mut app, dim_label), main_clock(&app));

    app.world_mut().run_schedule(FixedUpdate);
    app.update();
    assert_eq!(sub_app_clock(&mut app, dim_label), main_clock(&app));
}

#[test]
fn a_sub_world_cannot_keep_a_clock_of_its_own() {
    let mut app = build_host_app();
    let dim_label = spawn_dim(&mut app, "test:overworld");

    app.world_mut().run_schedule(FixedUpdate);
    app.update();

    app.sub_app_mut(DimAppLabel(dim_label))
        .world_mut()
        .resource_mut::<WorldClocks>()
        .get_mut(OVERWORLD)
        .unwrap()
        .total_ticks = 999_999;

    app.update();

    assert_eq!(sub_app_clock(&mut app, dim_label), main_clock(&app));
    assert_ne!(main_clock(&app).total_ticks, 999_999);
}
