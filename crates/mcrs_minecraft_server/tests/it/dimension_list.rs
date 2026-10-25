use bevy_app::{App, AppExit};
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_environment::world_clock::{WorldClock, WorldClocks};
use mcrs_minecraft_level::world::sub_app::DimSpawnQueue;
use mcrs_minecraft_light::block_light::BlockLightRegistry;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_server::world::heightmap::BlockHeightmapPredicates;
use mcrs_minecraft_server::{DimensionWorldsPlugin, MinecraftServerPlugins};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions};
use mcrs_minecraft_world::registry_load::test_registries;
use mcrs_minecraft_world::save::{WorldGenSettings, write_world_gen_settings};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;

use crate::support::preset;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn key(name: &str) -> ResourceKey<Dimension> {
    ResourceKey::from_location(name.parse().unwrap())
}

fn debug_dimension(dimension_type: &str) -> DimensionEntry {
    DimensionEntry {
        dimension_type: crate::support::dimension_type(dimension_type),
        generator: ChunkGenerator::Debug,
    }
}

fn scratch_world(tag: &str) -> std::path::PathBuf {
    let world = std::env::temp_dir().join(format!("mcrs-dimension-list-{tag}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&world).unwrap();
    world
}

#[test]
fn a_save_supplies_the_dimension_list_whatever_the_preset_names_and_startup_builds_the_tables() {
    let mut dimensions = preset("minecraft:normal");
    dimensions.insert(key("test:extra"), debug_dimension("minecraft:overworld"));
    let world = scratch_world("save");
    write_world_gen_settings(
        &world,
        &WorldGenSettings {
            seed: 3,
            dimensions,
        },
        test_registries(),
    )
    .unwrap();

    let mut app = booted(DimensionWorldsPlugin {
        world: Some(world.clone()),
        preset: "test:absent".to_owned(),
        ..Default::default()
    });
    app.update();
    the_startup_tables_and_clocks_exist_after_the_first_update(&app);
    let spawned = spawned_dimensions(app);
    assert_eq!(
        spawned,
        [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
            "test:extra"
        ]
    );
    std::fs::remove_dir_all(world).unwrap();
}

#[test]
fn a_preset_that_names_no_world_preset_asks_the_app_to_exit_with_an_error() {
    let mut app = booted(DimensionWorldsPlugin {
        preset: "test:absent".to_owned(),
        ..Default::default()
    });
    app.update();
    assert_eq!(app.should_exit(), Some(AppExit::error()));
}

fn booted(worlds: DimensionWorldsPlugin) -> App {
    let mut plugins = MinecraftServerPlugins {
        worlds,
        ..MinecraftServerPlugins::embedded()
    };
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.network.announce_on_lan = false;
    let mut app = App::new();
    app.add_plugins(plugins);
    app.finish();
    app.cleanup();
    app
}

fn the_startup_tables_and_clocks_exist_after_the_first_update(app: &App) {
    assert_ne!(
        *app.world().resource::<State<AppState>>().get(),
        AppState::Playing,
        "the update must not have reached Playing, or it proves nothing about Startup"
    );
    assert!(
        app.world().get_resource::<BlockLightRegistry>().is_some(),
        "no block light table after the first update"
    );
    assert!(
        app.world()
            .get_resource::<BlockHeightmapPredicates>()
            .is_some(),
        "no heightmap predicates after the first update"
    );
    let clocks = app.world().resource::<WorldClocks>();
    let registered = app
        .world()
        .resource::<CurrentRegistries>()
        .registry::<WorldClock>()
        .expect("the world clock registry is loaded");
    assert!(!registered.is_empty());
    assert_eq!(
        clocks.len(),
        registered.len(),
        "the clocks held after the first update differ from the registry's"
    );
    for (id, _, _) in registered.iter() {
        let id =
            mcrs_minecraft_core::Id::<WorldClock>::checked(id.number(), registered.len()).unwrap();
        assert!(clocks.get(id).is_some(), "{id:?} is not seeded");
    }
}

fn spawned_dimensions(mut app: App) -> Vec<String> {
    crate::support::update_until_playing(&mut app);
    app.world()
        .resource::<DimSpawnQueue>()
        .0
        .iter()
        .map(|request| request.dimension.as_str().to_owned())
        .collect()
}

mod exhaustive {
    use super::*;

    #[test]
    fn a_dimension_list_from_the_plugin_needs_no_preset_and_no_overworld() {
        let lobby = Dimensions::from([(key("mcrs:lobby"), debug_dimension("minecraft:overworld"))]);
        let spawned = spawned_dimensions(booted(DimensionWorldsPlugin {
            dimensions: Some(lobby),
            ..Default::default()
        }));
        assert_eq!(spawned, ["mcrs:lobby"]);
    }
}
