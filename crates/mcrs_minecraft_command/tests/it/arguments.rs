use crate::{OP, read};
use mcrs_command::Span;
use mcrs_command::SyntaxError;
use mcrs_minecraft_command::{
    EntityAnchorArgument, GameModeArgument, HeightmapArgument, Identifier, KeywordError,
    ResourceError, ResourceLocationArgument,
};
use mcrs_minecraft_protocol::GameMode;

#[test]
fn identifiers_default_their_namespace() {
    let id = |namespace, path| Identifier { namespace, path };
    let cases = [
        ("stone", Ok(id("minecraft", "stone")), 5),
        ("Stone", Ok(id("minecraft", "")), 0),
        (
            "a:b:c",
            Err(SyntaxError::argument(ResourceError::IdInvalid, 0)),
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
            KeywordError::GameModeInvalid(Span::new(0, 8)),
            8
        ))
    );
    assert_eq!(
        read(&GameModeArgument, "creativ", &OP).0,
        Err(SyntaxError::argument(
            KeywordError::GameModeInvalid(Span::new(0, 7)),
            7
        ))
    );
    assert_eq!(
        read(&EntityAnchorArgument, "foo", &OP),
        (
            Err(SyntaxError::argument(
                KeywordError::AnchorInvalid(Span::new(0, 3)),
                0
            )),
            0
        )
    );
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
                    KeywordError::EnumInvalid(Span::new(0, end)),
                    end
                )),
                end
            ),
            "{input}"
        );
    }
}
