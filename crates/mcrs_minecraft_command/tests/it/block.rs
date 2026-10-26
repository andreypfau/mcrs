use mcrs_command::{
    Argument, LibraryError, Reader, Suggestion, SuggestionBuilder, SyntaxError, build_list,
};
use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_block::definition::schema::{BlockProperties, BlockProperty, PropertyValue};
use mcrs_minecraft_command::{
    BlockPredicate, BlockPredicateArgument, CommandLevel, CommandSyntaxError, Identifier, Lookup,
    Source, error_message,
};
use mcrs_minecraft_core::{BlockStateId, StaticResourceLocation};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use std::sync::LazyLock;

/// `oak_stairs` and `wheat` with their vanilla properties, `stone`, `chest` with a block
/// entity, and the tag `logs` of `oak_log`.
struct Blocks {
    stairs: Vec<BlockProperty>,
    wheat: Vec<BlockProperty>,
    log: Vec<BlockProperty>,
}

fn words(values: &[&str]) -> Vec<PropertyValue> {
    values
        .iter()
        .map(|value| PropertyValue::Str((*value).into()))
        .collect()
}

fn property(name: &str, values: Vec<PropertyValue>) -> BlockProperty {
    BlockProperty {
        name: name.into(),
        values,
    }
}

static BLOCKS: LazyLock<Blocks> = LazyLock::new(|| Blocks {
    stairs: vec![
        property("facing", words(&["north", "south", "west", "east"])),
        property("half", words(&["top", "bottom"])),
        property("shape", words(&["straight", "inner_left"])),
        property(
            "waterlogged",
            vec![PropertyValue::Bool(true), PropertyValue::Bool(false)],
        ),
    ],
    wheat: vec![property("age", (0..=7).map(PropertyValue::Int).collect())],
    log: vec![property("axis", words(&["x", "y", "z"]))],
});

impl Lookup for Blocks {
    fn entries(&self, _: StaticResourceLocation) -> impl Iterator<Item = &str> {
        [
            "minecraft:oak_stairs",
            "minecraft:wheat",
            "minecraft:oak_log",
            "minecraft:stone",
            "minecraft:chest",
        ]
        .into_iter()
    }

    fn tags(&self, _: StaticResourceLocation) -> impl Iterator<Item = &str> {
        ["minecraft:logs"].into_iter()
    }

    fn tag_contains(
        &self,
        _: StaticResourceLocation,
        _: Identifier<'_>,
        _: StaticResourceLocation,
    ) -> bool {
        false
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn tag_entries(
        &self,
        _: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> impl Iterator<Item = &str> {
        tag.eq_str("minecraft:logs")
            .then_some("minecraft:oak_log")
            .into_iter()
    }

    fn block_properties(&self, block: Identifier<'_>) -> Option<&[BlockProperty]> {
        [
            ("minecraft:oak_stairs", self.stairs.as_slice()),
            ("minecraft:wheat", self.wheat.as_slice()),
            ("minecraft:oak_log", self.log.as_slice()),
            ("minecraft:stone", &[]),
            ("minecraft:chest", &[]),
        ]
        .into_iter()
        .find(|(name, _)| block.eq_str(name))
        .map(|(_, properties)| properties)
    }

    fn has_block_entity(&self, block: Identifier<'_>) -> bool {
        block.eq_str("minecraft:chest")
    }
}

fn source() -> Source<'static, Blocks> {
    Source::new(CommandLevel::Gamemasters, &BLOCKS)
}

pub(crate) fn read(input: &str) -> (Result<BlockPredicate<'_>, CommandSyntaxError>, usize) {
    let mut reader = Reader::new(input);
    let value = BlockPredicateArgument.parse(&mut reader, &source());
    (value, reader.cursor())
}

fn message(input: &str) -> Result<(Text, usize), usize> {
    match read(input).0 {
        Ok(_) => Err(read(input).1),
        Err(error) => Ok((error_message(input, &error), error.position)),
    }
}

fn key(key: &'static str, with: &[&str]) -> Text {
    Text::translate(
        key,
        with.iter()
            .map(|&text| text.to_owned().into_text())
            .collect::<Vec<Text>>(),
    )
}

#[test]
fn a_predicate_reads_a_block_or_a_tag_and_its_property_list() {
    let cases: [(&str, usize); 11] = [
        ("stone", 5),
        ("minecraft:stone", 15),
        ("stone []", 5),
        ("#logs[axis=y]{}", 15),
        ("stone{}", 7),
        ("stone{}[]", 7),
        ("chest{Items:[{id:\"minecraft:stone\"}]} x", 37),
        ("oak_stairs[ facing = north ]", 28),
        ("oak_stairs[facing=north,]", 25),
        ("oak_stairs[\"facing\"=\"north\"]", 28),
        ("#logs[foo=bar]", 14),
    ];
    for (input, cursor) in cases {
        let (value, end) = read(input);
        assert!(value.is_ok(), "{input}: {value:?}");
        assert_eq!(end, cursor, "{input}");
    }
}

#[test]
fn a_predicate_fails_with_the_key_and_the_position_of_its_rule() {
    let unclosed = key("argument.block.property.unclosed", &[]);
    let novalue = |name: &str, of: &str| key("argument.block.property.novalue", &[name, of]);
    let cases = [
        (
            "nope",
            key("argument.block.id.invalid", &["minecraft:nope"]),
            0,
        ),
        (
            "Stone",
            key("argument.block.id.invalid", &["minecraft:"]),
            0,
        ),
        (
            "#minecraft:nope",
            key("arguments.block.tag.unknown", &["minecraft:nope"]),
            0,
        ),
        (
            "oak_stairs[facing=north, ]",
            key(
                "argument.block.property.unknown",
                &["minecraft:oak_stairs", ""],
            ),
            25,
        ),
        (
            "oak_stairs[,]",
            key(
                "argument.block.property.unknown",
                &["minecraft:oak_stairs", ""],
            ),
            11,
        ),
        (
            "oak_stairs[facing=north,facing=south]",
            key(
                "argument.block.property.duplicate",
                &["facing", "minecraft:oak_stairs"],
            ),
            24,
        ),
        (
            "oak_stairs[facing]",
            novalue("facing", "minecraft:oak_stairs"),
            17,
        ),
        (
            "oak_stairs[facing=up]",
            key(
                "argument.block.property.invalid",
                &["minecraft:oak_stairs", "up", "facing"],
            ),
            18,
        ),
        ("oak_stairs[facing=north", unclosed.clone(), 23),
        ("oak_stairs[facing=north;]", unclosed.clone(), 23),
        (
            "wheat[age=8]",
            key(
                "argument.block.property.invalid",
                &["minecraft:wheat", "8", "age"],
            ),
            10,
        ),
        (
            "wheat[age=1.0]",
            key(
                "argument.block.property.invalid",
                &["minecraft:wheat", "1.0", "age"],
            ),
            10,
        ),
        ("#logs[axis=y, ]", novalue("", "#minecraft:logs"), 14),
        ("#logs[axis]", novalue("axis", "#minecraft:logs"), 6),
        ("#logs[axis=y", unclosed.clone(), 11),
        ("#logs[axis=y ", unclosed.clone(), 11),
        ("#logs[axis=y,", unclosed.clone(), 13),
        ("#logs[", unclosed, 6),
        (
            "#logs[axis=y,axis=x]",
            key(
                "argument.block.property.duplicate",
                &["axis", "#minecraft:logs"],
            ),
            13,
        ),
    ];
    for (input, text, position) in cases {
        assert_eq!(message(input), Ok((text, position)), "{input}");
    }
    assert_eq!(
        read("oak_stairs[facing=\"north").0.err(),
        Some(SyntaxError::library(LibraryError::ExpectedEndOfQuote, 24))
    );
}

#[test]
fn a_data_compound_fails_with_the_key_and_the_position_of_the_snbt_read() {
    let cases = [
        ("stone{", key("argument.literal.incorrect", &["\""]), 6),
        ("stone{a:1", key("argument.literal.incorrect", &["."]), 9),
        (
            "#logs[axis=y]{a:[B;300]}",
            key(
                "snbt.parser.number_parse_failure",
                &["300 does not fit in a byte"],
            ),
            23,
        ),
        (
            "stone{a:foo(1)}",
            key("snbt.parser.no_such_operation", &["foo/1"]),
            14,
        ),
    ];
    for (input, text, position) in cases {
        assert_eq!(message(input), Ok((text, position)), "{input}");
    }
}

fn entry(identifier: &str, properties: Vec<BlockProperty>, base: u16) -> BlockEntry {
    let count = properties.iter().map(|p| p.values.len() as u16).product();
    BlockEntry {
        identifier: identifier.parse().unwrap(),
        base_state_id: BlockStateId::checked(base, usize::MAX).unwrap(),
        default_state_id: BlockStateId::checked(base, usize::MAX).unwrap(),
        state_count: count,
        properties: BlockProperties(properties),
        placement_filter: None,
        container_slots: None,
    }
}

#[test]
fn the_test_compares_only_the_named_properties_with_their_value_texts() {
    let wheat = entry(
        "minecraft:wheat",
        vec![property("age", (0..=7).map(PropertyValue::Int).collect())],
        100,
    );
    let age_7 = BlockStateId::checked(107, usize::MAX).unwrap();
    let log = entry(
        "minecraft:oak_log",
        vec![property("axis", words(&["x", "y", "z"]))],
        200,
    );
    let axis_x = BlockStateId::checked(200, usize::MAX).unwrap();
    let cases = [
        ("wheat", true, &wheat, age_7, true),
        ("wheat[age=+7]", true, &wheat, age_7, true),
        ("wheat[age=07]", true, &wheat, age_7, true),
        ("wheat[age=6]", true, &wheat, age_7, false),
        ("wheat", false, &wheat, age_7, false),
        ("#logs[axis=x]", true, &log, axis_x, true),
        ("#logs[axis=up]", true, &log, axis_x, false),
        ("#logs[foo=bar]", true, &log, axis_x, false),
    ];
    for (input, in_subject, block, state, passes) in cases {
        let predicate = read(input).0.unwrap();
        assert_eq!(predicate.test(in_subject, block, state), passes, "{input}");
    }
}

#[test]
fn the_data_compound_matches_the_saved_data_of_the_block_entity() {
    let items = "{Items:[{id:\"minecraft:stone\",Slot:0b},{id:\"minecraft:dirt\",Slot:1b}],x:10}";
    let cases = [
        ("stone", None, true),
        ("stone{}", None, false),
        ("stone{}", Some(items), true),
        ("stone{x:10}", Some(items), true),
        ("stone{x:10b}", Some(items), false),
        (
            "chest{Items:[{id:\"minecraft:stone\"},{id:\"minecraft:stone\"}]}",
            Some(items),
            true,
        ),
        (
            "chest{Items:[{id:\"minecraft:stone\",Slot:1b}]}",
            Some(items),
            false,
        ),
    ];
    for (input, saved, passes) in cases {
        let predicate = read(input).0.unwrap();
        let saved = || saved.map(|text| mcrs_minecraft_nbt::snbt::parse_compound(text).unwrap());
        assert_eq!(predicate.test_data(saved), passes, "{input}");
    }
}

fn suggestions(input: &str) -> Vec<String> {
    let source = source();
    let mut out: Vec<Suggestion<'_, &'static str>> = Vec::new();
    let mut builder = SuggestionBuilder::new(input, 0, &mut out);
    BlockPredicateArgument::suggest(&source, &mut builder);
    let list = build_list(input, &mut out);
    let mut texts: Vec<String> = list
        .iter()
        .map(|(text, _)| text.chars().collect())
        .collect();
    texts.sort();
    texts
}

#[test]
fn suggestions_follow_the_stage_the_read_reached() {
    let cases: [(&str, &[&str]); 11] = [
        (
            "oak_stairs[",
            &["]", "facing=", "half=", "shape=", "waterlogged="],
        ),
        ("oak_stairs[FAC", &["facing="]),
        ("oak_stairs[logged", &[]),
        ("oak_stairs[facing=nor", &["east", "north", "south", "west"]),
        ("oak_stairs[facing=north", &[",", "]"]),
        ("wheat", &["["]),
        ("#logs[axis", &["axis="]),
        ("chest", &["{"]),
        ("stone", &[]),
        ("chest{", &[]),
        ("#logs[axis=y]", &[]),
    ];
    for (input, expected) in cases {
        assert_eq!(suggestions(input), expected, "{input}");
    }
}
