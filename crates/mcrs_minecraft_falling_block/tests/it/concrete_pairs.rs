use mcrs_minecraft_falling_block::ConcretePairsError;
use mcrs_minecraft_falling_block::ConcretePairsErrorKind;
use mcrs_minecraft_falling_block::ConcretePairsFile;
use mcrs_minecraft_falling_block::ConcreteTable;
use mcrs_minecraft_falling_block::PackConcretePairs;

use crate::fixture::DEFAULT_PAIRS;
use crate::fixture::DEFINITIONS;

fn file(json: &str) -> ConcretePairsFile {
    serde_json::from_str(json).unwrap()
}

fn load(packs: &[(&str, &ConcretePairsFile)]) -> Result<ConcreteTable, ConcretePairsError> {
    let packs: Vec<PackConcretePairs> = packs
        .iter()
        .map(|(pack, file)| PackConcretePairs {
            pack: (*pack).to_owned(),
            file: (*file).clone(),
        })
        .collect();
    ConcreteTable::load(&packs, &DEFINITIONS)
}

fn concrete_of(table: &ConcreteTable, powder: &str) -> Option<String> {
    let block = table.concrete(DEFINITIONS.id_of(powder).unwrap())?;
    Some(DEFINITIONS.entry(block).unwrap().identifier.to_string())
}

#[test]
fn fbk_trg_002_the_default_pairs_map_each_colour_powder_to_its_concrete() {
    let table = load(&[("mcrs", &DEFAULT_PAIRS)]).unwrap();
    let colours = [
        "white",
        "orange",
        "magenta",
        "light_blue",
        "yellow",
        "lime",
        "pink",
        "gray",
        "light_gray",
        "cyan",
        "purple",
        "blue",
        "brown",
        "green",
        "red",
        "black",
    ];
    assert_eq!(DEFAULT_PAIRS.values.len(), colours.len());
    for colour in colours {
        assert_eq!(
            concrete_of(&table, &format!("minecraft:{colour}_concrete_powder")),
            Some(format!("minecraft:{colour}_concrete")),
            "{colour}"
        );
    }
    assert_eq!(concrete_of(&table, "minecraft:sand"), None);
}

#[test]
fn fbk_trg_002_later_pack_overrides_and_replace_discards_earlier() {
    let base = file(
        r#"{"values": {"minecraft:sand": "minecraft:sandstone", "minecraft:gravel": "minecraft:stone"}}"#,
    );
    let later = file(r#"{"values": {"minecraft:sand": "minecraft:glass"}}"#);
    let replacing = file(r#"{"replace": true, "values": {"minecraft:sand": "minecraft:glass"}}"#);
    let cases = [
        (
            vec![("a", &base), ("b", &later)],
            Some("minecraft:glass"),
            Some("minecraft:stone"),
        ),
        (
            vec![("b", &later), ("a", &base)],
            Some("minecraft:sandstone"),
            Some("minecraft:stone"),
        ),
        (
            vec![("a", &base), ("b", &replacing)],
            Some("minecraft:glass"),
            None,
        ),
    ];
    for (packs, sand, gravel) in cases {
        let table = load(&packs).unwrap();
        let names: Vec<&str> = packs.iter().map(|(pack, _)| *pack).collect();
        assert_eq!(
            concrete_of(&table, "minecraft:sand").as_deref(),
            sand,
            "{names:?}"
        );
        assert_eq!(
            concrete_of(&table, "minecraft:gravel").as_deref(),
            gravel,
            "{names:?}"
        );
    }
}

#[test]
fn fbk_trg_002_unknown_or_self_mapped_block_fails_the_load() {
    let cases = [
        (
            r#"{"values": {"minecraft:no_such_powder": "minecraft:stone"}}"#,
            "minecraft:no_such_powder",
            ConcretePairsErrorKind::UnknownBlock,
        ),
        (
            r#"{"values": {"minecraft:sand": "minecraft:no_such_concrete"}}"#,
            "minecraft:no_such_concrete",
            ConcretePairsErrorKind::UnknownBlock,
        ),
        (
            r#"{"values": {"minecraft:sand": "minecraft:sand"}}"#,
            "minecraft:sand",
            ConcretePairsErrorKind::MapsToItself,
        ),
    ];
    for (json, identifier, kind) in cases {
        let broken = file(json);
        let error = load(&[("mcrs", &DEFAULT_PAIRS), ("custom", &broken)]).unwrap_err();
        assert_eq!(error.pack, "custom", "{json}");
        assert_eq!(error.identifier.as_str(), identifier, "{json}");
        assert_eq!(error.kind, kind, "{json}");
    }
}

#[test]
fn fbk_trg_002_a_file_without_values_or_with_an_unknown_key_does_not_parse() {
    for json in [r#"{"replace": true}"#, r#"{"values": {}, "extra": 1}"#] {
        assert!(
            serde_json::from_str::<ConcretePairsFile>(json).is_err(),
            "{json}"
        );
    }
}
