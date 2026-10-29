use crate::error::KeywordError;
use crate::identifier::text_matches;
use mcrs_command::{Argument, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_core::LookAnchor;
use mcrs_minecraft_core::heightmap::HeightmapName;
use mcrs_minecraft_game_mode::GameMode;

/// `minecraft:gamemode` (ARG-KW-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameModeArgument;

impl GameModeArgument {
    /// Suggests the game modes.
    pub fn suggest<T>(builder: &mut SuggestionBuilder<'_, '_, T>) {
        builder.suggest_matching(GameMode::ALL.map(GameMode::name), text_matches);
    }
}

impl<S> Argument<S> for GameModeArgument {
    type Value<'a> = GameMode;
    type Error = KeywordError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<GameMode, SyntaxError<KeywordError>> {
        let word = reader.read_unquoted();
        word.text(reader.input()).parse().map_err(|_| {
            SyntaxError::argument(KeywordError::GameModeInvalid(word), reader.cursor())
        })
    }
}

/// The value named `text` in `table`.
pub fn by_name<T: Copy>(table: &[(&str, T)], text: &str) -> Option<T> {
    table
        .iter()
        .find(|(name, _)| *name == text)
        .map(|&(_, value)| value)
}

const ANCHORS: [(&str, LookAnchor); 2] = [("feet", LookAnchor::Feet), ("eyes", LookAnchor::Eyes)];

/// `minecraft:entity_anchor` (ARG-KW-002).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntityAnchorArgument;

impl EntityAnchorArgument {
    /// Suggests the anchors.
    pub fn suggest<T>(builder: &mut SuggestionBuilder<'_, '_, T>) {
        builder.suggest_matching(ANCHORS.map(|(name, _)| name), text_matches);
    }
}

impl<S> Argument<S> for EntityAnchorArgument {
    type Value<'a> = LookAnchor;
    type Error = KeywordError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<LookAnchor, SyntaxError<KeywordError>> {
        let start = reader.cursor();
        let word = reader.read_unquoted();
        let text = word.text(reader.input());
        let Some(anchor) = by_name(&ANCHORS, text) else {
            reader.set_cursor(start);
            return Err(SyntaxError::argument(
                KeywordError::AnchorInvalid(word),
                start,
            ));
        };
        Ok(anchor)
    }
}

const HEIGHTMAPS: [(&str, HeightmapName); 4] = [
    ("world_surface", HeightmapName::WorldSurface),
    ("ocean_floor", HeightmapName::OceanFloor),
    ("motion_blocking", HeightmapName::MotionBlocking),
    (
        "motion_blocking_no_leaves",
        HeightmapName::MotionBlockingNoLeaves,
    ),
];

/// `minecraft:heightmap` (ARG-KW-003).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeightmapArgument;

impl HeightmapArgument {
    /// Suggests the heightmaps.
    pub fn suggest<T>(builder: &mut SuggestionBuilder<'_, '_, T>) {
        builder.suggest_matching(HEIGHTMAPS.map(|(name, _)| name), text_matches);
    }
}

impl<S> Argument<S> for HeightmapArgument {
    type Value<'a> = HeightmapName;
    type Error = KeywordError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<HeightmapName, SyntaxError<KeywordError>> {
        let word = reader.read_unquoted();
        by_name(&HEIGHTMAPS, word.text(reader.input())).ok_or(SyntaxError::argument(
            KeywordError::EnumInvalid(word),
            reader.cursor(),
        ))
    }
}
