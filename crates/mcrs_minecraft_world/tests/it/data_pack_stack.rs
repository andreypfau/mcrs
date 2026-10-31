use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_datapack::{Level, PackDiagnostic, StackedPack};
use mcrs_minecraft_registries::{Diagnostic, Registries};

use crate::common::{TEST_PACK, WithFiles};

#[test]
fn a_pack_that_ships_a_built_in_biome_replaces_the_built_in_entry() {
    let (registries, _) = WithFiles::test_pack(&[(
        "minecraft/worldgen/biome/plains.json",
        r##"{"temperature":0.125,"downfall":0.5,"has_precipitation":false,"effects":{"water_color":"#3f76e4"}}"##,
    )])
    .load()
    .unwrap_or_else(|error| panic!("{error}"));
    let set = &registries;

    let biomes = set.registry::<BiomeFile>().unwrap();
    let plains = biomes
        .by_name(&"minecraft:plains".parse().unwrap())
        .unwrap();
    let biome = biomes.get(plains).unwrap();
    assert_eq!(
        (biome.temperature, biome.downfall, biome.has_precipitation),
        (0.125, 0.5, false)
    );
    assert_eq!(
        set.origin(plains).map(StackedPack::id),
        Some("file/zz_test")
    );
}

fn banner_pattern(name: &str) -> String {
    format!(r#"{{"asset_id":"minecraft:{name}","translation_key":"k"}}"#)
}

fn has_banner_pattern(set: &Registries, name: &str) -> bool {
    set.names_of(&"minecraft:banner_pattern".parse().unwrap())
        .is_some_and(|names| names.iter().any(|entry| entry.as_str() == name))
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
        let (registries, diagnostics) = source
            .load()
            .unwrap_or_else(|error| panic!("{case}: {error}"));

        assert!(
            !has_banner_pattern(&registries, "minecraft:stray"),
            "{case}"
        );
        let reported: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|diagnostic| {
                matches!(diagnostic, Diagnostic::Pack(PackDiagnostic::UnavailablePack { pack, .. }) if pack == "file/zz_test")
            })
            .collect();
        assert_eq!(reported.len(), 1, "{case}: {:?}", diagnostics);
        assert_eq!(reported[0].level(), Level::Warning, "{case}");
    }
}

#[test]
fn the_vanilla_pack_holds_only_the_minecraft_namespace_of_the_asset_root() {
    let (registries, _) = WithFiles::over_assets()
        .with("mcrs/banner_pattern/local.json", banner_pattern("local"))
        .with("example/banner_pattern/other.json", banner_pattern("other"))
        .with("minecraft/banner_pattern/kept.json", banner_pattern("kept"))
        .load()
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(has_banner_pattern(&registries, "minecraft:kept"));
    assert!(!has_banner_pattern(&registries, "mcrs:local"));
    assert!(!has_banner_pattern(&registries, "example:other"));
}

#[test]
fn a_feature_pack_is_not_read_as_part_of_the_vanilla_pack() {
    let (registries, _) = WithFiles::over_assets()
        .with(
            "minecraft/datapacks/zz_feature/pack.mcmeta",
            r#"{"features":{"enabled":["minecraft:not_enabled"]}}"#,
        )
        .unreadable("minecraft/datapacks/zz_feature/data/minecraft/banner_pattern/feature.json")
        .load()
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(!has_banner_pattern(&registries, "minecraft:feature"));
}

#[test]
fn a_jar_file_that_a_built_in_entry_replaces_is_never_read() {
    WithFiles::over_assets()
        .unreadable("minecraft/worldgen/noise/temperature.json")
        .load()
        .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn an_active_overlay_of_a_local_pack_is_read_under_its_own_root() {
    let (registries, _) = WithFiles::over_assets()
        .pack(TEST_PACK)
        .with(
            format!("{TEST_PACK}/pack.mcmeta"),
            format!(
                r#"{{"pack":{{"description":"","min_format":{format},"max_format":{format}}},
                "overlays":{{"entries":[{{"directory":"later","min_format":{format},"max_format":{format}}}]}}}}"#,
                format = VERSION.pack_version.data_major,
            ),
        )
        .with(
            format!("{TEST_PACK}/later/minecraft/banner_pattern/overlaid.json"),
            banner_pattern("overlaid"),
        )
        .load()
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(has_banner_pattern(&registries, "minecraft:overlaid"));
}
