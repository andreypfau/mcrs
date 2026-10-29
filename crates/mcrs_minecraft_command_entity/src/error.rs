use core::fmt;
use mcrs_command::StringSpan;
use mcrs_minecraft_command::{
    ErrorText, ResourceError, identifier_text, snbt_text, span_text, string_text,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_snbt::SnbtError;
use mcrs_text_span::Span;

/// The errors of an entity selector and of its search.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum EntityError {
    /// The source may not use a selector that starts with `@`.
    SelectorNotAllowed,
    /// The `@` has no kind after it.
    SelectorMissing,
    /// The kind after `@` is none of the six.
    SelectorUnknown(char),
    /// The text is neither a name nor a UUID.
    EntityInvalid,
    /// No option has the key at the span.
    OptionsUnknown(StringSpan),
    /// The option at the span cannot follow the options before it.
    OptionsInapplicable(StringSpan),
    /// The option at the span has no `=` and value.
    OptionsValueless(StringSpan),
    /// The options have no closing `]`.
    OptionsUnterminated,
    /// The `distance` range holds a negative value.
    DistanceNegative,
    /// The `level` range holds a negative value.
    LevelNegative,
    /// The `limit` is below one.
    LimitTooSmall,
    /// The word at the span names no sort order.
    SortIrreversible(Span),
    /// The word at the span names no game mode.
    ModeInvalid(Span),
    /// The identifier at the span names no entity type.
    TypeInvalid(Span),
    /// The range has neither a minimum nor a maximum.
    RangeEmpty,
    /// The minimum of the range is above its maximum.
    RangeSwapped,
    /// The argument takes one player and the selector can find more.
    PlayerTooMany,
    /// The argument takes one entity and the selector can find more.
    EntityTooMany,
    /// The argument takes players and the selector can find other entities.
    PlayerEntities,
    /// An error of the SNBT read; its spans are positions in the command line.
    Nbt(SnbtError),
    /// An error of an identifier or of a registry entry that the selector names.
    Resource(ResourceError),
}

impl fmt::Display for EntityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EntityError::SelectorNotAllowed => "selectors are not allowed",
            EntityError::SelectorMissing => "missing selector kind",
            EntityError::SelectorUnknown(_) => "unknown selector kind",
            EntityError::EntityInvalid => "invalid name or UUID",
            EntityError::OptionsUnknown(_) => "unknown selector option",
            EntityError::OptionsInapplicable(_) => "inapplicable selector option",
            EntityError::OptionsValueless(_) => "selector option without a value",
            EntityError::OptionsUnterminated => "unterminated selector options",
            EntityError::DistanceNegative => "negative distance",
            EntityError::LevelNegative => "negative level",
            EntityError::LimitTooSmall => "limit below one",
            EntityError::SortIrreversible(_) => "unknown sort order",
            EntityError::ModeInvalid(_) => "unknown game mode",
            EntityError::TypeInvalid(_) => "unknown entity type",
            EntityError::RangeEmpty => "empty range",
            EntityError::RangeSwapped => "swapped range",
            EntityError::PlayerTooMany => "more than one player",
            EntityError::EntityTooMany => "more than one entity",
            EntityError::PlayerEntities => "selector finds entities other than players",
            EntityError::Nbt(_) => "invalid SNBT",
            EntityError::Resource(error) => return fmt::Display::fmt(error, f),
        })
    }
}

impl core::error::Error for EntityError {}

impl From<ResourceError> for EntityError {
    fn from(error: ResourceError) -> Self {
        EntityError::Resource(error)
    }
}

impl ErrorText for EntityError {
    fn text(&self, input: &str) -> Text {
        use EntityError::*;
        match *self {
            SelectorNotAllowed => Text::translate("argument.entity.selector.not_allowed", vec![]),
            SelectorMissing => Text::translate("argument.entity.selector.missing", vec![]),
            SelectorUnknown(kind) => Text::translate(
                "argument.entity.selector.unknown",
                vec![format!("@{kind}").into_text()],
            ),
            EntityInvalid => Text::translate("argument.entity.invalid", vec![]),
            OptionsUnknown(name) => Text::translate(
                "argument.entity.options.unknown",
                vec![string_text(input, name)],
            ),
            OptionsInapplicable(name) => Text::translate(
                "argument.entity.options.inapplicable",
                vec![string_text(input, name)],
            ),
            OptionsValueless(name) => Text::translate(
                "argument.entity.options.valueless",
                vec![string_text(input, name)],
            ),
            OptionsUnterminated => Text::translate("argument.entity.options.unterminated", vec![]),
            DistanceNegative => {
                Text::translate("argument.entity.options.distance.negative", vec![])
            }
            LevelNegative => Text::translate("argument.entity.options.level.negative", vec![]),
            LimitTooSmall => Text::translate("argument.entity.options.limit.toosmall", vec![]),
            SortIrreversible(word) => Text::translate(
                "argument.entity.options.sort.irreversible",
                vec![span_text(input, word)],
            ),
            ModeInvalid(word) => Text::translate(
                "argument.entity.options.mode.invalid",
                vec![span_text(input, word)],
            ),
            TypeInvalid(id) => Text::translate(
                "argument.entity.options.type.invalid",
                vec![identifier_text(input, id)],
            ),
            RangeEmpty => Text::translate("argument.range.empty", vec![]),
            RangeSwapped => Text::translate("argument.range.swapped", vec![]),
            PlayerTooMany => Text::translate("argument.player.toomany", vec![]),
            EntityTooMany => Text::translate("argument.entity.toomany", vec![]),
            PlayerEntities => Text::translate("argument.player.entities", vec![]),
            Nbt(error) => snbt_text(input, error),
            Resource(error) => error.text(input),
        }
    }
}
