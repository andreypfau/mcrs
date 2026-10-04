// Integration tests for the per-dimension sub-app lifecycle. Each test
// constructs a host `App` via the shared `common` fixture, enqueues a
// synthetic spawn request, drains the queue through the production builder,
// and inspects the resulting sub-app population.

use bevy_app::AppLabel;
use bevy_ecs::prelude::*;
use bevy_time::{Fixed, Time};
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionId, DimensionTypeConfig};
use mcrs_minecraft_level::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnRequest,
};
use mcrs_minecraft_server::world::sub_app_builder::{
    DimSubAppHandle, drain_dim_despawn_queue, drain_dim_spawn_queue, gather_dim_registries,
    spawn_dim_subapp,
};

use crate::host_app;

#[test]
fn dim_subapp_removed_on_despawn() {
    let mut app = host_app::make_host_app();
    host_app::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 1);

    // The label-anchor entity in the host world is the same value the
    // sub-app was interned under.
    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
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
    let mut app = host_app::make_host_app();
    host_app::enqueue_spawn(&mut app, "test:overworld", true);
    host_app::enqueue_spawn(&mut app, "test:nether", false);
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
}

#[test]
fn registries_present_in_all_subapps() {
    let mut app = host_app::make_host_app();
    host_app::enqueue_spawn(&mut app, "test:overworld", true);
    host_app::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);

    let host_registry: RegistryAccess = app.world().resource::<RegistryAccess>().clone();
    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 2, "two sub-apps after two spawns");
    for label in &labels {
        let sub_app = app.sub_apps().sub_apps.get(label).expect("sub-app present");
        let world = sub_app.world();
        let access = world
            .get_resource::<RegistryAccess>()
            .expect("RegistryAccess resource present in sub-app");
        assert!(
            host_registry.shares_inner_with(access),
            "RegistryAccess clone must share the host Arc"
        );
        assert!(
            world.get_resource::<Blocks>().is_some(),
            "the block definition corpus is present in the sub-app"
        );
    }
}

#[test]
fn time_extracted_into_subapp() {
    let mut app = host_app::make_host_app();
    host_app::enqueue_spawn(&mut app, "test:overworld", true);
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
fn worldgen_chunk_plugin_present_in_each_subapp() {
    use mcrs_minecraft_server::world::chunk::ColumnScheduler;

    let mut app = host_app::make_host_app();
    host_app::enqueue_spawn(&mut app, "test:overworld", true);
    host_app::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 2, "two sub-apps after two spawns");

    for label in &labels {
        let sub_app = app.sub_apps().sub_apps.get(label).expect("sub-app present");
        assert!(
            sub_app.world().get_resource::<ColumnScheduler>().is_some(),
            "sub-app {label:?} must have ColumnScheduler — confirms worldgen ChunkPlugin is registered, not just the engine storage stub"
        );
    }
}

/// Regression test: the `DimTick` driver must run more than just `Fixed*`.
/// An earlier `DimTick` chained only Fixed*
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
        sub_app.add_systems(PostStartup, |mut h: ResMut<ScheduleHits>| {
            h.post_startup += 1
        });
        sub_app.add_systems(First, |mut h: ResMut<ScheduleHits>| h.first += 1);
        sub_app.add_systems(PreUpdate, |mut h: ResMut<ScheduleHits>| h.pre_update += 1);
        sub_app.add_systems(Update, |mut h: ResMut<ScheduleHits>| h.update += 1);
        sub_app.add_systems(PostUpdate, |mut h: ResMut<ScheduleHits>| h.post_update += 1);
        sub_app.add_systems(Last, |mut h: ResMut<ScheduleHits>| h.last += 1);
    }

    let mut app = host_app::make_host_app();
    let registries = gather_dim_registries(app.world());
    let request = DimSpawnRequest {
        dimension_id: DimensionId::new("test:overworld"),
        type_config: DimensionTypeConfig::new(-64, 384),
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
    assert_eq!(
        hits.update, PUMPS,
        "Update must run on every pump (covers spawn_player)"
    );
    assert_eq!(
        hits.post_update, PUMPS,
        "PostUpdate must run on every pump (covers despawn_disconnected_clients)"
    );
    assert_eq!(hits.last, PUMPS, "Last must run on every pump");
}
