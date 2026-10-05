use std::sync::LazyLock;
use std::time::{Duration, Instant};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::AssetSourceId;
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::layered_file_source;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_level::world::sub_app::DimSpawnQueue;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registry::{LoadReport, Pack, PackFile, RegistrySet, WorldRegistries};
use mcrs_minecraft_server::{Lighting, MinecraftServerPlugin};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake};
use mcrs_minecraft_world::registries::{
    read_packs, static_registries, test_registries, world_registries,
};
use mcrs_minecraft_world::save::{
    WorldGenSettings, read_world_gen_settings, write_world_gen_settings,
};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

static STATICS: LazyLock<RegistrySet> = LazyLock::new(|| {
    let bytes = std::fs::read(format!("{ASSETS}/mcrs/reports/registries.json")).unwrap();
    static_registries(&bytes).unwrap().0
});

static WORLD: LazyLock<WorldRegistries> = LazyLock::new(|| {
    let bytes = std::fs::read(format!("{ASSETS}/mcrs/reports/datapack.json")).unwrap();
    world_registries(&bytes).expect("the report parses")
});

fn key(name: &str) -> ResourceKey<keys::Dimension> {
    ResourceKey::from_location(name.parse().unwrap())
}

fn normal() -> Dimensions {
    let set = test_registries();
    let id = set
        .registry::<keys::WorldPreset>()
        .and_then(|registry| registry.get("minecraft:normal"))
        .expect("the normal preset is loaded");
    set.entries::<keys::WorldPreset, WorldPreset>().unwrap()[id]
        .dimensions
        .clone()
}

fn debug_dimension(set: &RegistrySet, dimension_type: &str) -> DimensionEntry {
    DimensionEntry {
        dimension_type: set
            .registry::<keys::DimensionType>()
            .and_then(|registry| registry.get(dimension_type))
            .expect("the dimension type is loaded"),
        generator: ChunkGenerator::Debug,
    }
}

fn baked(
    base: &Dimensions,
    set: &RegistrySet,
) -> Vec<(ResourceKey<keys::Dimension>, DimensionEntry)> {
    let mut report = LoadReport::new();
    let list = bake(base, set, &mut report);
    assert!(report.is_empty(), "{report}");
    list.expect("a list with an overworld bakes")
}

fn names(list: &[(ResourceKey<keys::Dimension>, DimensionEntry)]) -> Vec<&str> {
    list.iter().map(|(key, _)| key.as_str()).collect()
}

fn refusal(base: &Dimensions) -> String {
    let mut report = LoadReport::new();
    assert!(bake(base, test_registries(), &mut report).is_none());
    report.to_string()
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
        .map(|request| request.dimension_id.as_str())
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

#[test]
fn a_dimension_list_without_the_overworld_stops_startup() {
    let mut base = normal();
    base.remove("minecraft:overworld");
    let text = refusal(&base);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("minecraft:dimension"), "{text}");
    assert!(text.contains("minecraft:overworld"), "{text}");
}

#[test]
fn an_empty_dimension_list_is_refused_for_the_overworld() {
    let text = refusal(&Dimensions::new());
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("minecraft:dimension"), "{text}");
    assert!(text.contains("minecraft:overworld"), "{text}");
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

#[test]
fn the_baked_order_ignores_input_order() {
    let set = test_registries();
    let entry = debug_dimension(set, "minecraft:overworld");
    let extras = ["z:last", "a:first", "minecraft:the_end", "m:middle"];

    let mut forward = Dimensions::new();
    let mut backward = Dimensions::new();
    let mut full = normal();
    for name in ["minecraft:overworld", "minecraft:the_nether"] {
        forward.insert(key(name), full.remove(name).unwrap());
    }
    for name in extras {
        forward.insert(key(name), entry.clone());
    }
    for name in extras.iter().rev() {
        backward.insert(key(name), entry.clone());
    }
    for name in ["minecraft:the_nether", "minecraft:overworld"] {
        backward.insert(key(name), forward[name].clone());
    }

    let expected = [
        "minecraft:overworld",
        "minecraft:the_nether",
        "minecraft:the_end",
        "a:first",
        "m:middle",
        "z:last",
    ];
    assert_eq!(names(&baked(&forward, set)), expected);
    assert_eq!(baked(&forward, set), baked(&backward, set));
}

#[test]
fn baking_twice_gives_the_same_list() {
    let set = test_registries();
    let mut base = normal();
    base.insert(
        key("test:extra"),
        debug_dimension(set, "minecraft:the_nether"),
    );

    let first = baked(&base, set);
    assert_eq!(first, baked(&base, set));

    let world = scratch_world("round-trip");
    let dimensions: Dimensions = first.iter().cloned().collect();
    write_world_gen_settings(
        &world,
        &WorldGenSettings {
            seed: 1,
            dimensions,
        },
        set,
    )
    .unwrap();
    let read = read_world_gen_settings(&world, set).unwrap();
    assert_eq!(baked(&read.dimensions, set), first);
    std::fs::remove_dir_all(world).unwrap();
}
