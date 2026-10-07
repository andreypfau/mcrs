use bevy_app::{App, AppLabel};
use bevy_app::{
    First, FixedUpdate, Last, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, Update,
};
use bevy_ecs::prelude::*;
use bevy_time::{Fixed, Time};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_environment::world_clock::{ClockState, WorldClockPlugin, WorldClocks};
use mcrs_minecraft_level::session::{PlayerSessionCounter, Session};
use mcrs_minecraft_level::world::dimension::Dimension;
use mcrs_minecraft_level::world::sub_app::{DimAppLabel, DimDespawnQueue};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_registry::shared::SharedResource;
use mcrs_minecraft_registry::{Id, Registry, RegistrySet};
use mcrs_minecraft_server::world::bus::InboundPlayerSpawn;
use mcrs_minecraft_server::world::sub_app_builder::{
    DimSubAppHandle, drain_dim_despawn_queue, drain_dim_spawn_queue,
};
use mcrs_minecraft_server::world_options::DimensionList;

use crate::host_app;
use mcrs_minecraft_environment::world_clock::WorldClock;

#[test]
fn dim_sub_apps_are_isolated_worlds_that_come_and_go() {
    let mut app = host_app::make_host_app();
    host_app::enqueue_spawn(&mut app, "minecraft:overworld", "minecraft:overworld");
    host_app::enqueue_spawn(&mut app, "minecraft:the_nether", "minecraft:the_nether");
    drain_dim_spawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 2);

    let host_dimensions = app
        .world_mut()
        .query::<&Dimension>()
        .iter(app.world())
        .count();
    assert_eq!(
        host_dimensions, 0,
        "host world should hold zero Dimension entities"
    );

    let host_registry: RegistrySet = app.world().resource::<RegistrySet>().clone();
    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
    assert_eq!(handles.len(), 2, "one host-side handle entity per sub-app");

    for &label in &handles {
        let world = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(&DimAppLabel(label).intern())
            .expect("sub-app under the handle's DimAppLabel")
            .world_mut();
        let dimensions = world.query::<&Dimension>().iter(world).count();
        assert_eq!(
            dimensions, 1,
            "exactly one Dimension entity per sub-app world"
        );
        let set = world
            .get_resource::<RegistrySet>()
            .expect("RegistrySet resource present in sub-app");
        assert!(
            host_registry.shares_with(set),
            "RegistrySet clone must share the host Arc"
        );
        assert!(
            world.get_resource::<Blocks>().is_some(),
            "the block definition corpus is present in the sub-app"
        );

        assert_eq!(world.query::<&Session>().iter(world).count(), 0);
        assert_eq!(
            world.query::<&ServerSideConnection>().iter(world).count(),
            0
        );
        assert!(!world.contains_resource::<PlayerSessionCounter>());
        assert!(!world.contains_resource::<DimensionList>());
    }

    app.world_mut()
        .resource_mut::<DimDespawnQueue>()
        .0
        .push(handles[0]);
    drain_dim_despawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 1);
    assert!(
        app.sub_apps()
            .sub_apps
            .contains_key(&DimAppLabel(handles[1]).intern()),
        "only the despawned dimension's sub-app is removed"
    );
}

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

fn world_clocks(app: &App) -> Registry<WorldClock> {
    app.world()
        .resource::<RegistrySet>()
        .registry::<WorldClock>()
        .expect("the world clock registry is loaded")
}

fn overworld_clock(app: &App) -> Id<WorldClock> {
    world_clocks(app)
        .require(&mcrs_minecraft_environment::keys::world_clock::OVERWORLD)
        .expect("the overworld clock is registered")
}

fn sub_app_clock(app: &mut App, dim_label: Entity) -> ClockState {
    let overworld = overworld_clock(app);
    *app.sub_app_mut(DimAppLabel(dim_label))
        .world()
        .resource::<WorldClocks>()
        .get(overworld)
        .expect("the extract carried the overworld clock into the sub-world")
}

fn main_clock(app: &App) -> ClockState {
    *app.world()
        .resource::<WorldClocks>()
        .get(overworld_clock(app))
        .unwrap()
}

/// Systems on every non-`Fixed` schedule of a sub-app run on each pump, with
/// the `Startup` family once: an earlier driver chained only the `Fixed*`
/// schedules and left `spawn_player`, loot loading and column-view attachment
/// inert. The main app owns time and the clocks; each pump hands the sub-world
/// the same values, overwriting any it advanced on its own.
#[test]
fn a_dim_sub_app_runs_the_whole_pipeline_on_the_main_app_time_and_clocks() {
    let mut app = host_app::make_host_app();
    app.add_message::<InboundPlayerSpawn>();
    app.add_plugins(WorldClockPlugin);
    let mut clocks = WorldClocks::default();
    clocks.reconcile_with_registry(&world_clocks(&app));
    app.insert_resource(clocks);
    host_app::drive_to_playing(&mut app);
    host_app::materialise_sub_apps(&mut app, &[("minecraft:overworld", "minecraft:overworld")]);
    let dim_label = app
        .world_mut()
        .query_filtered::<Entity, With<DimSubAppHandle>>()
        .single(app.world())
        .unwrap();
    install_counters(app.sub_app_mut(DimAppLabel(dim_label)));

    let before = main_clock(&app).total_ticks;
    for _ in 0..7 {
        app.world_mut().run_schedule(FixedUpdate);
    }
    app.update();
    assert!(main_clock(&app).total_ticks >= before + 7);
    assert_eq!(sub_app_clock(&mut app, dim_label), main_clock(&app));

    let host_fixed = *app.world().resource::<Time<Fixed>>();
    let sub_fixed = *app
        .sub_app(DimAppLabel(dim_label))
        .world()
        .resource::<Time<Fixed>>();
    assert_eq!(host_fixed.elapsed(), sub_fixed.elapsed());
    assert_eq!(host_fixed.delta(), sub_fixed.delta());

    let overworld = overworld_clock(&app);
    app.sub_app_mut(DimAppLabel(dim_label))
        .world_mut()
        .resource_mut::<WorldClocks>()
        .get_mut(overworld)
        .unwrap()
        .total_ticks = 999_999;
    app.update();
    assert_eq!(sub_app_clock(&mut app, dim_label), main_clock(&app));
    assert_ne!(main_clock(&app).total_ticks, 999_999);

    app.world_mut().run_schedule(FixedUpdate);
    app.update();
    assert_eq!(sub_app_clock(&mut app, dim_label), main_clock(&app));

    const PUMPS: u32 = 3;
    let hits = app
        .sub_app(DimAppLabel(dim_label))
        .world()
        .resource::<ScheduleHits>();
    assert_eq!(
        *hits,
        ScheduleHits {
            pre_startup: 1,
            startup: 1,
            post_startup: 1,
            first: PUMPS,
            pre_update: PUMPS,
            update: PUMPS,
            post_update: PUMPS,
            last: PUMPS,
        }
    );
}
