use mcrs_command::StringValue;
use mcrs_minecraft_command::Identifier;
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_protocol::GameMode;
use mcrs_text_span::Span;

/// An option that holds one of several values, as the search tests it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ListOption<'a> {
    Name {
        name: StringValue<'a>,
        negated: bool,
    },
    GameMode {
        mode: GameMode,
        negated: bool,
    },
    Team {
        team: &'a str,
        negated: bool,
    },
    Type {
        entity_type: EntityType,
        negated: bool,
    },
    TypeTag {
        tag: Identifier<'a>,
        negated: bool,
    },
    Tag {
        tag: &'a str,
        negated: bool,
    },
    Nbt {
        negated: bool,
    },
    Scores {
        empty: bool,
    },
    Advancements {
        empty: bool,
    },
    Predicate {
        id: Identifier<'a>,
        span: Span,
    },
}
