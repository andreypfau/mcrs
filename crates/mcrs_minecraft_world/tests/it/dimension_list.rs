use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_registries::{Key, Registries, RegistryOf, Resolver};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake, bake_list};
use mcrs_minecraft_world::registry_load::test_registries_arc;
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;

fn key(name: &str) -> ResourceKey<Dimension> {
    ResourceKey::from_location(name.parse().unwrap())
}

fn normal() -> Dimensions {
    let presets = test_registries_arc()
        .registry::<RegistryOf<WorldPreset>>()
        .expect("the presets are loaded");
    let id = presets
        .by_name(
            &mcrs_minecraft_world::keys::world_preset::NORMAL
                .location()
                .to_arc(),
        )
        .expect("the normal preset is loaded");
    presets.get(id).unwrap().dimensions.clone()
}

fn debug_dimension(set: &Registries, dimension_type: &str) -> DimensionEntry {
    DimensionEntry {
        dimension_type: set
            .id(&Key::new(dimension_type.parse().unwrap()))
            .expect("the dimension type is loaded"),
        generator: ChunkGenerator::Debug,
    }
}

fn baked(base: &Dimensions, set: &Registries) -> Vec<(ResourceKey<Dimension>, DimensionEntry)> {
    bake(base, set).expect("a list with an overworld bakes")
}

fn names(list: &[(ResourceKey<Dimension>, DimensionEntry)]) -> Vec<&str> {
    list.iter().map(|(key, _)| key.as_str()).collect()
}

#[test]
fn an_empty_dimension_list_from_the_plugin_is_refused() {
    let text = bake_list(&Dimensions::new(), test_registries_arc()).unwrap_err();
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
        let text = bake(&base, test_registries_arc()).unwrap_err();
        assert!(text.contains("minecraft:dimension"), "{case}: {text}");
        assert!(text.contains("minecraft:overworld"), "{case}: {text}");
    }
}

#[test]
fn the_baked_order_ignores_input_order() {
    let set = test_registries_arc();
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
