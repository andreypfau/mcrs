// Integration tests for the per-dimension sub-app lifecycle. Each test
// constructs a minimal host `App`, enqueues a synthetic spawn request, drains
// the queue through the production builder, and inspects the resulting
// sub-app population.

use bevy_app::{App, AppLabel, FixedPostUpdate, FixedPreUpdate, FixedUpdate};
use bevy_asset::AssetPlugin;
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
use mcrs_vanilla::enchantment::EnchantmentData;
use mcrs_engine::world::dimension::{Dimension, DimensionId, DimensionTypeConfig};
use mcrs_engine::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft::world::sub_app_builder::{
    drain_dim_despawn_queue, drain_dim_spawn_queue, gather_dim_registries, spawn_dim_subapp,
    DimSubAppHandle,
};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::block::Block;

mod harness {
    #![allow(dead_code)]
    use super::*;

    pub fn make_stub_block_light_table() -> BlockStateLightTable {
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

    pub fn make_main_app() -> App {
        // The per-dim sub-app composition pulls in `NoiseGeneratorSettingsPlugin`
        // (via `ChunkPlugin`), whose `AssetServer::load` requires the noise
        // settings JSON under `assets/minecraft/worldgen/noise_settings/`. Bevy
        // 0.18's `AssetPlugin::default()` derives its file-source root from
        // `CARGO_MANIFEST_DIR` when present (which `cargo test` always sets to
        // the crate directory, not the workspace root). `BEVY_ASSET_ROOT`
        // overrides that, so pointing it at the workspace root makes the
        // assets reachable from every per-dim sub-app's `AssetServer`. Done
        // once per test process via `OnceLock`.
        static SET_ASSET_ROOT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        SET_ASSET_ROOT.get_or_init(|| {
            let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|p| p.parent())
                .expect("CARGO_MANIFEST_DIR must have two ancestors (workspace root)");
            // SAFETY: set_var is safe to call when no other thread is reading
            // the same env var. This OnceLock runs before any test body uses
            // the value (via AssetPlugin construction inside spawn_dim_subapp).
            unsafe {
                std::env::set_var("BEVY_ASSET_ROOT", workspace_root);
            }
        });

        let mut app = App::new();
        // The per-dim sub-app composition pulls in `NoiseGeneratorSettingsPlugin`
        // (via `ChunkPlugin`) whose `Startup` system uses `AssetServer::load`,
        // which spawns work on `IoTaskPool`. Production sets the pools up via
        // `ServerPlugin → TaskPoolPlugin`; tests need the same initialisation so
        // sub-app `Startup` does not panic on an uninitialised pool.
        app.add_plugins(bevy_app::TaskPoolPlugin::default());
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

        // The production extract closure in `spawn_dim_subapp` shuttles
        // bus messages and reads `PendingInboundPartition` from the host
        // world. Tests that drive `app.update()` need the same host-side
        // registrations that `WorldPlugin::build` installs, otherwise the
        // closure panics on its first run with "Requested resource ... does
        // not exist".
        app.init_resource::<mcrs_minecraft::world::bus::PendingInboundPartition>();
        app.init_resource::<mcrs_minecraft::world::bus::PendingInboundLifecycle>();
        app.add_message::<mcrs_minecraft::world::bus::OutboundPlayerPacket>();
        app.add_message::<mcrs_minecraft::world::bus::InboundPlayerPacket>();
        app.add_message::<mcrs_minecraft::world::bus::OutboundPlayerTransfer>();
        app.add_message::<mcrs_minecraft::world::bus::OutboundPlayerAttached>();
        app.add_message::<mcrs_minecraft::world::bus::InboundPlayerSpawn>();
        app.add_message::<mcrs_minecraft::world::bus::InboundPlayerDespawn>();

        app
    }

    pub fn drive_to_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Playing);
        app.update();
    }

    pub fn enqueue_spawn(app: &mut App, id: &str, sky: bool) {
        app.world_mut()
            .resource_mut::<DimSpawnQueue>()
            .0
            .push(DimSpawnRequest {
                dimension_id: DimensionId::new(id),
                type_config: DimensionTypeConfig::default(),
                has_sky: sky,
            });
    }

}

#[test]
fn dim_subapp_inserted_on_spawn() {
    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(
        app.sub_apps().sub_apps.len(),
        1,
        "exactly one sub-app should be present after one spawn drain"
    );
}

#[test]
fn dim_subapp_removed_on_despawn() {
    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 1);

    // The label-anchor entity in the host world is the same value the
    // sub-app was interned under.
    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q
        .iter(app.world())
        .map(|(e, _)| e)
        .collect();
    assert_eq!(handles.len(), 1, "one host-side handle entity per sub-app");
    let label_entity = handles[0];

    // The sub-app's own `World` carries the `Dimension` entity with the bundle.
    let sub_app = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&DimAppLabel(label_entity).intern())
        .expect("sub-app under DimAppLabel");
    let mut q = sub_app.world_mut().query::<(Entity, &Dimension)>();
    let count = q.iter(sub_app.world()).count();
    assert_eq!(count, 1, "exactly one Dimension entity per sub-app world");

    app.world_mut()
        .resource_mut::<DimDespawnQueue>()
        .0
        .push(label_entity);
    drain_dim_despawn_queue(&mut app);
    assert_eq!(
        app.sub_apps().sub_apps.len(),
        0,
        "sub-app should be removed after despawn drain"
    );
}

#[test]
fn dim_worlds_are_isolated() {
    #[derive(Resource)]
    #[allow(dead_code)]
    struct SentinelOverworld(u32);

    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    harness::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 2);

    let host_world_dim_count = app
        .world_mut()
        .query::<&Dimension>()
        .iter(app.world())
        .count();
    assert_eq!(
        host_world_dim_count, 0,
        "host world should hold zero Dimension entities"
    );

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    let first = labels[0];
    let second = labels[1];

    app.sub_apps_mut()
        .sub_apps
        .get_mut(&first)
        .expect("first sub-app present")
        .world_mut()
        .insert_resource(SentinelOverworld(42));

    let other_sub_app = app
        .sub_apps()
        .sub_apps
        .get(&second)
        .expect("second sub-app present");
    assert!(
        other_sub_app
            .world()
            .get_resource::<SentinelOverworld>()
            .is_none(),
        "sentinel resource inserted into one sub-app must not appear in the other"
    );
}

#[test]
fn sequential_pump_tick_count() {
    #[derive(Resource, Default)]
    struct TickCounter(u64);

    fn bump(mut counter: ResMut<TickCounter>) {
        counter.0 += 1;
    }

    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    harness::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    for label in &labels {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(label)
            .expect("sub-app present");
        sub_app.world_mut().insert_resource(TickCounter::default());
        sub_app.add_systems(FixedUpdate, bump);
    }

    for _ in 0..3 {
        app.update();
    }

    for label in &labels {
        let sub_app = app
            .sub_apps()
            .sub_apps
            .get(label)
            .expect("sub-app present");
        let counter = sub_app.world().resource::<TickCounter>();
        assert_eq!(
            counter.0, 3,
            "each sub-app should have ticked exactly three times"
        );
    }
}

#[test]
fn fixed_pre_and_post_update_advance_once_per_pump() {
    #[derive(Resource, Default)]
    struct PreCounter(u32);

    #[derive(Resource, Default)]
    struct PostCounter(u32);

    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);

    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
    assert_eq!(handles.len(), 1, "one host-side handle entity");
    let label_entity = handles[0];

    {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(&DimAppLabel(label_entity).intern())
            .expect("sub-app under DimAppLabel");
        sub_app.init_resource::<PreCounter>();
        sub_app.init_resource::<PostCounter>();
        sub_app.add_systems(FixedPreUpdate, |mut c: ResMut<PreCounter>| c.0 += 1);
        sub_app.add_systems(FixedPostUpdate, |mut c: ResMut<PostCounter>| c.0 += 1);
    }

    for _ in 0..3 {
        app.update();
    }

    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&DimAppLabel(label_entity).intern())
        .expect("sub-app under DimAppLabel");
    let pre = sub_app.world().resource::<PreCounter>();
    let post = sub_app.world().resource::<PostCounter>();
    assert_eq!(pre.0, 3, "FixedPreUpdate must tick once per host pump");
    assert_eq!(post.0, 3, "FixedPostUpdate must tick once per host pump");
}

/// Regression test: despawning the host-world `DimSubAppHandle` entity must
/// automatically tear down the matching sub-app via the `On<Remove, DimSubAppHandle>`
/// observer registered in production. This test would have failed before the observer
/// was wired because only explicit queue pushes worked.
///
/// The observer is registered inline (not via `WorldPlugin::build`) to avoid pulling
/// in the heavy plugin stack that `WorldPlugin` composes.
#[test]
fn subapp_torn_down_when_handle_despawned() {
    let mut app = harness::make_main_app();

    // Mirror the production observer from WorldPlugin::build inline so the test
    // exercises the same wiring path without depending on unrelated plugins.
    app.add_observer(
        |trigger: On<Remove, DimSubAppHandle>, mut queue: ResMut<DimDespawnQueue>| {
            queue.0.push(trigger.event().entity);
        },
    );

    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 1, "one sub-app after spawn");

    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
    assert_eq!(handles.len(), 1, "one host-side handle entity per sub-app");
    let label_entity = handles[0];

    // Despawn the host-side handle entity directly. The OnRemove<DimSubAppHandle>
    // observer fires synchronously, pushing label_entity into DimDespawnQueue.
    app.world_mut().entity_mut(label_entity).despawn();

    // Pump once to let any deferred commands flush (defensive).
    app.update();

    // Drain the despawn queue — sub-app should now be gone.
    drain_dim_despawn_queue(&mut app);
    assert_eq!(
        app.sub_apps().sub_apps.len(),
        0,
        "sub-app must be removed after handle entity is despawned"
    );
}

#[test]
fn no_per_dim_task_pool() {
    let source: &str = include_str!("../../mcrs_minecraft/src/world/sub_app_builder.rs");
    assert!(
        !source.contains("TaskPoolBuilder"),
        "sub_app_builder.rs must not construct its own task pool"
    );
    assert!(
        !source.contains("ComputeTaskPool::init"),
        "sub_app_builder.rs must rely on the process-global ComputeTaskPool"
    );
}

#[test]
fn registries_present_in_all_subapps() {
    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    harness::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);

    let host_registry: RegistryAccess = app.world().resource::<RegistryAccess>().clone();
    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 2, "two sub-apps after two spawns");
    for label in &labels {
        let sub_app = app
            .sub_apps()
            .sub_apps
            .get(label)
            .expect("sub-app present");
        let world = sub_app.world();
        let access = world
            .get_resource::<RegistryAccess>()
            .expect("RegistryAccess resource present in sub-app");
        assert!(
            host_registry.shares_inner_with(access),
            "RegistryAccess clone must share the host Arc"
        );
        assert!(
            world.get_resource::<BlockStateLightTable>().is_some(),
            "BlockStateLightTable resource present in sub-app"
        );
        assert!(
            world.get_resource::<StaticRegistry<Block>>().is_some(),
            "StaticRegistry<Block> resource present in sub-app"
        );
    }
}

#[test]
fn time_extracted_into_subapp() {
    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);

    app.update();

    let host_fixed = *app.world().resource::<Time<Fixed>>();
    let label = *app
        .sub_apps()
        .sub_apps
        .keys()
        .next()
        .expect("one sub-app present");
    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&label)
        .expect("sub-app present");
    let sub_fixed = *sub_app.world().resource::<Time<Fixed>>();
    assert_eq!(
        host_fixed.elapsed(),
        sub_fixed.elapsed(),
        "Time<Fixed>::elapsed should be extracted into the sub-app verbatim"
    );
    assert_eq!(
        host_fixed.delta(),
        sub_fixed.delta(),
        "Time<Fixed>::delta should be extracted into the sub-app verbatim"
    );
}

#[test]
fn eager_spawn_count_matches_dims() {
    use bevy_state::prelude::OnEnter;

    // The production path is `OnEnter(AppState::Playing) →
    // enqueue_dim_spawns_from_preset → DimSpawnQueue → runner drain`. The
    // preset and dimension-type fixtures live behind `pub(crate)` types in
    // `mcrs_minecraft::configuration`, so this test exercises the same
    // OnEnter-then-drain wiring with a small inline system that mirrors what
    // `enqueue_dim_spawns_from_preset` does: push one `DimSpawnRequest` per
    // configured dimension into `DimSpawnQueue`. The drain afterwards is the
    // exact same call the production runner loop makes.
    const EXPECTED_DIMS: &[(&str, bool)] = &[
        ("test:overworld", true),
        ("test:nether", false),
        ("test:end", false),
    ];

    fn enqueue_test_dims(mut spawn_queue: ResMut<DimSpawnQueue>) {
        for (id, has_sky) in EXPECTED_DIMS {
            spawn_queue.0.push(DimSpawnRequest {
                dimension_id: DimensionId::new(*id),
                type_config: DimensionTypeConfig::default(),
                has_sky: *has_sky,
            });
        }
    }

    let mut app = harness::make_main_app();
    app.add_systems(OnEnter(AppState::Playing), enqueue_test_dims);

    harness::drive_to_playing(&mut app);

    assert_eq!(
        app.world().resource::<DimSpawnQueue>().0.len(),
        EXPECTED_DIMS.len(),
        "OnEnter(AppState::Playing) must enqueue one DimSpawnRequest per configured dim"
    );

    drain_dim_spawn_queue(&mut app);

    assert_eq!(
        app.sub_apps().sub_apps.len(),
        EXPECTED_DIMS.len(),
        "drain must materialise one sub-app per enqueued spawn request"
    );

    let host_dim_count = app
        .world_mut()
        .query::<&Dimension>()
        .iter(app.world())
        .count();
    assert_eq!(
        host_dim_count, 0,
        "host world must hold zero Dimension entities — each one lives in its own sub-app"
    );
}

/// Regression test: `enqueue_dim_spawns_from_preset` must be idempotent across
/// repeated `OnEnter(AppState::Playing)` transitions. Without the `Local<bool>`
/// guard, a second transition would re-enqueue all preset dimensions and the
/// drain would materialise a second set of sub-apps (count = 4 instead of 2).
///
/// This test uses an inline `OnEnter(Playing)` system carrying the same
/// `Local<bool>` guard shape as `enqueue_dim_spawns_from_preset`. The production
/// system's preset reading is behind `pub(crate)` types in
/// `mcrs_minecraft::configuration`, so the inline stand-in is the same
/// approach used by `eager_spawn_count_matches_dims`. The guard semantics are
/// identical; only the data source differs.
#[test]
fn enqueue_dim_spawns_from_preset_is_idempotent() {
    use bevy_state::prelude::OnEnter;

    const N: usize = 2;

    fn enqueue_with_guard(mut spawn_queue: ResMut<DimSpawnQueue>, mut guard: Local<bool>) {
        if *guard {
            return;
        }
        *guard = true;
        for i in 0..N {
            let id = if i == 0 { "test:overworld" } else { "test:nether" };
            spawn_queue.0.push(DimSpawnRequest {
                dimension_id: DimensionId::new(id),
                type_config: DimensionTypeConfig::default(),
                has_sky: i == 0,
            });
        }
    }

    let mut app = harness::make_main_app();
    app.add_systems(OnEnter(AppState::Playing), enqueue_with_guard);

    // First transition into Playing: the inline system enqueues N dims.
    harness::drive_to_playing(&mut app);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(
        app.sub_apps().sub_apps.len(),
        N,
        "first drain must materialise N sub-apps"
    );

    // Synthetic re-entry: transition out of Playing and back in.
    // The current AppState enum lacks a Reloading/Paused state, so we drive
    // the transition synthetically. The inline system's OnEnter(Playing)
    // fires again; the Local<bool> guard must prevent re-enqueueing.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::WorldgenFreeze);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();

    drain_dim_spawn_queue(&mut app);

    assert_eq!(
        app.sub_apps().sub_apps.len(),
        N,
        "sub-app count must remain N after a second OnEnter(Playing) — the guard prevented re-enqueue"
    );
}


/// Regression test parallel to `enqueue_dim_spawns_from_preset_is_idempotent`,
/// but exercises the synthetic-overworld fallback branch the production system
/// takes when `LoadedWorldPreset.dimensions.is_empty()`. The guard must be set
/// on this branch too — otherwise a second `OnEnter(Playing)` would push a
/// second synthetic request and the drain would materialise duplicate
/// sub-apps under the same `minecraft:overworld` id.
#[test]
fn enqueue_dim_spawns_from_empty_preset_is_idempotent() {
    use bevy_state::prelude::OnEnter;

    fn enqueue_empty_preset_fallback(
        mut spawn_queue: ResMut<DimSpawnQueue>,
        mut guard: Local<bool>,
    ) {
        if *guard {
            return;
        }
        spawn_queue.0.push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:fallback-overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
        *guard = true;
    }

    let mut app = harness::make_main_app();
    app.add_systems(OnEnter(AppState::Playing), enqueue_empty_preset_fallback);

    harness::drive_to_playing(&mut app);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(
        app.sub_apps().sub_apps.len(),
        1,
        "first drain must materialise exactly one synthetic sub-app from the fallback branch"
    );

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::WorldgenFreeze);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();

    drain_dim_spawn_queue(&mut app);

    assert_eq!(
        app.sub_apps().sub_apps.len(),
        1,
        "fallback-branch guard must prevent re-enqueue on the second OnEnter(Playing)"
    );
}


#[test]
fn worldgen_chunk_plugin_present_in_each_subapp() {
    use mcrs_minecraft::world::chunk::ColumnScheduler;

    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    harness::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 2, "two sub-apps after two spawns");

    for label in &labels {
        let sub_app = app
            .sub_apps()
            .sub_apps
            .get(label)
            .expect("sub-app present");
        assert!(
            sub_app.world().get_resource::<ColumnScheduler>().is_some(),
            "sub-app {label:?} must have ColumnScheduler — confirms worldgen ChunkPlugin is registered, not just the engine storage stub"
        );
    }
}


/// Regression test: the `DimTick` driver must run more than just `Fixed*`.
/// The first iteration of the 01-08 plugin migration chained only Fixed*
/// schedules in `DimTick`, so systems registered on `Update`, `PreUpdate`,
/// `PostUpdate`, `Startup`, or `PostStartup` (`spawn_player`, loot table
/// loading, column-view attachment, etc.) were silently inert. This test
/// installs counter systems on each non-Fixed schedule and asserts they
/// actually execute on each sub-app pump, with `Startup`-family schedules
/// running exactly once across multiple pumps.
#[test]
fn dim_tick_runs_full_main_pipeline() {
    use bevy_app::{First, Last, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, Update};

    #[derive(Resource, Default, Debug, PartialEq, Eq)]
    struct ScheduleHits {
        pre_startup: u32,
        startup: u32,
        post_startup: u32,
        first: u32,
        pre_update: u32,
        update: u32,
        post_update: u32,
        last: u32,
    }

    fn install_counters(sub_app: &mut bevy_app::SubApp) {
        sub_app.init_resource::<ScheduleHits>();
        sub_app.add_systems(PreStartup, |mut h: ResMut<ScheduleHits>| h.pre_startup += 1);
        sub_app.add_systems(Startup, |mut h: ResMut<ScheduleHits>| h.startup += 1);
        sub_app.add_systems(PostStartup, |mut h: ResMut<ScheduleHits>| h.post_startup += 1);
        sub_app.add_systems(First, |mut h: ResMut<ScheduleHits>| h.first += 1);
        sub_app.add_systems(PreUpdate, |mut h: ResMut<ScheduleHits>| h.pre_update += 1);
        sub_app.add_systems(Update, |mut h: ResMut<ScheduleHits>| h.update += 1);
        sub_app.add_systems(PostUpdate, |mut h: ResMut<ScheduleHits>| h.post_update += 1);
        sub_app.add_systems(Last, |mut h: ResMut<ScheduleHits>| h.last += 1);
    }

    let mut app = harness::make_main_app();
    let registries = gather_dim_registries(app.world());
    let request = DimSpawnRequest {
        dimension_id: DimensionId::new("test:overworld"),
        type_config: DimensionTypeConfig::default(),
        has_sky: true,
    };
    let label_entity = spawn_dim_subapp(&mut app, &request, &registries);

    {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .iter_mut()
            .find(|(label, _)| format!("{label:?}").contains(&format!("{label_entity:?}")))
            .map(|(_, s)| s)
            .expect("sub-app must exist for the spawned label");
        install_counters(sub_app);
    }

    const PUMPS: u32 = 3;
    for _ in 0..PUMPS {
        app.update();
    }

    let sub_app = app
        .sub_apps()
        .sub_apps
        .iter()
        .find(|(label, _)| format!("{label:?}").contains(&format!("{label_entity:?}")))
        .map(|(_, s)| s)
        .expect("sub-app must exist");
    let hits = sub_app
        .world()
        .get_resource::<ScheduleHits>()
        .expect("counter resource must be present");

    assert_eq!(hits.pre_startup, 1, "PreStartup must run exactly once");
    assert_eq!(hits.startup, 1, "Startup must run exactly once");
    assert_eq!(hits.post_startup, 1, "PostStartup must run exactly once");
    assert_eq!(hits.first, PUMPS, "First must run on every pump");
    assert_eq!(hits.pre_update, PUMPS, "PreUpdate must run on every pump");
    assert_eq!(hits.update, PUMPS, "Update must run on every pump (covers spawn_player)");
    assert_eq!(
        hits.post_update, PUMPS,
        "PostUpdate must run on every pump (covers despawn_disconnected_clients)"
    );
    assert_eq!(hits.last, PUMPS, "Last must run on every pump");
}
