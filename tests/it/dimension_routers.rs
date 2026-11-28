use bevy_app::App;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine::between_ticks::drain_dim_spawn_queue;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::dimension_generators::DimensionGenerators;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_game_registries::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_vanilla_worldgen::DimensionPlans;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use std::time::Duration;

#[test]
fn every_noise_dimension_reaches_its_sub_app_with_a_router() {
    let mut app = App::new();
    app.add_plugins(MinecraftServerPlugins::embedded());
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

    let plans = &app.world().resource::<DimensionPlans>().0;
    for dimension in [
        "minecraft:overworld",
        "minecraft:the_nether",
        "minecraft:the_end",
    ] {
        let id = ResourceLocationBuf::read(dimension).unwrap();
        assert!(
            plans.get(&id).is_some_and(|plan| plan.router().is_some()),
            "no router compiled for {dimension}; have {:?}",
            plans.keys().collect::<Vec<_>>()
        );
    }
    let block_of = |name: &str| {
        let id = ResourceLocationBuf::read(name).unwrap();
        let router = plans[&id].router().unwrap();
        (router.default_block_state, router.default_fluid_state)
    };
    assert_ne!(
        block_of("minecraft:overworld"),
        block_of("minecraft:the_nether"),
        "the overworld and the nether were given the same terrain block and fluid"
    );

    let sources: Vec<_> = app
        .world()
        .resource::<DimensionList>()
        .keys()
        .iter()
        .zip(&app.world().resource::<DimensionGenerators>().0)
        .filter_map(|(key, generator)| match generator {
            ChunkGenerator::Noise(generator) => {
                Some((key.location().clone(), generator.biome_source.clone()))
            }
            _ => None,
        })
        .collect();
    let expected = plans
        .values()
        .filter(|plan| plan.router().is_some())
        .count();
    drain_dim_spawn_queue(&mut app);

    let mut sub_apps: Vec<_> = app
        .world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .sub_apps_mut()
        .collect();
    assert!(!sub_apps.is_empty(), "the preset spawned no dimension");
    let with_router = sub_apps
        .iter()
        .filter(|sub_app| sub_app.world().get_resource::<FillContext>().is_some())
        .count();
    assert_eq!(
        with_router,
        expected,
        "{with_router} of {} sub-apps got a router",
        sub_apps.len()
    );

    let mut checked = 0;
    for sub_app in &mut sub_apps {
        let world = sub_app.world_mut();
        let dimension = world
            .query::<&ResourceKeyBuf<Dimension>>()
            .single(world)
            .expect("a sub-app holds one dimension")
            .location()
            .clone();
        let Some(context) = world.get_resource::<FillContext>() else {
            continue;
        };
        let held = context
            .biome
            .as_ref()
            .unwrap_or_else(|| panic!("{dimension} reached its sub-app with no biome source"));
        let expected_source = sources
            .iter()
            .find(|(key, _)| *key == dimension)
            .map(|(_, source)| source)
            .unwrap_or_else(|| panic!("{dimension} has no noise generator in the list"));
        assert!(
            **held == *expected_source,
            "{dimension} was given another dimension's biome source"
        );
        checked += 1;
    }
    assert_eq!(checked, expected, "a dimension went unchecked");
}
