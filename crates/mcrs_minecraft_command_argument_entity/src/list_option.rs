use mcrs_command::Span;
use mcrs_command::StringValue;
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_resource::ResourceLocation;

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
        tag: ResourceLocation<'a>,
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
        id: ResourceLocation<'a>,
        span: Span,
    },
}
