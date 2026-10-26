use crate::{OP, read};
use glam::DVec3;
use mcrs_command::{LibraryError, Span, SuggestionBuilder, SyntaxError, build_list};
use mcrs_minecraft_command::argument::coordinates::{Coordinates, WorldCoordinate};
use mcrs_minecraft_command::{
    ArgumentError, EntityAnchorArgument, GameModeArgument, Identifier, ResourceLocationArgument,
    RotationArgument, Vec3Argument, error_feedback,
};
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::text::Text;

fn absolute(value: f64) -> WorldCoordinate {
    WorldCoordinate {
        value,
        relative: false,
    }
}

fn relative(value: f64) -> WorldCoordinate {
    WorldCoordinate {
        value,
        relative: true,
    }
}

#[test]
fn vec3_reads_world_and_local_coordinates() {
    let vec3 = Vec3Argument;
    let cases = [
        (
            "1 2 3",
            Ok(Coordinates::World([
                absolute(1.5),
                absolute(2.0),
                absolute(3.5),
            ])),
        ),
        (
            "5.0 -0 -0",
            Ok(Coordinates::World([
                absolute(5.0),
                absolute(-0.0),
                absolute(0.5),
            ])),
        ),
        (
            "~ ~1 ~-2.5",
            Ok(Coordinates::World([
                relative(0.0),
                relative(1.0),
                relative(-2.5),
            ])),
        ),
        ("^ ^ ^5", Ok(Coordinates::Local([0.0, 0.0, 5.0]))),
        (
            "~ ^ ~",
            Err(SyntaxError::argument(ArgumentError::PosMixed, 2)),
        ),
        (
            "^ ~ ^",
            Err(SyntaxError::argument(ArgumentError::PosMixed, 0)),
        ),
        (
            "1 2",
            Err(SyntaxError::argument(ArgumentError::Pos3dIncomplete, 0)),
        ),
        (
            "1 2 ",
            Err(SyntaxError::argument(ArgumentError::PosMissingDouble, 4)),
        ),
        (
            "1  2 3",
            Err(SyntaxError::library(LibraryError::ExpectedDouble, 2)),
        ),
        (
            "~x ~ ~",
            Err(SyntaxError::library(LibraryError::ExpectedDouble, 1)),
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(read(&vec3, input, &OP).0, expected, "{input}");
    }
}

#[test]
fn vec3_completes_the_fields_that_the_input_lacks() {
    let cases: [(&str, &[&str]); 5] = [
        ("", &["~", "~ ~", "~ ~ ~"]),
        ("1 ", &["1 ~", "1 ~ ~"]),
        ("1 2", &["1 2 ~"]),
        ("^", &["^ ^", "^ ^ ^"]),
        ("1x", &[]),
    ];
    for (input, expected) in cases {
        let mut out = Vec::<mcrs_command::Suggestion<'_, ()>>::new();
        Vec3Argument.suggest(&mut SuggestionBuilder::new(input, 0, &mut out));
        let texts: Vec<String> = build_list(input, &mut out)
            .iter()
            .map(|(text, _)| text.chars().collect())
            .collect();
        assert_eq!(texts, expected, "{input}");
    }
}

#[test]
fn local_coordinates_follow_the_rotation_of_the_source() {
    let source = mcrs_minecraft_command::Source {
        position: DVec3::new(0.5, 64.0, 0.5),
        ..OP
    };
    let forwards = Coordinates::Local([0.0, 0.0, 1.0]).position(&source);
    assert!(
        (forwards - DVec3::new(0.5, 64.0, 1.5)).abs().max_element() < 1e-6,
        "{forwards}"
    );
    let left = Coordinates::Local([1.0, 0.0, 0.0]).position(&source);
    assert!(
        (left - DVec3::new(1.5, 64.0, 0.5)).abs().max_element() < 1e-6,
        "{left}"
    );
    let turned = mcrs_minecraft_command::Source {
        yaw: 90.0,
        ..source
    };
    let west = Coordinates::Local([0.0, 0.0, 2.0]).position(&turned);
    assert!(
        (west - DVec3::new(-1.5, 64.0, 0.5)).abs().max_element() < 1e-6,
        "{west}"
    );
}

#[test]
fn rotation_needs_two_world_coordinates() {
    let cases = [
        ("~15 ~-5", None),
        (
            "90",
            Some(SyntaxError::argument(ArgumentError::RotationIncomplete, 0)),
        ),
        (
            "^ ^",
            Some(SyntaxError::argument(ArgumentError::PosMixed, 0)),
        ),
    ];
    for (input, error) in cases {
        assert_eq!(
            read(&RotationArgument, input, &OP).0.err(),
            error,
            "{input}"
        );
    }
    let source = mcrs_minecraft_command::Source {
        yaw: 30.0,
        pitch: 10.0,
        ..OP
    };
    let (value, _) = read(&RotationArgument, "~15 ~-5", &OP);
    assert_eq!(value.unwrap().resolve(&source), (45.0, 5.0));
}

#[test]
fn identifiers_default_their_namespace() {
    let id = |namespace, path| Identifier { namespace, path };
    let cases = [
        ("stone", Ok(id("minecraft", "stone")), 5),
        ("Stone", Ok(id("minecraft", "")), 0),
        (
            "a:b:c",
            Err(SyntaxError::argument(ArgumentError::IdInvalid, 0)),
            0,
        ),
        (":x", Ok(id("minecraft", "x")), 2),
        ("stone[x]", Ok(id("minecraft", "stone")), 5),
    ];
    for (input, expected, cursor) in cases {
        assert_eq!(
            read(&ResourceLocationArgument, input, &OP),
            (expected, cursor),
            "{input}"
        );
    }
}

#[test]
fn keywords_fail_at_their_own_positions() {
    assert_eq!(
        read(&GameModeArgument, "creative", &OP).0,
        Ok(GameMode::Creative)
    );
    assert_eq!(
        read(&GameModeArgument, "Creative", &OP).0,
        Err(SyntaxError::argument(
            ArgumentError::GameModeInvalid(Span::new(0, 8)),
            8
        ))
    );
    assert_eq!(
        read(&GameModeArgument, "creativ", &OP).0,
        Err(SyntaxError::argument(
            ArgumentError::GameModeInvalid(Span::new(0, 7)),
            7
        ))
    );
    assert_eq!(
        read(&EntityAnchorArgument, "foo", &OP),
        (
            Err(SyntaxError::argument(
                ArgumentError::AnchorInvalid(Span::new(0, 3)),
                0
            )),
            0
        )
    );
}

#[test]
fn an_error_shows_the_ten_code_units_before_it() {
    let input = "gamemode creativ";
    let error = SyntaxError::argument(ArgumentError::GameModeInvalid(Span::new(9, 16)), 16);
    let [message, context] = error_feedback(input, &error);
    assert_eq!(
        message,
        mcrs_minecraft_protocol::text::IntoText::color(
            Text::translate("argument.gamemode.invalid", vec!["creativ".into()]),
            mcrs_minecraft_protocol::text::Color::RED
        )
    );
    let plain = context.to_legacy_lossy();
    assert!(plain.contains("...de creativ"), "{plain}");
}
