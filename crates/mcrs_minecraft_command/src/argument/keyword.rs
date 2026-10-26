use crate::error::{ArgumentError, CommandSyntaxError};
use crate::identifier::text_matches;
use crate::source::Anchor;
use mcrs_command::{Argument, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_protocol::GameMode;

/// `minecraft:gamemode` (ARG-KW-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameModeArgument;

impl GameModeArgument {
    pub fn suggest<T>(builder: &mut SuggestionBuilder<'_, '_, T>) {
        builder.suggest_matching(GameMode::ALL.map(GameMode::name), text_matches);
    }
}

impl<S> Argument<S> for GameModeArgument {
    type Value<'a> = GameMode;
    type Error = ArgumentError;

    fn parse<'a>(&self, reader: &mut Reader<'a>, _: &S) -> Result<GameMode, CommandSyntaxError> {
        let word = reader.read_unquoted();
        GameMode::from_name(word.text(reader.input())).ok_or(SyntaxError::argument(
            ArgumentError::GameModeInvalid(word),
            reader.cursor(),
        ))
    }
}

/// `minecraft:entity_anchor` (ARG-KW-002).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntityAnchorArgument;

impl EntityAnchorArgument {
    pub fn suggest<T>(builder: &mut SuggestionBuilder<'_, '_, T>) {
        builder.suggest_matching(["feet", "eyes"], text_matches);
    }
}

impl<S> Argument<S> for EntityAnchorArgument {
    type Value<'a> = Anchor;
    type Error = ArgumentError;

    fn parse<'a>(&self, reader: &mut Reader<'a>, _: &S) -> Result<Anchor, CommandSyntaxError> {
        let start = reader.cursor();
        let word = reader.read_unquoted();
        match word.text(reader.input()) {
            "feet" => Ok(Anchor::Feet),
            "eyes" => Ok(Anchor::Eyes),
            _ => {
                reader.set_cursor(start);
                Err(SyntaxError::argument(
                    ArgumentError::AnchorInvalid(word),
                    start,
                ))
            }
        }
    }
}
