use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_registry::{LoadReport, RegistrySet};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake, bake_list};
use mcrs_minecraft_world::registries::test_registries;
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
    let mut base = normal();
    base.remove("minecraft:the_end");
    for name in ["z:last", "a:first", "minecraft:the_end", "m:middle"] {
        base.insert(key(name), entry.clone());
    }

    assert_eq!(
        names(&baked(&base, set)),
        [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
            "a:first",
            "m:middle",
            "z:last",
        ]
    );
}

#[test]
fn a_data_pack_dimension_replaces_the_preset_entry_of_its_key() {
    let set = crate::loaded_registries::load_shipped_and(
        "minecraft/dimension",
        &[(
            "overworld",
            r#"{"type":"minecraft:overworld_caves","generator":{"type":"minecraft:debug"}}"#
                .to_owned(),
        )],
    )
    .unwrap_or_else(|report| panic!("{report}"));
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
