use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_level::world::sub_app::DimSpawnQueue;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_server::{Lighting, MinecraftServerPlugin};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions};
use mcrs_minecraft_world::registries::test_registries;
use mcrs_minecraft_world::save::{WorldGenSettings, write_world_gen_settings};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;

use crate::support::preset;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn key(name: &str) -> ResourceKey<Dimension> {
    ResourceKey::from_location(name.parse().unwrap())
}

fn debug_dimension(set: &RegistrySet, dimension_type: &str) -> DimensionEntry {
    DimensionEntry {
        dimension_type: set
            .registry::<DimensionType>()
            .and_then(|registry| registry.by_name(dimension_type))
            .expect("the dimension type is loaded"),
        generator: ChunkGenerator::Debug,
    }
}

fn scratch_world(tag: &str) -> std::path::PathBuf {
    let world = std::env::temp_dir().join(format!("mcrs-dimension-list-{tag}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&world).unwrap();
    world
}

#[test]
fn a_save_supplies_the_dimension_list_whatever_the_preset_names() {
    let set = test_registries();
    let mut dimensions = preset("minecraft:normal");
    dimensions.insert(
        key("test:extra"),
        debug_dimension(set, "minecraft:overworld"),
    );
    let world = scratch_world("save");
    write_world_gen_settings(
        &world,
        &WorldGenSettings {
            seed: 3,
            dimensions,
        },
        set,
    )
    .unwrap();

    let spawned = spawned_dimensions(MinecraftServerPlugin {
        world: Some(world.clone()),
        preset: "test:absent".to_owned(),
        ..MinecraftServerPlugin::embedded()
    });
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

fn spawned_dimensions(plugin: MinecraftServerPlugin) -> Vec<String> {
    let mut app = App::new();
    app.add_plugins(MinecraftServerPlugin {
        asset_path: Some(ASSETS.to_owned()),
        lighting: Lighting::Propagated,
        announce_on_lan: false,
        ..plugin
    });
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + Duration::from_secs(45);
    while *app.world().resource::<State<AppState>>().get() != AppState::Playing {
        assert!(Instant::now() < deadline, "never reached Playing");
        app.update();
    }
    app.world()
        .resource::<DimSpawnQueue>()
        .0
        .iter()
        .map(|request| request.dimension.as_str().to_owned())
        .collect()
}

#[test]
fn a_dimension_list_from_the_plugin_needs_no_preset_and_no_overworld() {
    let lobby = Dimensions::from([(
        key("mcrs:lobby"),
        debug_dimension(test_registries(), "minecraft:overworld"),
    )]);
    let spawned = spawned_dimensions(MinecraftServerPlugin {
        dimensions: Some(lobby),
        ..MinecraftServerPlugin::embedded()
    });
    assert_eq!(spawned, ["mcrs:lobby"]);
}
