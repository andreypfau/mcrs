use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_registries::{Diagnostic, Level};
use mcrs_minecraft_registry::RegistrySet;

use crate::common::{TEST_PACK, WithFiles};

#[test]
fn a_pack_that_ships_a_built_in_biome_replaces_the_built_in_entry() {
    let loaded = WithFiles::test_pack(&[(
        "minecraft/worldgen/biome/plains.json",
        r##"{"temperature":0.125,"downfall":0.5,"has_precipitation":false,"effects":{"water_color":"#3f76e4"}}"##,
    )])
    .load()
    .unwrap_or_else(|error| panic!("{error}"));
    let set = &loaded.set;

    let biomes = set.registry::<Biome>().unwrap();
    let plains = biomes.by_name("minecraft:plains").unwrap();
    let values = set.entries::<Biome, Biome>().unwrap();
    let biome = values.get(plains).unwrap();
    assert_eq!(
        (biome.temperature, biome.downfall, biome.has_precipitation),
        (0.125, 0.5, false)
    );
    assert_eq!(
        set.pack_of("minecraft:worldgen/biome", plains.index()),
        Some("file/zz_test")
    );
}

fn banner_pattern(name: &str) -> String {
    format!(r#"{{"asset_id":"minecraft:{name}","translation_key":"k"}}"#)
}

fn has_banner_pattern(set: &RegistrySet, name: &str) -> bool {
    set.table("minecraft:banner_pattern")
        .and_then(|table| table.number(name))
        .is_some()
}

#[test]
fn a_pack_directory_the_loader_cannot_read_is_reported_and_not_loaded() {
    let without_metadata = WithFiles::over_assets().with(
        "mcrs/datapacks/zz_test/minecraft/banner_pattern/stray.json",
        banner_pattern("stray"),
    );
    let with_data_directory = WithFiles::over_assets()
        .pack(TEST_PACK)
        .with(
            format!("{TEST_PACK}/data/minecraft/banner_pattern/stray.json"),
            banner_pattern("stray"),
        )
        .with(
            format!("{TEST_PACK}/minecraft/banner_pattern/stray.json"),
            banner_pattern("stray"),
        );
    for (case, source) in [
        ("without metadata", without_metadata),
        ("with a data directory", with_data_directory),
    ] {
        let loaded = source
            .load()
            .unwrap_or_else(|error| panic!("{case}: {error}"));

        assert!(
            !has_banner_pattern(&loaded.set, "minecraft:stray"),
            "{case}"
        );
        let reported: Vec<&Diagnostic> = loaded
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                matches!(diagnostic, Diagnostic::UnavailablePack { pack, .. } if pack == "file/zz_test")
            })
            .collect();
        assert_eq!(reported.len(), 1, "{case}: {:?}", loaded.diagnostics);
        assert_eq!(reported[0].level(), Level::Warning, "{case}");
    }
}

#[test]
fn the_vanilla_pack_holds_only_the_minecraft_namespace_of_the_asset_root() {
    let loaded = WithFiles::over_assets()
        .with("mcrs/banner_pattern/local.json", banner_pattern("local"))
        .with("example/banner_pattern/other.json", banner_pattern("other"))
        .with("minecraft/banner_pattern/kept.json", banner_pattern("kept"))
        .load()
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(has_banner_pattern(&loaded.set, "minecraft:kept"));
    assert!(!has_banner_pattern(&loaded.set, "mcrs:local"));
    assert!(!has_banner_pattern(&loaded.set, "example:other"));
}

#[test]
fn a_feature_pack_is_not_read_as_part_of_the_vanilla_pack() {
    let loaded = WithFiles::over_assets()
        .with(
            "minecraft/datapacks/zz_feature/pack.mcmeta",
            r#"{"features":{"enabled":["minecraft:not_enabled"]}}"#,
        )
        .unreadable("minecraft/datapacks/zz_feature/data/minecraft/banner_pattern/feature.json")
        .load()
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(!has_banner_pattern(&loaded.set, "minecraft:feature"));
}

#[test]
fn an_active_overlay_of_a_local_pack_is_read_under_its_own_root() {
    let loaded = WithFiles::over_assets()
        .with(
            format!("{TEST_PACK}/pack.mcmeta"),
            r#"{"pack":{"description":"","min_format":122,"max_format":122},
                "overlays":{"entries":[{"directory":"later","min_format":122,"max_format":122}]}}"#,
        )
        .with(
            format!("{TEST_PACK}/later/minecraft/banner_pattern/overlaid.json"),
            banner_pattern("overlaid"),
        )
        .load()
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(has_banner_pattern(&loaded.set, "minecraft:overlaid"));
}
