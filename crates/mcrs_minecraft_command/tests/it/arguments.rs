use crate::{OP, read};
use glam::{BVec3, DVec3};
use mcrs_command::{LibraryError, SuggestionBuilder, SyntaxError, build_list};
use mcrs_minecraft_command::argument::coordinates::{Coordinates, WorldCoordinate};
use mcrs_minecraft_command::{
    ArgumentError, BlockPosArgument, EntityAnchorArgument, GameModeArgument, HeightmapArgument,
    Identifier, ResourceLocationArgument, RotationArgument, SwizzleArgument, Vec3Argument, align,
    block_floor, error_feedback, rotation_toward,
};
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
use mcrs_text_span::Span;

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
fn vec3_and_block_pos_read_world_and_local_coordinates() {
    let world = |x, y, z| Ok(Coordinates::World([x, y, z]));
    let local = |x, y, z| Ok(Coordinates::Local([x, y, z]));
    let argument = |error, at| Err(SyntaxError::argument(error, at));
    let library = |error, at| Err(SyntaxError::library(error, at));
    let mixed = |at| argument(ArgumentError::PosMixed, at);
    let cases = [
        (
            "1 2 3",
            world(absolute(1.5), absolute(2.0), absolute(3.5)),
            world(absolute(1.0), absolute(2.0), absolute(3.0)),
        ),
        (
            "5.0 -0 -0",
            world(absolute(5.0), absolute(-0.0), absolute(0.5)),
            library(LibraryError::InvalidInt(Span::new(0, 3)), 0),
        ),
        (
            "-0 ~ ~",
            world(absolute(0.5), relative(0.0), relative(0.0)),
            world(absolute(0.0), relative(0.0), relative(0.0)),
        ),
        (
            "~ ~1 ~-2.5",
            world(relative(0.0), relative(1.0), relative(-2.5)),
            world(relative(0.0), relative(1.0), relative(-2.5)),
        ),
        ("^ ^ ^5", local(0.0, 0.0, 5.0), local(0.0, 0.0, 5.0)),
        ("~ ^ ~", mixed(2), mixed(2)),
        ("^ ~ ^", mixed(0), mixed(0)),
        (
            "1 2",
            argument(ArgumentError::Pos3dIncomplete, 0),
            argument(ArgumentError::Pos3dIncomplete, 0),
        ),
        (
            "1 2 ",
            argument(ArgumentError::PosMissingDouble, 4),
            argument(ArgumentError::PosMissingInt, 4),
        ),
        (
            "1  2 3",
            library(LibraryError::ExpectedDouble, 2),
            library(LibraryError::ExpectedInt, 2),
        ),
        (
            "~x ~ ~",
            library(LibraryError::ExpectedDouble, 1),
            library(LibraryError::ExpectedDouble, 1),
        ),
    ];
    for (input, vec3, block_pos) in cases {
        assert_eq!(read(&Vec3Argument, input, &OP).0, vec3, "vec3 {input}");
        assert_eq!(
            read(&BlockPosArgument, input, &OP).0,
            block_pos,
            "block_pos {input}"
        );
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
    let [message, _] = error_feedback(input, &error);
    assert_eq!(
        message,
        Text::translate("argument.gamemode.invalid", vec!["creativ".into()]).color(Color::RED)
    );

    let cases = [
        ("gamemode creativ", 16, "...de creativ", None),
        ("gamemode creativ", 9, "gamemode ", Some("creativ")),
        ("0123456789", 10, "0123456789", None),
        ("a\u{1f600}bcdefghij", 14, "...bcdefghij", None),
        ("a\u{1f600}bcdefghij x", 14, "...bcdefghij", Some(" x")),
    ];
    for (input, position, before, rest) in cases {
        let error = SyntaxError::argument(ArgumentError::PosMixed, position);
        let [_, context] = error_feedback(input, &error);
        let mut expected = before.into_text();
        if let Some(rest) = rest {
            expected = expected.add_child(rest.color(Color::RED).underlined());
        }
        let expected = expected
            .add_child(
                Text::translate("command.context.here", vec![])
                    .color(Color::RED)
                    .italic(),
            )
            .color(Color::GRAY)
            .on_click_suggest_command(format!("/{input}"));
        assert_eq!(context, expected, "{input} at {position}");
    }
}

#[test]
fn a_block_position_is_the_floor_of_the_point() {
    let source = mcrs_minecraft_command::Source {
        position: DVec3::new(10.7, 64.0, -0.3),
        ..OP
    };
    let cases = [
        ("~ ~ ~", [10, 64, -1]),
        ("~0.5 ~-1 ~", [11, 63, -1]),
        ("5 -3 7", [5, -3, 7]),
        ("~3000000000 ~ ~", [i32::MAX, 64, -1]),
    ];
    for (input, expected) in cases {
        let coordinates = read(&BlockPosArgument, input, &OP).0.unwrap();
        assert_eq!(
            block_floor(coordinates.position(&source)),
            expected,
            "{input}"
        );
    }
}

#[test]
fn swizzle_reads_a_set_of_distinct_axes() {
    let axes = |x, y, z| Ok(BVec3::new(x, y, z));
    let invalid = |at| Err(SyntaxError::argument(ArgumentError::SwizzleInvalid, at));
    let cases = [
        ("xz", axes(true, false, true), 2),
        ("zx", axes(true, false, true), 2),
        ("yzx", axes(true, true, true), 3),
        ("xz run", axes(true, false, true), 2),
        ("X", invalid(1), 1),
        ("xx", invalid(2), 2),
        ("xw", invalid(2), 2),
        ("x,z", invalid(2), 2),
        (" x", invalid(0), 0),
    ];
    for (input, expected, cursor) in cases {
        assert_eq!(
            read(&SwizzleArgument, input, &OP),
            (expected, cursor),
            "{input}"
        );
    }
    let cases = [
        (
            BVec3::new(true, false, true),
            DVec3::new(1.7, 64.2, -0.2),
            DVec3::new(1.0, 64.2, -1.0),
        ),
        (
            BVec3::new(false, true, false),
            DVec3::new(1.7, 64.2, -0.2),
            DVec3::new(1.7, 64.0, -0.2),
        ),
        (
            BVec3::new(true, false, false),
            DVec3::new(3_000_000_000.5, 64.0, 0.0),
            DVec3::new(2_147_483_647.0, 64.0, 0.0),
        ),
    ];
    for (axes, from, to) in cases {
        assert_eq!(align(axes, from), to, "{axes:?}");
    }
}

#[test]
fn heightmap_reads_only_the_lower_case_kept_types() {
    use mcrs_minecraft_core::HeightmapName;
    assert_eq!(
        read(&HeightmapArgument, "ocean_floor", &OP).0,
        Ok(HeightmapName::OceanFloor)
    );
    for input in ["WORLD_SURFACE", "world_surface_wg", "1", ""] {
        let end = input.len();
        assert_eq!(
            read(&HeightmapArgument, input, &OP),
            (
                Err(SyntaxError::argument(
                    ArgumentError::EnumInvalid(Span::new(0, end)),
                    end
                )),
                end
            ),
            "{input}"
        );
    }
}

#[test]
fn the_rotation_toward_a_point_uses_the_arctangent_of_the_game() {
    let from = DVec3::new(0.5, 64.0, 0.5);
    let cases = [
        (DVec3::new(10.5, 64.0, 0.5), (-90.0, -0.0)),
        (DVec3::new(0.5, 64.0, 10.5), (0.0, -0.0)),
        (DVec3::new(0.5, 74.0, 0.5), (-90.0, -90.0)),
        (from, (-90.0, -0.0)),
    ];
    for (to, expected) in cases {
        let (yaw, pitch) = rotation_toward(from, to);
        assert_eq!((yaw, pitch), expected, "{to}");
        assert_eq!(
            pitch.is_sign_negative(),
            expected.1.is_sign_negative(),
            "{to}"
        );
    }
    let (_, down) = rotation_toward(from + DVec3::Y * 1.62, DVec3::new(0.5, 64.0, 10.5));
    assert!(down > 0.0, "{down}");
}

#[test]
fn time_reads_a_tick_count_in_its_unit() {
    use mcrs_minecraft_command::TimeArgument;
    let at_least_zero = TimeArgument { min: 0 };
    let any = TimeArgument { min: i32::MIN };
    let low = |ticks, cursor| {
        Err(SyntaxError::argument(
            ArgumentError::TickCountTooLow { min: 0, ticks },
            cursor,
        ))
    };
    let cases = [
        (at_least_zero, "1d", Ok(24_000)),
        (at_least_zero, "1.5s", Ok(30)),
        (at_least_zero, "0.1d", Ok(2_400)),
        (at_least_zero, "0.5", Ok(1)),
        (any, "-0.5", Ok(0)),
        (at_least_zero, "100000d", Ok(i32::MAX)),
        (
            at_least_zero,
            "1D",
            Err(SyntaxError::argument(ArgumentError::TimeInvalidUnit, 2)),
        ),
        (
            at_least_zero,
            "1e3",
            Err(SyntaxError::argument(ArgumentError::TimeInvalidUnit, 3)),
        ),
        (at_least_zero, "-1", low(-1, 2)),
        (
            at_least_zero,
            "d",
            Err(SyntaxError::library(LibraryError::ExpectedFloat, 0)),
        ),
    ];
    for (argument, input, expected) in cases {
        assert_eq!(read(&argument, input, &OP).0, expected, "{input}");
    }
}
