use crate::error::EntityError;
use crate::option_key::KEYS;
use crate::options::Invertible;
use crate::selector::{KINDS, Next, SORTS, Selector, SelectorResult, State, error, read_selector};
use mcrs_command::{Argument, Reader, SuggestionBuilder, SuggestionText};
use mcrs_minecraft_command::{
    Lookup, Source, prefixed_identifier_matches, suggest_prefixed, text_matches,
};
use mcrs_minecraft_entity::keys::{ENTITY_TYPE, EntityType};
use mcrs_minecraft_protocol::GameMode;

/// `minecraft:entity` (ARG-ENT, section 0.3 for the four forms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityArgument {
    pub single: bool,
    pub players_only: bool,
}

impl EntityArgument {
    pub const ONE_ENTITY: Self = Self {
        single: true,
        players_only: false,
    };
    pub const ENTITIES: Self = Self {
        single: false,
        players_only: false,
    };
    pub const ONE_PLAYER: Self = Self {
        single: true,
        players_only: true,
    };
    pub const PLAYERS: Self = Self {
        single: false,
        players_only: true,
    };
}

impl EntityArgument {
    fn check(&self, selector: &Selector<'_>, start: usize) -> SelectorResult<()> {
        if self.single && selector.max_results.is_none_or(|max| max > 1) {
            let kind = if self.players_only {
                EntityError::PlayerTooMany
            } else {
                EntityError::EntityTooMany
            };
            return error(kind, start);
        }
        if self.players_only && selector.includes_entities && !selector.current_entity {
            return error(EntityError::PlayerEntities, start);
        }
        Ok(())
    }

    /// Suggests what can follow the read (ARG-ENT-011), for the server: player names, kinds,
    /// option keys and the values of `sort`, `gamemode` and `type`.
    pub fn suggest<'t, L: Lookup>(
        &self,
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, &'static str>,
    ) {
        let input = builder.input();
        let start = builder.start();
        let mut reader = Reader::at(input, start);
        let mut state = State::new(input);
        let _ = read_selector(&mut reader, source, &mut state);
        let next = state.stop.next;
        let selectors = source.level.can_use_selectors();
        let mut at = builder.at(state.stop.cursor);
        let remaining = at.remaining();
        if next == Next::Kind || next == Next::NameOrSelector && selectors {
            let kinds = KINDS.iter().map(|entry| (entry.text, Some(entry.tooltip)));
            suggest_prefixed(&mut at, &[""], kinds, text_starts);
        }
        match next {
            Next::NameOrSelector | Next::Name => {
                at.suggest_matching(source.lookup.player_names(), text_matches);
            }
            Next::Kind => {}
            Next::Open => at.suggest(SuggestionText::new("["), None),
            Next::Key => {
                for entry in &KEYS {
                    let text = SuggestionText::concat(entry.name, "=");
                    if state.available(entry.key) && text.starts_with(remaining) {
                        at.suggest(text, Some(entry.tooltip));
                    }
                }
            }
            Next::AfterValue => {
                at.suggest(SuggestionText::new(","), None);
                at.suggest(SuggestionText::new("]"), None);
            }
            Next::Sort => {
                let sorts = SORTS.iter().map(|&(name, _)| (name, None));
                suggest_prefixed(&mut at, &[""], sorts, text_starts);
            }
            Next::GameMode => {
                let names = GameMode::ALL.iter().map(|mode| (mode.name(), None));
                let prefixes = state.modes.prefixes();
                suggest_prefixed(&mut at, prefixes, names, text_starts);
            }
            Next::Type => {
                let prefixes = state.types.prefixes();
                let types = EntityType::ALL
                    .iter()
                    .map(|entity_type| (entity_type.as_static_str(), None));
                suggest_prefixed(&mut at, prefixes, types, prefixed_identifier_matches);
                if state.types != Invertible::Single {
                    let tags = source
                        .lookup
                        .tags(ENTITY_TYPE.location())
                        .map(|tag| (tag, None));
                    suggest_prefixed(&mut at, &["#", "!#"], tags, prefixed_identifier_matches);
                }
            }
            Next::Nothing => {}
        }
    }
}

fn text_starts(remaining: &str, prefix: &str, id: &str) -> bool {
    SuggestionText::concat(prefix, id).starts_with(remaining)
}

impl<L> Argument<Source<'_, L>> for EntityArgument {
    type Value<'a> = Selector<'a>;
    type Error = EntityError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> SelectorResult<Selector<'a>> {
        let start = reader.cursor();
        let mut state = State::new(reader.input());
        read_selector(reader, source, &mut state)?;
        self.check(&state.selector, start)?;
        Ok(state.selector)
    }
}
