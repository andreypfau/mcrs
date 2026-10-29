use crate::{GUEST, OP, read};
use glam::DVec3;
use mcrs_command::Span;
use mcrs_command::{LibraryError, StringSpan, SuggestionBuilder, SuggestionList, SyntaxError};
use mcrs_minecraft_command::{ErrorText, Source};
use mcrs_minecraft_command_entity::{Candidate, EntityArgument, EntityBounds, EntityError, Target};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::text::{IntoText, Text};
use uuid::Uuid;

fn parsed(argument: &EntityArgument, input: &str) -> Result<(), SyntaxError<EntityError>> {
    read(argument, input, &OP).0.map(drop)
}

fn invalid(error: EntityError, position: usize) -> Result<(), SyntaxError<EntityError>> {
    Err(SyntaxError::argument(error, position))
}

fn key(start: usize, end: usize) -> StringSpan {
    StringSpan {
        span: Span::new(start, end),
        quoted: false,
    }
}

#[test]
fn uuid_text_has_five_hexadecimal_groups() {
    let cases = [
        (
            "dd12be42-52a9-4a91-a8a1-11c01849e498",
            0xdd12be42_52a9_4a91_a8a1_11c01849e498,
        ),
        (
            "DD12BE42-52A9-4A91-A8A1-11C01849E498",
            0xdd12be42_52a9_4a91_a8a1_11c01849e498,
        ),
        ("00000000-0000-0000-0000-000000000001", 1),
    ];
    for (input, expected) in cases {
        let (value, _) = read(&EntityArgument::ENTITIES, input, &OP);
        match value.unwrap().target {
            Target::Uuid(uuid) => assert_eq!(uuid, Uuid::from_u128(expected), "{input}"),
            other => panic!("{input}: {other:?}"),
        }
    }
    for input in [
        "0-0-0-0-1",
        "+1-+2-3-4-5",
        "fffffffff-0-0-0-0",
        "\"urn:uuid:00000000-0000-0000-0000-000000000001\"",
        "00000000-0000-0000-0000-0000000000000",
        "abcdefghijklmnopq",
        "\"\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\"",
    ] {
        let (value, cursor) = read(&EntityArgument::ENTITIES, input, &OP);
        assert_eq!(
            (value.map(drop), cursor),
            (invalid(EntityError::EntityInvalid, 0), 0),
            "{input}"
        );
    }
    let (name, _) = read(&EntityArgument::ENTITIES, "0-0-0-0", &OP);
    assert!(matches!(name.unwrap().target, Target::Name(_)));
}

#[test]
fn options_follow_the_grammar_of_the_brackets() {
    use EntityError as E;
    let nbt = |kind, position| {
        invalid(
            E::Nbt(mcrs_minecraft_snbt::SnbtError { kind, position }),
            position,
        )
    };
    let cases = [
        ("@e[]", Ok(())),
        ("@e[nbt={a:1},nbt=!{b:[2]}]", Ok(())),
        ("@e[ type = cow ]", Ok(())),
        ("@e[type=cow,]", Ok(())),
        ("@e[type=!cow,type=!pig]", Ok(())),
        ("@e[y_rotation=350..10]", Ok(())),
        ("@e[type=cow", invalid(E::OptionsUnterminated, 11)),
        ("@e[type=cow x]", invalid(E::OptionsUnterminated, 12)),
        ("@e[,type=cow]", invalid(E::OptionsUnknown(key(3, 3)), 3)),
        (
            "@e[sort=up]",
            invalid(E::SortIrreversible(Span::new(8, 10)), 8),
        ),
        (
            "@e[type=cow,type=pig]",
            invalid(E::OptionsInapplicable(key(12, 16)), 12),
        ),
        (
            "@e[type=!cow,type=pig]",
            invalid(E::OptionsInapplicable(key(13, 17)), 18),
        ),
        (
            "@e[type=#a,type=!#a]",
            invalid(E::OptionsInapplicable(key(11, 15)), 16),
        ),
        (
            "@e[limit=1,limit=2]",
            invalid(E::OptionsInapplicable(key(11, 16)), 11),
        ),
        ("@s[limit=1]", invalid(E::OptionsInapplicable(key(3, 8)), 3)),
        ("@e[type=foo]", invalid(E::TypeInvalid(Span::new(8, 11)), 8)),
        (
            "@e[nbt=5]",
            nbt(mcrs_minecraft_snbt::SnbtErrorKind::ExpectedCompound, 8),
        ),
        ("@e[limit=0]", invalid(E::LimitTooSmall, 9)),
        ("@e[distance=..]", invalid(E::RangeEmpty, 12)),
        ("@e[distance=5..1]", invalid(E::RangeSwapped, 12)),
        ("@x", invalid(E::SelectorUnknown('x'), 1)),
        ("@", invalid(E::SelectorMissing, 1)),
        (
            "@e[x=a]",
            Err(SyntaxError::library(LibraryError::ExpectedDouble, 5)),
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(
            parsed(&EntityArgument::ENTITIES, input),
            expected,
            "{input}"
        );
    }
}

#[test]
fn the_argument_form_limits_the_selector() {
    let cases = [
        (
            EntityArgument::ONE_ENTITY,
            "@e",
            invalid(EntityError::EntityTooMany, 0),
        ),
        (
            EntityArgument::ONE_PLAYER,
            "@a",
            invalid(EntityError::PlayerTooMany, 0),
        ),
        (
            EntityArgument::PLAYERS,
            "@e",
            invalid(EntityError::PlayerEntities, 0),
        ),
        (
            EntityArgument::PLAYERS,
            "00000000-0000-0000-0000-000000000001",
            invalid(EntityError::PlayerEntities, 0),
        ),
        (EntityArgument::PLAYERS, "@e[type=player]", Ok(())),
        (EntityArgument::PLAYERS, "@s", Ok(())),
        (EntityArgument::ONE_ENTITY, "@e[limit=1]", Ok(())),
    ];
    for (argument, input, expected) in cases {
        assert_eq!(parsed(&argument, input), expected, "{input}");
    }
    let (value, _) = read(&EntityArgument::ENTITIES, "@a", &GUEST);
    assert_eq!(
        value.unwrap_err(),
        SyntaxError::argument(EntityError::SelectorNotAllowed, 0)
    );
    assert!(read(&EntityArgument::ENTITIES, "Steve", &GUEST).0.is_ok());
}

pub(crate) struct Mob {
    uuid: Uuid,
    name: &'static str,
    entity_type: EntityType,
    position: DVec3,
    mode: Option<GameMode>,
    dimension: usize,
}

impl Candidate for Mob {
    fn uuid(&self) -> Uuid {
        self.uuid
    }
    fn name(&self) -> &str {
        self.name
    }
    fn entity_type(&self) -> EntityType {
        self.entity_type
    }
    fn position(&self) -> DVec3 {
        self.position
    }
    fn bounding_box(&self) -> EntityBounds {
        EntityBounds {
            min: self.position - DVec3::new(0.3, 0.0, 0.3),
            max: self.position + DVec3::new(0.3, 1.8, 0.3),
        }
    }
    fn yaw(&self) -> f32 {
        0.0
    }
    fn pitch(&self) -> f32 {
        0.0
    }
    fn game_mode(&self) -> Option<GameMode> {
        self.mode
    }
    fn experience_level(&self) -> Option<i32> {
        self.mode.map(|_| 0)
    }
    fn alive(&self) -> bool {
        true
    }
    fn has_tag(&self, _: &str) -> bool {
        false
    }
    fn has_any_tag(&self) -> bool {
        false
    }
    fn team(&self) -> Option<&str> {
        None
    }
    fn dimension(&self) -> usize {
        self.dimension
    }
}

pub(crate) fn world() -> Vec<Mob> {
    let player = |n: u128, name, x, mode, dimension| Mob {
        uuid: Uuid::from_u128(n),
        name,
        entity_type: EntityType::Player,
        position: DVec3::new(x, 64.0, 0.0),
        mode: Some(mode),
        dimension,
    };
    vec![
        player(1, "Steve", 10.0, GameMode::Creative, 0),
        player(2, "Alex", 3.0, GameMode::Survival, 0),
        player(3, "Far", 1.0, GameMode::Survival, 1),
        Mob {
            uuid: Uuid::from_u128(4),
            name: "Pig",
            entity_type: EntityType::Pig,
            position: DVec3::new(0.0, 64.0, 0.0),
            mode: None,
            dimension: 0,
        },
    ]
}

#[derive(Clone, Copy)]
enum Order {
    Exact,
    Unordered,
    /// Each candidate of an earlier dimension first; unordered inside one dimension.
    ByDimension,
}

fn names(input: &str, order: Order) -> Vec<&'static str> {
    let world = world();
    let source = Source {
        position: DVec3::new(0.0, 64.0, 0.0),
        ..OP
    };
    let (selector, _) = read(&EntityArgument::ENTITIES, input, &source);
    let mut out = Vec::new();
    selector
        .unwrap()
        .select(&source, world.first(), world.iter(), &mut |_| 0, &mut out)
        .unwrap();
    match order {
        Order::Exact => {}
        Order::Unordered => out.sort_unstable_by_key(|mob| mob.name),
        Order::ByDimension => {
            assert!(
                out.is_sorted_by_key(|mob| mob.dimension),
                "{input}: the dimensions are out of order"
            );
            out.sort_unstable_by_key(|mob| (mob.dimension, mob.name));
        }
    }
    out.iter().map(|mob| mob.name).collect()
}

#[test]
fn a_search_applies_the_kind_and_the_options() {
    use Order::*;
    let cases: [(&str, Order, &[&str]); 14] = [
        ("@a", Unordered, &["Alex", "Far", "Steve"]),
        ("@e[limit=3]", ByDimension, &["Alex", "Pig", "Steve"]),
        ("@a[limit=3]", Unordered, &["Alex", "Far", "Steve"]),
        ("@a[distance=0..]", Unordered, &["Alex", "Steve"]),
        ("@e[sort=nearest,x=1,limit=2]", Exact, &["Pig", "Alex"]),
        ("@e", ByDimension, &["Alex", "Pig", "Steve", "Far"]),
        ("@p", Exact, &["Far"]),
        ("@e[type=!player]", Exact, &["Pig"]),
        ("@a[gamemode=!creative]", Unordered, &["Alex", "Far"]),
        ("@e[distance=..5]", Unordered, &["Alex", "Pig"]),
        ("@e[sort=furthest,limit=2]", Exact, &["Steve", "Alex"]),
        ("@s", Exact, &["Steve"]),
        ("alex", Exact, &["Alex"]),
        ("@a[x=2,y=64,z=0,dx=1,dy=0,dz=0]", Exact, &["Alex"]),
    ];
    for (input, order, expected) in cases {
        assert_eq!(names(input, order), expected, "{input}");
    }
}

#[test]
fn a_search_fails_for_a_missing_predicate_or_a_source_without_selectors() {
    let world = world();
    let mut out = world.iter().take(1).collect();
    let (selector, _) = read(&EntityArgument::ENTITIES, "@e[predicate=x]", &OP);
    let error = selector
        .unwrap()
        .select(&OP, None, world.iter(), &mut |_| 0, &mut out)
        .unwrap_err();
    assert_eq!(
        error.text("@e[predicate=x]"),
        Text::translate(
            "argument.resource.not_found",
            vec!["minecraft:x".into_text(), "minecraft:predicate".into_text()],
        )
    );
    assert!(out.is_empty());

    let (selector, _) = read(&EntityArgument::ENTITIES, "@a", &OP);
    let found = selector
        .unwrap()
        .select(&GUEST, None, world.iter(), &mut |_| 0, &mut out);
    assert_eq!(found, Err(EntityError::SelectorNotAllowed));
    let (selector, _) = read(&EntityArgument::ENTITIES, "steve", &OP);
    let found = selector
        .unwrap()
        .select(&GUEST, None, world.iter(), &mut |_| 0, &mut out);
    assert_eq!(found, Ok(()));
    assert_eq!(
        out.iter().map(|mob| mob.name).collect::<Vec<_>>(),
        ["Steve"]
    );
}

fn suggestions(input: &str) -> (Span, Vec<String>) {
    let mut out = Vec::new();
    EntityArgument::ENTITIES.suggest(&OP, &mut SuggestionBuilder::new(input, 0, &mut out));
    let list = SuggestionList::build(input, &mut out);
    let texts = list
        .iter()
        .map(|(text, _)| text.chars().collect())
        .collect();
    (list.span, texts)
}

#[test]
fn suggestions_follow_the_place_where_the_read_stopped() {
    let cases: [(&str, usize, &[&str]); 8] = [
        ("", 0, &["@a", "@e", "@n", "@p", "@r", "@s"]),
        (
            "@a[gamemode=",
            12,
            &[
                "!adventure",
                "!creative",
                "!spectator",
                "!survival",
                "adventure",
                "creative",
                "spectator",
                "survival",
            ],
        ),
        ("@a[gamemode=!c", 12, &["!creative"]),
        (
            "@a[gamemode=!survival,gamemode=",
            31,
            &["!adventure", "!creative", "!spectator", "!survival"],
        ),
        ("@e[type=armadi", 8, &["minecraft:armadillo"]),
        ("@e[type=ARMADI", 8, &["minecraft:armadillo"]),
        ("@e[type=!armadi", 8, &["!minecraft:armadillo"]),
        ("@e[type=!cow,type=!armadi", 18, &["!minecraft:armadillo"]),
    ];
    for (input, start, expected) in cases {
        assert_eq!(
            suggestions(input),
            (
                Span::new(start, input.len()),
                expected.iter().map(|text| text.to_string()).collect()
            ),
            "{input}"
        );
    }
    assert_eq!(
        suggestions("@e[type=!cow,type=armadi").1,
        Vec::<String>::new()
    );
    let keys: [(&str, usize, &[&str], &[&str]); 4] = [
        ("@e[", 3, &["limit=", "sort=", "type="], &["]"]),
        ("@e[ ", 4, &["limit=", "type="], &["]"]),
        ("@e[type=cow,", 12, &["limit=", "tag="], &["]", "type="]),
        ("@s[", 3, &["type=", "tag="], &["]", "limit=", "sort="]),
    ];
    for (input, start, present, absent) in keys {
        let (span, keys) = suggestions(input);
        assert_eq!(span, Span::new(start, input.len()), "{input}");
        for key in present {
            assert!(keys.iter().any(|k| k == key), "{input}: {key} in {keys:?}");
        }
        for key in absent {
            assert!(!keys.iter().any(|k| k == key), "{input}: {key} in {keys:?}");
        }
    }
}
