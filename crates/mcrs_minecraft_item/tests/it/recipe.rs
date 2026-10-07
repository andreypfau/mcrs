use std::sync::LazyLock;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::TrimPattern;
use mcrs_minecraft_item::keys::TRIM_PATTERN;
use mcrs_minecraft_item::recipe::Recipe;
use mcrs_minecraft_registry::{Registry, RegistrySet};
use mcrs_minecraft_worldgen_testing::{assets_dir, json_files, reencode, tagged_report};

fn recipe_set() -> &'static RegistrySet {
    static SET: LazyLock<RegistrySet> = LazyLock::new(|| {
        let base = assets_dir().join("minecraft/trim_pattern");
        let names = json_files(&base).into_iter().map(|path| {
            let name = path.file_stem().and_then(|stem| stem.to_str()).unwrap();
            ResourceLocation::minecraft(name).unwrap()
        });
        tagged_report()
            .clone()
            .with(Registry::<TrimPattern>::new(TRIM_PATTERN, names).unwrap())
            .unwrap()
    });
    &SET
}

#[test]
fn every_shipped_recipe_round_trips() {
    let files = json_files(&assets_dir().join("minecraft/recipe"));
    recipe_set().scope(|| {
        for path in &files {
            let bytes = std::fs::read(path).unwrap();
            let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let recipe: Recipe = serde_json::from_slice(&bytes)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert_eq!(
                reencode(&recipe),
                raw,
                "{} does not round-trip",
                path.display()
            );
        }
    });
    assert_eq!(files.len(), 2042);
}

#[test]
fn a_malformed_recipe_is_refused_naming_the_fault() {
    let cases = [
        (
            r###"{"type":"minecraft:crafting_shaped","key":{"#":"minecraft:stick"},"pattern":["#","X"],"result":"minecraft:stick"}"###,
            "Pattern references symbol 'X' but it's not defined in the key",
        ),
        (
            r###"{"type":"minecraft:crafting_shaped","key":{"#":"minecraft:stick","X":"minecraft:dirt"},"pattern":["#"],"result":"minecraft:stick"}"###,
            "Key defines symbols that aren't used in pattern: [X]",
        ),
        (
            r###"{"type":"minecraft:crafting_shaped","key":{"#":"minecraft:stick"},"pattern":["#","##"],"result":"minecraft:stick"}"###,
            "Invalid pattern: each row must be the same width",
        ),
        (
            r###"{"type":"minecraft:crafting_shaped","key":{"##":"minecraft:stick"},"pattern":["#"],"result":"minecraft:stick"}"###,
            "is an invalid symbol (must be 1 character only)",
        ),
        (
            r###"{"type":"minecraft:crafting_shapeless","ingredients":[],"result":"minecraft:stick"}"###,
            "List is too short: 0, expected range [1-9]",
        ),
        (
            r###"{"type":"minecraft:crafting_shapeless","ingredients":["minecraft:stick","minecraft:stick","minecraft:stick","minecraft:stick","minecraft:stick","minecraft:stick","minecraft:stick","minecraft:stick","minecraft:stick","minecraft:stick"],"result":"minecraft:stick"}"###,
            "List is too long: 10, expected range [1-9]",
        ),
        (
            r###"{"type":"minecraft:crafting_shapeless","ingredients":["minecraft:air"],"result":"minecraft:stick"}"###,
            "Ingredient can't contain air",
        ),
        (
            r###"{"type":"minecraft:crafting_transmute","input":"minecraft:stick","material":"minecraft:dirt","material_count":9,"result":"minecraft:stick"}"###,
            "Range must be within [1..8], but was [9..9]",
        ),
    ];
    for (json, expected) in cases {
        let error = recipe_set()
            .scope(|| serde_json::from_str::<Recipe>(json))
            .expect_err(json)
            .to_string();
        assert!(
            error.contains(expected),
            "{error:?} should say {expected:?}"
        );
    }
}
