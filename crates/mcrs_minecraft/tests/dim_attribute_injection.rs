//! Asserts that dim-span pairs fire for each spawned dimension and that
//! descendant `lighting::*` spans inherit the `dim` field through the
//! tracing parent chain.
//!
//! Tests in this file compile immediately but FAIL until the dim-span
//! injection is wired in the production SubApp builder — assertions will
//! report "expected dim_extract span for test:overworld but found 0"
//! rather than passing vacuously. That failure mode is intentional.
//!
//! Fixture shape mirrors bus_e2e.rs: `build_app` + `enqueue_*` helpers +
//! `drive_to_playing_and_spawn_subapps` using the production
//! `drain_dim_spawn_queue` entry-point.
//!
//! Parent-chain note: Bevy per-system spans use `parent: None` and do NOT
//! inherit `dim`. Only project-controlled `#[instrument]` spans inherit `dim`
//! through the tracing parent chain. The parent-chain assertion targets
//! `lighting::*` descendants, not `"system"` spans.

#![cfg(feature = "telemetry-tracy")]

mod common;

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::AppState;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig, InDimension};
use mcrs_engine::world::sub_app::{DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::world::bridge::partition_main_inbound;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer,
    PendingInboundLifecycle, PendingInboundPartition,
};
use mcrs_minecraft::world::player_index::PlayerIndex;
use mcrs_minecraft::world::sub_app_builder::{drain_dim_spawn_queue, DimSubAppHandle};
use mcrs_minecraft_lighting::test_bench::bench_helpers;
use vanilla::block::Block;
use vanilla::enchantment::EnchantmentData;

#[allow(unused_imports)]
use mcrs_vanilla as vanilla;

fn make_stub_block_light_table() -> mcrs_minecraft_lighting::table::BlockStateLightTable {
    bench_helpers::make_stub_block_light_table_with_torch()
}

fn build_app() -> App {
    static SET_ASSET_ROOT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    SET_ASSET_ROOT.get_or_init(|| {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("CARGO_MANIFEST_DIR must have two ancestors (workspace root)");
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

    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundPartition>();
    app.init_resource::<PendingInboundLifecycle>();
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

/// Use a single-section dimension so the heightmap scan finalises in tick 1.
/// A multi-section dimension with only one chunk loaded leaves the top section
/// absent, the scan returns early every tick, and lighting BFS work never starts.
fn single_section_type_config() -> DimensionTypeConfig {
    DimensionTypeConfig::new(0, 16)
}

fn enqueue_overworld(app: &mut App) {
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:overworld"),
            type_config: single_section_type_config(),
            has_sky: true,
        });
}

fn enqueue_the_nether(app: &mut App) {
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:the_nether"),
            type_config: single_section_type_config(),
            has_sky: false,
        });
}

fn drive_to_playing_and_spawn_subapps(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    drain_dim_spawn_queue(app);
}

/// Collect all DimSubAppHandle label entities from the main world.
fn label_entities(app: &mut App) -> Vec<bevy_ecs::entity::Entity> {
    let mut q = app.world_mut().query::<(bevy_ecs::entity::Entity, &DimSubAppHandle)>();
    q.iter(app.world()).map(|(e, _)| e).collect()
}

/// Seed a torch chunk in a SubApp world so the lighting lifecycle runs on the
/// next tick under the dim_tick span. The SubApp world has its own isolated
/// entity allocator; chunk entities created here are not visible in the main
/// world and vice-versa.
fn seed_chunk_in_subapp(app: &mut App, label: bevy_ecs::entity::Entity) {
    let sub = app.sub_app_mut(DimAppLabel(label));
    let sub_world = sub.world_mut();

    // Find the dimension entity in the SubApp world.
    let dim_entity = sub_world
        .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::prelude::With<mcrs_engine::world::dimension::DimensionTypeConfig>>()
        .iter(sub_world)
        .next()
        .expect("SubApp world must have a Dimension entity with DimensionTypeConfig");

    let palette = bench_helpers::torch_palette_with_one_emitter();
    sub_world.spawn((
        InDimension(dim_entity),
        ChunkPos::new(0, 0, 0),
        ChunkEntities::default(),
        Chunk,
        ChunkLoaded,
        palette,
    ));
}

/// Asserts that dim-span pairs (`dim_extract` / `dim_tick`) fire for each
/// spawned dimension, and that descendant `lighting::*` spans carry the
/// `dim` field through the tracing parent chain.
///
/// Uses the production `drain_dim_spawn_queue` entry-point — NOT a custom
/// `set_extract` call — to ensure the bus shuttle closure is preserved
/// inside the extract wrapper.
///
/// The parent-chain assertion targets `lighting::*` spans rather than
/// `"system"` spans because Bevy emits per-system spans with `parent: None`,
/// which breaks the tracing inheritance chain. Project-controlled
/// `#[instrument]` spans (like `lighting::*`) inherit `dim` normally.
#[test]
fn dim_span_pair_fires_and_lighting_inherits_dim_field() {
    common::install_global_capture();
    let (_guard, buffer) = common::lock_and_clear();

    let mut app = build_app();
    enqueue_overworld(&mut app);
    enqueue_the_nether(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    // Seed a torch chunk into each SubApp world so lighting BFS work is
    // queued on tick 1. With no loaded chunks the BFS never starts and
    // no lighting::* spans are produced.
    let labels = label_entities(&mut app);
    for label in &labels {
        seed_chunk_in_subapp(&mut app, *label);
    }

    // Pump enough ticks for: tick 1 — column lifecycle (reconcile → heightmap
    // scan → seed → BFS pending), ticks 2-3 — BFS propagate + distribute.
    for _ in 0..4 {
        app.update();
    }

    let captured = buffer.lock().unwrap();

    // (a) dim_extract fires for test:overworld
    assert!(
        captured
            .iter()
            .any(|s| s.name == "dim_extract"
                && s.fields.get("dim").map(|d| d.as_str()) == Some("test:overworld")),
        "expected dim_extract span with dim = \"test:overworld\" but found 0 emissions"
    );

    // (b) dim_tick fires for test:overworld
    assert!(
        captured
            .iter()
            .any(|s| s.name == "dim_tick"
                && s.fields.get("dim").map(|d| d.as_str()) == Some("test:overworld")),
        "expected dim_tick span with dim = \"test:overworld\" but found 0 emissions"
    );

    // (c) dim_extract fires for test:the_nether
    assert!(
        captured
            .iter()
            .any(|s| s.name == "dim_extract"
                && s.fields.get("dim").map(|d| d.as_str()) == Some("test:the_nether")),
        "expected dim_extract span with dim = \"test:the_nether\" but found 0 emissions"
    );

    // (d) dim_tick fires for test:the_nether
    assert!(
        captured
            .iter()
            .any(|s| s.name == "dim_tick"
                && s.fields.get("dim").map(|d| d.as_str()) == Some("test:the_nether")),
        "expected dim_tick span with dim = \"test:the_nether\" but found 0 emissions"
    );

    // (e) At least one lighting::* span captured during an overworld pump
    //     has parent_dim == "test:overworld" (parent-chain inheritance).
    assert!(
        captured
            .iter()
            .any(|s| s.name.starts_with("lighting::")
                && s.parent_dim.as_deref() == Some("test:overworld")),
        "expected at least one lighting::* span with parent dim = \"test:overworld\" \
         but found 0; note: only #[instrument] spans inherit dim, not Bevy system spans"
    );

    // (f) At least one lighting::* span with parent_dim == "test:the_nether".
    assert!(
        captured
            .iter()
            .any(|s| s.name.starts_with("lighting::")
                && s.parent_dim.as_deref() == Some("test:the_nether")),
        "expected at least one lighting::* span with parent dim = \"test:the_nether\" \
         but found 0; note: only #[instrument] spans inherit dim, not Bevy system spans"
    );
}
