use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_registry::{LoadReport, RegistrySet};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake, bake_list};
use mcrs_minecraft_world::registries::test_registries;
use mcrs_minecraft_world::save::{
    WorldGenSettings, read_world_gen_settings, write_world_gen_settings,
};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;

fn key(name: &str) -> ResourceKey<Dimension> {
    ResourceKey::from_location(name.parse().unwrap())
}

fn normal() -> Dimensions {
    let set = test_registries();
    let id = set
        .registry::<WorldPreset>()
        .and_then(|registry| registry.get(&mcrs_minecraft_world::keys::world_preset::NORMAL))
        .expect("the normal preset is loaded");
    set.entries::<WorldPreset, WorldPreset>().unwrap()[id]
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

#[test]
fn an_empty_dimension_list_from_the_plugin_is_refused() {
    let mut report = LoadReport::new();
    assert!(bake_list(&Dimensions::new(), test_registries(), &mut report).is_none());
    let text = report.to_string();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("minecraft:dimension"), "{text}");
}

#[test]
fn a_dimension_list_without_the_overworld_is_refused() {
    let mut without_overworld = normal();
    without_overworld.remove("minecraft:overworld");
    for (case, base) in [
        ("without the overworld", without_overworld),
        ("empty", Dimensions::new()),
    ] {
        let mut report = LoadReport::new();
        assert!(
            bake(&base, test_registries(), &mut report).is_none(),
            "{case}"
        );
        let text = report.to_string();
        assert_eq!(text.lines().count(), 1, "{case}: {text}");
        assert!(text.contains("minecraft:dimension"), "{case}: {text}");
        assert!(text.contains("minecraft:overworld"), "{case}: {text}");
    }
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

    let world = std::env::temp_dir().join(format!(
        "mcrs-dimension-list-round-trip-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&world);
    std::fs::create_dir_all(&world).unwrap();
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
