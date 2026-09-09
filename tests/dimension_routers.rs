use bevy_app::App;
use bevy_state::state::State;
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_server::MinecraftServerPlugin;
use mcrs_minecraft_server::world::generate::{DimensionBiomeSources, DimensionRouters};
use mcrs_minecraft_server::world::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_world::worldgen::beta_biome::ActiveBiomeSource;
use mcrs_minecraft_worldgen::bevy::DimensionNoiseRouter;
use std::sync::Arc;
use std::time::Duration;

/// The preset is loaded once, in the host, and every dimension's router is
/// compiled from it there. A dimension that reached its sub-app without one
/// generates nothing and says so only in a log line, so assert the handoff.
#[test]
fn every_noise_dimension_reaches_its_sub_app_with_a_router() {
    let mut app = App::new();
    app.add_plugins(MinecraftServerPlugin::embedded());
    // The block corpus is loaded in `Plugin::finish`, which `App::run` would
    // have called for us.
    app.finish();
    app.cleanup();

    let mut reached_playing = false;
    for _ in 0..20_000 {
        app.update();
        if **app.world().resource::<State<AppState>>() == AppState::Playing {
            reached_playing = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(reached_playing, "the server never reached Playing");

    let routers = app.world().resource::<DimensionRouters>();
    for dimension in [
        "minecraft:overworld",
        "minecraft:the_nether",
        "minecraft:the_end",
    ] {
        let id = ResourceLocation::parse(dimension).unwrap();
        assert!(
            routers.0.contains_key(&id),
            "no router compiled for {dimension}; have {:?}",
            routers.0.keys().collect::<Vec<_>>()
        );
    }
    // Each dimension's noise settings name their own terrain block, so a build
    // that resolved them globally would hand every dimension the same pair.
    let block_of = |name: &str| {
        let id = ResourceLocation::parse(name).unwrap();
        let router = &routers.0[&id];
        (router.default_block_state, router.default_fluid_state)
    };
    assert_ne!(
        block_of("minecraft:overworld"),
        block_of("minecraft:the_nether"),
        "the overworld and the nether were given the same terrain block and fluid"
    );

    let sources = app.world().resource::<DimensionBiomeSources>().clone();
    // A sub-app is keyed by an entity, so the router it was handed is what says
    // which dimension it is: the terrain block pair is distinct per dimension,
    // which the assertion above holds to.
    let dimension_of: Vec<_> = routers
        .0
        .iter()
        .map(|(id, router)| {
            (
                (router.default_block_state, router.default_fluid_state),
                id.clone(),
            )
        })
        .collect();

    let expected = routers.0.len();
    drain_dim_spawn_queue(&mut app);

    let sub_apps = &app.sub_apps().sub_apps;
    assert!(!sub_apps.is_empty(), "the preset spawned no dimension");
    let with_router = sub_apps
        .values()
        .filter(|sub_app| {
            sub_app
                .world()
                .get_resource::<DimensionNoiseRouter>()
                .is_some()
        })
        .count();
    assert_eq!(
        with_router,
        expected,
        "{with_router} of {} sub-apps got a router",
        sub_apps.len()
    );

    // The biome source decides the biome a column reports, and with it the
    // surface rules and the carvers. A single host-wide source would hand the
    // nether the overworld's biomes while it samples the nether's router.
    let mut checked = 0;
    for sub_app in sub_apps.values() {
        let Some(router) = sub_app.world().get_resource::<DimensionNoiseRouter>() else {
            continue;
        };
        let key = (router.0.default_block_state, router.0.default_fluid_state);
        let dimension = dimension_of
            .iter()
            .find(|(pair, _)| *pair == key)
            .map(|(_, id)| id)
            .expect("a sub-app carries a router no dimension compiled");
        let held = sub_app
            .world()
            .get_resource::<ActiveBiomeSource>()
            .unwrap_or_else(|| panic!("{dimension} reached its sub-app with no biome source"));
        assert!(
            Arc::ptr_eq(&held.0, &sources.0[dimension]),
            "{dimension} was given another dimension's biome source"
        );
        checked += 1;
    }
    assert_eq!(checked, expected, "a dimension went unchecked");
}
