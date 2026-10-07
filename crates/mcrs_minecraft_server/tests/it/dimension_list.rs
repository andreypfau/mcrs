use std::sync::LazyLock;
use std::time::{Duration, Instant};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::AssetSourceId;
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::layered_file_source;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_level::world::sub_app::DimSpawnQueue;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registry::{LoadReport, Pack, PackFile, RegistrySet, WorldRegistries};
use mcrs_minecraft_server::{Lighting, MinecraftServerPlugin};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake};
use mcrs_minecraft_world::registries::{
    read_packs, static_registries, test_registries, world_registries,
};
use mcrs_minecraft_world::save::{WorldGenSettings, write_world_gen_settings};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

static STATICS: LazyLock<RegistrySet> = LazyLock::new(|| static_registries().unwrap());

static WORLD: LazyLock<WorldRegistries> = LazyLock::new(|| {
    let bytes = std::fs::read(format!("{ASSETS}/mcrs/reports/datapack.json")).unwrap();
    world_registries(&bytes).expect("the report parses")
});

fn key(name: &str) -> ResourceKey<Dimension> {
    ResourceKey::from_location(name.parse().unwrap())
}

fn normal() -> Dimensions {
    let set = test_registries();
    let id = set
        .registry::<mcrs_minecraft_world::worldgen::world_preset::WorldPreset>()
        .and_then(|registry| registry.get(&mcrs_minecraft_world::keys::world_preset::NORMAL))
        .expect("the normal preset is loaded");
    set.entries::<mcrs_minecraft_world::worldgen::world_preset::WorldPreset, WorldPreset>()
        .unwrap()[id]
        .dimensions
        .clone()
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

fn baked(base: &Dimensions, set: &RegistrySet) -> Vec<(ResourceKey<Dimension>, DimensionEntry)> {
    let mut report = LoadReport::new();
    let list = bake(base, set, &mut report);
    assert!(report.is_empty(), "{report}");
    list.expect("a list with an overworld bakes")
}

fn names(list: &[(ResourceKey<Dimension>, DimensionEntry)]) -> Vec<&str> {
    list.iter().map(|(key, _)| key.as_str()).collect()
}

fn load_with_dimension_files(files: &[(&str, &str)]) -> RegistrySet {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        layered_file_source(
            &AssetPlugin::default().file_path,
            mcrs_minecraft_worldgen_builtin::asset,
        ),
    );
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        },
    ));
    let asset_server = app.world().resource::<AssetServer>().clone();
    let mut packs = read_packs(&asset_server, &WORLD, &STATICS);
    packs.push(Pack {
        name: "extra".to_owned(),
        files: files
            .iter()
            .map(|(name, json)| PackFile {
                path: format!("minecraft/dimension/{name}.json"),
                bytes: Some(json.as_bytes().to_vec()),
            })
            .collect(),
        built: Vec::new(),
    });
    WORLD
        .load(&STATICS, &packs)
        .unwrap_or_else(|report| panic!("{report}"))
}

fn scratch_world(tag: &str) -> std::path::PathBuf {
    let world = std::env::temp_dir().join(format!("mcrs-dimension-list-{tag}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&world).unwrap();
    world
}

#[test]
fn a_save_listing_a_dimension_the_preset_lacks_spawns_it() {
    let set = test_registries();
    let mut dimensions = normal();
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

    let mut app = App::new();
    app.add_plugins(MinecraftServerPlugin {
        asset_path: Some(ASSETS.to_owned()),
        lighting: Lighting::Propagated,
        announce_on_lan: false,
        world: Some(world.clone()),
        ..MinecraftServerPlugin::embedded()
    });
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + Duration::from_secs(45);
    while *app.world().resource::<State<AppState>>().get() != AppState::Playing {
        assert!(Instant::now() < deadline, "never reached Playing");
        app.update();
    }

    let spawned: Vec<&str> = app
        .world()
        .resource::<DimSpawnQueue>()
        .0
        .iter()
        .map(|request| request.dimension.as_str())
        .collect();
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

#[test]
fn a_data_pack_dimension_replaces_the_preset_entry_of_its_key() {
    let set = load_with_dimension_files(&[(
        "overworld",
        r#"{"type":"minecraft:overworld_caves","generator":{"type":"minecraft:debug"}}"#,
    )]);
    let base = normal();
    let list = baked(&base, &set);

    assert_eq!(
        names(&list),
        [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end"
        ]
    );
    assert_eq!(
        list[0].1,
        debug_dimension(&set, "minecraft:overworld_caves")
    );
    assert_ne!(list[0].1, base["minecraft:overworld"]);
    assert_eq!(list[1].1, base["minecraft:the_nether"]);
    assert_eq!(list[2].1, base["minecraft:the_end"]);
}
