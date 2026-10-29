use mcrs_command::StringSpan;
use mcrs_minecraft_command::{
    ErrorText, ResourceError, identifier_text, snbt_text, span_text, string_text,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_snbt::SnbtError;
use mcrs_text_span::Span;

/// The errors of an entity selector and of its search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EntityError {
    SelectorNotAllowed,
    SelectorMissing,
    SelectorUnknown(char),
    EntityInvalid,
    OptionsUnknown(StringSpan),
    OptionsInapplicable(StringSpan),
    OptionsValueless(StringSpan),
    OptionsUnterminated,
    DistanceNegative,
    LevelNegative,
    LimitTooSmall,
    SortIrreversible(Span),
    ModeInvalid(Span),
    TypeInvalid(Span),
    RangeEmpty,
    RangeSwapped,
    PlayerTooMany,
    EntityTooMany,
    PlayerEntities,
    /// An error of the SNBT read; its spans are positions in the command line.
    Nbt(SnbtError),
    Resource(ResourceError),
}

impl From<ResourceError> for EntityError {
    fn from(error: ResourceError) -> Self {
        EntityError::Resource(error)
    }
}

impl ErrorText for EntityError {
    fn text(&self, input: &str) -> Text {
        use EntityError as E;
        match *self {
            E::SelectorNotAllowed => {
                Text::translate("argument.entity.selector.not_allowed", vec![])
            }
            E::SelectorMissing => Text::translate("argument.entity.selector.missing", vec![]),
            E::SelectorUnknown(kind) => Text::translate(
                "argument.entity.selector.unknown",
                vec![format!("@{kind}").into_text()],
            ),
            E::EntityInvalid => Text::translate("argument.entity.invalid", vec![]),
            E::OptionsUnknown(name) => Text::translate(
                "argument.entity.options.unknown",
                vec![string_text(input, name)],
            ),
            E::OptionsInapplicable(name) => Text::translate(
                "argument.entity.options.inapplicable",
                vec![string_text(input, name)],
            ),
            E::OptionsValueless(name) => Text::translate(
                "argument.entity.options.valueless",
                vec![string_text(input, name)],
            ),
            E::OptionsUnterminated => {
                Text::translate("argument.entity.options.unterminated", vec![])
            }
            E::DistanceNegative => {
                Text::translate("argument.entity.options.distance.negative", vec![])
            }
            E::LevelNegative => Text::translate("argument.entity.options.level.negative", vec![]),
            E::LimitTooSmall => Text::translate("argument.entity.options.limit.toosmall", vec![]),
            E::SortIrreversible(word) => Text::translate(
                "argument.entity.options.sort.irreversible",
                vec![span_text(input, word)],
            ),
            E::ModeInvalid(word) => Text::translate(
                "argument.entity.options.mode.invalid",
                vec![span_text(input, word)],
            ),
            E::TypeInvalid(id) => Text::translate(
                "argument.entity.options.type.invalid",
                vec![identifier_text(input, id)],
            ),
            E::RangeEmpty => Text::translate("argument.range.empty", vec![]),
            E::RangeSwapped => Text::translate("argument.range.swapped", vec![]),
            E::PlayerTooMany => Text::translate("argument.player.toomany", vec![]),
            E::EntityTooMany => Text::translate("argument.entity.toomany", vec![]),
            E::PlayerEntities => Text::translate("argument.player.entities", vec![]),
            E::Nbt(error) => snbt_text(input, error),
            E::Resource(error) => error.text(input),
        }
    }
}
