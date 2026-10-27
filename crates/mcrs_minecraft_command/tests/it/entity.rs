use crate::{GUEST, OP, read};
use glam::DVec3;
use mcrs_command::{LibraryError, Span, StringSpan, SuggestionBuilder, SyntaxError, build_list};
use mcrs_minecraft_command::argument::entity::Target;
use mcrs_minecraft_command::{ArgumentError, Candidate, EntityArgument, NoLookup, Source};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_protocol::GameMode;
use uuid::Uuid;

fn kind(input: &str) -> Result<(), ArgumentError> {
    kind_of(&EntityArgument::ENTITIES, input)
}

fn kind_of(argument: &EntityArgument, input: &str) -> Result<(), ArgumentError> {
    read(argument, input, &OP)
        .0
        .map(drop)
        .map_err(|error| match error.kind {
            mcrs_command::ErrorKind::Argument(kind) => kind,
            mcrs_command::ErrorKind::Library(kind) => panic!("{input}: {kind:?}"),
        })
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
        ("0-0-0-0-0", 0),
        ("0-0-0-0-1", 1),
    ];
    for (input, expected) in cases {
        let (value, _) = read(&EntityArgument::ENTITIES, input, &OP);
        match value.unwrap().target {
            Target::Uuid(uuid) => assert_eq!(uuid, Uuid::from_u128(expected), "{input}"),
            other => panic!("{input}: {other:?}"),
        }
    }
    for input in [
        "+1-+2-3-4-5",
        "fffffffff-0-0-0-0",
        "00000000-0000-0000-0000-0000000000000",
    ] {
        assert_eq!(kind(input), Err(ArgumentError::EntityInvalid), "{input}");
    }
    let (name, _) = read(&EntityArgument::ENTITIES, "0-0-0-0", &OP);
    assert!(matches!(name.unwrap().target, Target::Name(_)));
    assert_eq!(kind("abcdefghijklmnopq"), Err(ArgumentError::EntityInvalid));
}

#[test]
fn options_follow_the_grammar_of_the_brackets() {
    let unterminated = Err(ArgumentError::OptionsUnterminated);
    let cases: [(&str, Result<(), ArgumentError>); 8] = [
        ("@e[]", Ok(())),
        ("@e[ type = cow ]", Ok(())),
        ("@e[type=cow,]", Ok(())),
        ("@e[type=cow", unterminated),
        ("@e[type=cow x]", unterminated),
        ("@e[type=!cow,type=!pig]", Ok(())),
        ("@e[limit=0]", Err(ArgumentError::LimitTooSmall)),
        ("@e[distance=..]", Err(ArgumentError::RangeEmpty)),
    ];
    for (input, expected) in cases {
        assert_eq!(kind(input), expected, "{input}");
    }
    let any = Span::new(0, 0);
    let key = StringSpan {
        span: any,
        quoted: false,
    };
    let by_kind = [
        ("@e[,type=cow]", ArgumentError::OptionsUnknown(key)),
        ("@e[sort=up]", ArgumentError::SortIrreversible(any)),
        (
            "@e[type=cow,type=pig]",
            ArgumentError::OptionsInapplicable(key),
        ),
        (
            "@e[type=!cow,type=pig]",
            ArgumentError::OptionsInapplicable(key),
        ),
        (
            "@e[type=#a,type=!#a]",
            ArgumentError::OptionsInapplicable(key),
        ),
        (
            "@e[limit=1,limit=2]",
            ArgumentError::OptionsInapplicable(key),
        ),
        ("@s[limit=1]", ArgumentError::OptionsInapplicable(key)),
        ("@e[type=foo]", ArgumentError::TypeInvalid(any)),
    ];
    for (input, expected) in by_kind {
        assert_eq!(
            kind(input).map_err(|error| core::mem::discriminant(&error)),
            Err(core::mem::discriminant(&expected)),
            "{input}"
        );
    }
    assert_eq!(kind("@e[distance=5..1]"), Err(ArgumentError::RangeSwapped));
    assert_eq!(kind("@e[y_rotation=350..10]"), Ok(()));
    assert_eq!(kind("@x"), Err(ArgumentError::SelectorUnknown('x')));
    assert_eq!(kind("@"), Err(ArgumentError::SelectorMissing));
    let (error, _) = read(&EntityArgument::ENTITIES, "@e[x=a]", &OP);
    assert_eq!(
        error.unwrap_err(),
        SyntaxError::library(LibraryError::ExpectedDouble, 5)
    );
}

#[test]
fn the_argument_form_limits_the_selector() {
    let cases = [
        (
            EntityArgument::ONE_ENTITY,
            "@e",
            Err(ArgumentError::EntityTooMany),
        ),
        (
            EntityArgument::ONE_PLAYER,
            "@a",
            Err(ArgumentError::PlayerTooMany),
        ),
        (
            EntityArgument::PLAYERS,
            "@e",
            Err(ArgumentError::PlayerEntities),
        ),
        (
            EntityArgument::PLAYERS,
            "0-0-0-0-1",
            Err(ArgumentError::PlayerEntities),
        ),
        (EntityArgument::PLAYERS, "@e[type=player]", Ok(())),
        (EntityArgument::PLAYERS, "@s", Ok(())),
        (EntityArgument::ONE_ENTITY, "@e[limit=1]", Ok(())),
    ];
    for (argument, input, expected) in cases {
        assert_eq!(kind_of(&argument, input), expected, "{input}");
    }
    let (value, _) = read(&EntityArgument::ENTITIES, "@a", &GUEST);
    assert_eq!(
        value.unwrap_err(),
        SyntaxError::argument(ArgumentError::SelectorNotAllowed, 0)
    );
    assert!(read(&EntityArgument::ENTITIES, "Steve", &GUEST).0.is_ok());
}

struct Mob {
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
    fn bounding_box(&self) -> (DVec3, DVec3) {
        (
            self.position - DVec3::new(0.3, 0.0, 0.3),
            self.position + DVec3::new(0.3, 1.8, 0.3),
        )
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

fn world() -> Vec<Mob> {
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

fn names(input: &str) -> Vec<&'static str> {
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
    let mut names: Vec<_> = out.iter().map(|mob| mob.name).collect();
    if !input.contains("sort") && !input.contains("limit") && !input.starts_with("@p") {
        names.sort_unstable();
    }
    names
}

#[test]
fn a_search_applies_the_kind_and_the_options() {
    let cases: [(&str, &[&str]); 14] = [
        ("@a", &["Alex", "Far", "Steve"]),
        ("@e[limit=3]", &["Steve", "Alex", "Pig"]),
        ("@a[limit=3]", &["Steve", "Alex", "Far"]),
        ("@a[distance=0..]", &["Alex", "Steve"]),
        ("@e[sort=nearest,x=1,limit=2]", &["Pig", "Alex"]),
        ("@e", &["Alex", "Far", "Pig", "Steve"]),
        ("@p", &["Far"]),
        ("@e[type=!player]", &["Pig"]),
        ("@a[gamemode=!creative]", &["Alex", "Far"]),
        ("@e[distance=..5]", &["Alex", "Pig"]),
        ("@e[sort=furthest,limit=2]", &["Steve", "Alex"]),
        ("@s", &["Steve"]),
        ("alex", &["Alex"]),
        ("@a[x=2,y=64,z=0,dx=1,dy=0,dz=0]", &["Alex"]),
    ];
    for (input, expected) in cases {
        assert_eq!(names(input), expected, "{input}");
    }
    let (selector, _) = read(&EntityArgument::ENTITIES, "@e[predicate=x]", &OP);
    let world = world();
    let mut out = Vec::new();
    let found =
        selector
            .unwrap()
            .select::<Mob, NoLookup>(&OP, None, world.iter(), &mut |_| 0, &mut out);
    assert!(found.is_err());
}

fn suggestions(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    EntityArgument::ENTITIES.suggest(&OP, &mut SuggestionBuilder::new(input, 0, &mut out));
    build_list(input, &mut out)
        .iter()
        .map(|(text, _)| text.chars().collect())
        .collect()
}

#[test]
fn suggestions_follow_the_place_where_the_read_stopped() {
    let cases: [(&str, &[&str]); 9] = [
        ("", &["@a", "@e", "@n", "@p", "@r", "@s"]),
        (
            "@a[gamemode=",
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
        ("@a[gamemode=!c", &["!creative"]),
        (
            "@a[gamemode=!survival,gamemode=",
            &["!adventure", "!creative", "!spectator", "!survival"],
        ),
        ("@e[type=armadi", &["minecraft:armadillo"]),
        ("@e[type=ARMADI", &["minecraft:armadillo"]),
        ("@e[type=!armadi", &["!minecraft:armadillo"]),
        ("@e[type=!cow,type=armadi", &[]),
        ("@e[type=!cow,type=!armadi", &["!minecraft:armadillo"]),
    ];
    for (input, expected) in cases {
        assert_eq!(suggestions(input), expected, "{input}");
    }
    for input in ["@e[", "@e[ ", "@e[type=cow,"] {
        let keys = suggestions(input);
        assert!(keys.iter().any(|key| key == "limit="), "{input}: {keys:?}");
        assert!(!keys.iter().any(|key| key == "]"), "{input}: {keys:?}");
    }
}
