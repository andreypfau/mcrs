use mcrs_minecraft_entity::variant::ChickenSoundVariant;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_snbt::parse_tag;
use mcrs_minecraft_sound::BlockSoundSet;
use mcrs_minecraft_worldgen_testing::{json_files, vanilla_dir};

use crate::common::{read_named, write_named};

#[test]
fn every_shipped_sound_set_loads_and_writes_back_byte_for_byte() {
    let files = json_files(&vanilla_dir().join("block_sound_set"));
    assert_eq!(files.len(), 137);
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let set: BlockSoundSet = read_named(test_registries(), &text)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            serde_json::to_string_pretty(&write_named(test_registries(), &set)).unwrap(),
            text,
            "{}",
            path.display()
        );
    }
}

#[test]
fn a_sound_set_is_written_as_the_game_writes_it() {
    let cases = [
        (
            r#"{"volume": 0.3, "pitch": 1, "place_sound": "minecraft:block.anvil.place"}"#,
            r#"{place_sound:"minecraft:block.anvil.place",volume:0.3f}"#,
        ),
        (r#"{"volume": 1.0, "pitch": 1.5}"#, "{pitch:1.5f}"),
        ("{}", "{}"),
        (
            r#"{"step_sound": {"sound_id": "test:clack", "range": "far", "extra": 1}}"#,
            r#"{step_sound:{sound_id:"test:clack"}}"#,
        ),
        (
            r#"{"hit_sound": {"sound_id": "test:clack", "range": 4}, "unknown": true}"#,
            r#"{hit_sound:{range:4.0f,sound_id:"test:clack"}}"#,
        ),
    ];
    for (json, snbt) in cases {
        let set: BlockSoundSet =
            read_named(test_registries(), json).unwrap_or_else(|error| panic!("{json}: {error}"));
        let named = write_named(test_registries(), &set);
        let ours = sorted(mcrs_minecraft_nbt::to_nbt_tag(&named).unwrap());
        assert_eq!(ours, sorted(parse_tag(snbt).unwrap()), "{json}");
    }
}

#[test]
fn a_malformed_sound_set_fails_to_load() {
    let cases = [
        r#"{"volume": 0}"#,
        r#"{"volume": 10.5}"#,
        r#"{"pitch": 2.5}"#,
        r#"{"volume": "loud"}"#,
        r#"{"place_sound": "minecraft:does.not.exist"}"#,
        r#"{"place_sound": {"range": 4}}"#,
    ];
    for json in cases {
        assert!(
            read_named::<BlockSoundSet>(test_registries(), json).is_err(),
            "{json} loaded"
        );
    }
    let error = read_named::<BlockSoundSet>(test_registries(), r#"{"volume": 0}"#).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("Value must be within range [1.0E-5;10.0]: 0.0"),
        "{error}"
    );
}

#[test]
fn an_entity_sound_variant_may_leave_out_any_sound() {
    let silent: ChickenSoundVariant = read_named(
        test_registries(),
        r#"{"adult_sounds": {}, "baby_sounds": {}, "unknown": 1}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&write_named(test_registries(), &silent)).unwrap(),
        r#"{"adult_sounds":{},"baby_sounds":{}}"#
    );
    assert!(
        read_named::<ChickenSoundVariant>(test_registries(), r#"{"adult_sounds": {}}"#).is_err()
    );
}

fn sorted(tag: NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(mut compound) => {
            compound.child_tags.sort_by(|(a, _), (b, _)| a.cmp(b));
            for (_, child) in &mut compound.child_tags {
                *child = sorted(std::mem::replace(child, NbtTag::End));
            }
            NbtTag::Compound(compound)
        }
        other => other,
    }
}
