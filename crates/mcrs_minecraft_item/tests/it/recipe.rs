use mcrs_minecraft_item::recipe::Recipe;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_worldgen_testing::{json_files, reencode, vanilla_dir};

#[test]
fn every_shipped_recipe_round_trips() {
    let files = json_files(&vanilla_dir().join("recipe"));
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let recipe: Recipe<Named> = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            reencode(&recipe),
            raw,
            "{} does not round-trip",
            path.display()
        );
    }
    assert_eq!(files.len(), 2055);
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
        let error = serde_json::from_str::<Recipe<Named>>(json)
            .expect_err(json)
            .to_string();
        assert!(
            error.contains(expected),
            "{error:?} should say {expected:?}"
        );
    }
}
