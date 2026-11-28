use crate::error::EntityError;
use crate::invertible::Invertible;
use crate::option_key::Key;
use crate::options::read_options;
use mcrs_command::{Reader, StringValue, SyntaxError};
use mcrs_minecraft_command::Source;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use uuid::Uuid;

const MAX_NAME_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Order {
    Arbitrary,
    Nearest,
    Furthest,
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorKind {
    NearestPlayer,
    AllPlayers,
    RandomPlayer,
    SelfEntity,
    AllEntities,
    NearestEntity,
}

impl SelectorKind {
    const fn players_only(self) -> bool {
        matches!(
            self,
            SelectorKind::NearestPlayer | SelectorKind::AllPlayers | SelectorKind::RandomPlayer
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Target<'a> {
    Name(StringValue<'a>),
    Uuid(Uuid),
    Kind(SelectorKind),
}

#[derive(Clone, Copy, Debug)]
pub struct Selector<'a> {
    pub target: Target<'a>,
    pub(crate) max_results: Option<u32>,
    pub(crate) includes_entities: bool,
    pub(crate) world_limited: bool,
    pub(crate) order: Order,
    pub(crate) type_limit: Option<EntityType>,
    pub(crate) current_entity: bool,
    pub(crate) alive: bool,
    pub(crate) origin: [Option<f64>; 3],
    pub(crate) size: [Option<f64>; 3],
    pub(crate) distance: Option<MinMaxBounds<f64>>,
    pub(crate) level: Option<MinMaxBounds<i32>>,
    pub(crate) x_rotation: Option<MinMaxBounds<f32>>,
    pub(crate) y_rotation: Option<MinMaxBounds<f32>>,
    pub(crate) input: &'a str,
    pub(crate) options: Option<usize>,
}

impl Selector<'_> {
    #[must_use]
    pub fn includes_entities(&self) -> bool {
        self.includes_entities
    }
}

pub(crate) struct KindEntry {
    pub(crate) text: &'static str,
    pub(crate) tooltip: &'static str,
    kind: SelectorKind,
    max_results: Option<u32>,
    order: Order,
}

pub(crate) const KINDS: [KindEntry; 6] = [
    KindEntry {
        text: "@p",
        tooltip: "argument.entity.selector.nearestPlayer",
        kind: SelectorKind::NearestPlayer,
        max_results: Some(1),
        order: Order::Nearest,
    },
    KindEntry {
        text: "@a",
        tooltip: "argument.entity.selector.allPlayers",
        kind: SelectorKind::AllPlayers,
        max_results: None,
        order: Order::Arbitrary,
    },
    KindEntry {
        text: "@r",
        tooltip: "argument.entity.selector.randomPlayer",
        kind: SelectorKind::RandomPlayer,
        max_results: Some(1),
        order: Order::Random,
    },
    KindEntry {
        text: "@s",
        tooltip: "argument.entity.selector.self",
        kind: SelectorKind::SelfEntity,
        max_results: Some(1),
        order: Order::Arbitrary,
    },
    KindEntry {
        text: "@e",
        tooltip: "argument.entity.selector.allEntities",
        kind: SelectorKind::AllEntities,
        max_results: None,
        order: Order::Arbitrary,
    },
    KindEntry {
        text: "@n",
        tooltip: "argument.entity.selector.nearestEntity",
        kind: SelectorKind::NearestEntity,
        max_results: Some(1),
        order: Order::Nearest,
    },
];

pub(crate) const SORTS: [(&str, Order); 4] = [
    ("nearest", Order::Nearest),
    ("furthest", Order::Furthest),
    ("random", Order::Random),
    ("arbitrary", Order::Arbitrary),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Next {
    NameOrSelector,
    Name,
    Kind,
    Open,
    Key,
    AfterValue,
    Sort,
    GameMode,
    Type,
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stop {
    pub(crate) next: Next,
    pub(crate) cursor: usize,
}

pub(crate) struct State<'a> {
    pub(crate) selector: Selector<'a>,
    pub(crate) names: Invertible,
    pub(crate) modes: Invertible,
    pub(crate) teams: Invertible,
    pub(crate) types: Invertible,
    pub(crate) limit_set: bool,
    pub(crate) sort_set: bool,
    pub(crate) scores_set: bool,
    pub(crate) advancements_set: bool,
    pub(crate) stop: Stop,
    pub(crate) check_duplicates: bool,
}

impl<'a> State<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            selector: Selector {
                target: Target::Kind(SelectorKind::AllEntities),
                max_results: None,
                includes_entities: true,
                world_limited: false,
                order: Order::Arbitrary,
                type_limit: None,
                current_entity: false,
                alive: false,
                origin: [None; 3],
                size: [None; 3],
                distance: None,
                level: None,
                x_rotation: None,
                y_rotation: None,
                input,
                options: None,
            },
            names: Invertible::None,
            modes: Invertible::None,
            teams: Invertible::None,
            types: Invertible::None,
            limit_set: false,
            sort_set: false,
            scores_set: false,
            advancements_set: false,
            stop: Stop {
                next: Next::NameOrSelector,
                cursor: 0,
            },
            check_duplicates: true,
        }
    }

    pub(crate) fn stop_at(&mut self, next: Next, cursor: usize) {
        self.stop = Stop { next, cursor };
    }

    pub(crate) fn available(&self, key: Key) -> bool {
        let selector = &self.selector;
        match key {
            Key::Name => self.names != Invertible::Single,
            Key::GameMode => self.modes != Invertible::Single,
            Key::Team => self.teams != Invertible::Single,
            Key::Type => self.types != Invertible::Single,
            Key::Distance => selector.distance.is_none(),
            Key::Level => selector.level.is_none(),
            Key::X => selector.origin[0].is_none(),
            Key::Y => selector.origin[1].is_none(),
            Key::Z => selector.origin[2].is_none(),
            Key::Dx => selector.size[0].is_none(),
            Key::Dy => selector.size[1].is_none(),
            Key::Dz => selector.size[2].is_none(),
            Key::XRotation => selector.x_rotation.is_none(),
            Key::YRotation => selector.y_rotation.is_none(),
            Key::Limit => !selector.current_entity && !self.limit_set,
            Key::Sort => !selector.current_entity && !self.sort_set,
            Key::Scores => !self.scores_set,
            Key::Advancements => !self.advancements_set,
            Key::Tag | Key::Nbt | Key::Predicate => true,
        }
    }
}

pub(crate) type SelectorResult<T> = Result<T, SyntaxError<EntityError>>;

pub(crate) fn error<T>(kind: EntityError, position: usize) -> SelectorResult<T> {
    Err(SyntaxError::argument(kind, position))
}

pub(crate) fn rewind<T>(
    reader: &mut Reader<'_>,
    start: usize,
    kind: EntityError,
) -> SelectorResult<T> {
    reader.set_cursor(start);
    error(kind, start)
}

enum UuidForm {
    NotUuid,
    Malformed,
    Valid(Uuid),
}

const UUID_LENGTH: usize = 36;

fn parse_uuid(text: &str) -> UuidForm {
    if text.matches('-').count() != 4 {
        return UuidForm::NotUuid;
    }
    match Uuid::try_parse(text) {
        Ok(uuid) if text.len() == UUID_LENGTH => UuidForm::Valid(uuid),
        _ => UuidForm::Malformed,
    }
}

fn uuid_form(value: &StringValue<'_>) -> UuidForm {
    if let Some(text) = value.as_unescaped() {
        return parse_uuid(text);
    }
    if value.chars().filter(|&c| c == '-').count() != 4 {
        return UuidForm::NotUuid;
    }
    UuidForm::Malformed
}

pub(crate) fn read_selector<'a, L>(
    reader: &mut Reader<'a>,
    source: &Source<'_, L>,
    state: &mut State<'a>,
) -> SelectorResult<()> {
    let start = reader.cursor();
    state.stop_at(Next::NameOrSelector, start);
    if reader.peek_is('@') {
        if !source.level.can_use_selectors() {
            return error(EntityError::SelectorNotAllowed, start);
        }
        reader.skip();
        state.stop_at(Next::Kind, start);
        let Some(letter) = reader.peek() else {
            return error(EntityError::SelectorMissing, reader.cursor());
        };
        let Some(entry) = KINDS.iter().find(|entry| entry.text.ends_with(letter)) else {
            return error(EntityError::SelectorUnknown(letter), reader.cursor());
        };
        reader.skip();
        let kind = entry.kind;
        let players = kind.players_only();
        let current_entity = kind == SelectorKind::SelfEntity;
        let selector = &mut state.selector;
        *selector = Selector {
            target: Target::Kind(kind),
            max_results: entry.max_results,
            includes_entities: !players,
            order: entry.order,
            type_limit: players.then_some(EntityType::Player),
            alive: !players && !current_entity,
            current_entity,
            ..*selector
        };
        state.stop_at(Next::Open, reader.cursor());
        if reader.peek_is('[') {
            reader.skip();
            state.selector.options = Some(reader.cursor());
            read_options(reader, state, &mut |_| {})?;
        }
        return Ok(());
    }
    if !reader.at_end() {
        state.stop_at(Next::Name, start);
    }
    let string = reader.read_string()?;
    let value = StringValue::new(reader.input(), string);
    let uuid = uuid_form(&value);
    let target = match uuid {
        UuidForm::Valid(uuid) => Target::Uuid(uuid),
        UuidForm::NotUuid if !value.is_empty() && value.len() <= MAX_NAME_BYTES => {
            Target::Name(value)
        }
        _ => return rewind(reader, start, EntityError::EntityInvalid),
    };
    let selector = &mut state.selector;
    selector.target = target;
    selector.includes_entities = matches!(target, Target::Uuid(_));
    selector.max_results = Some(1);
    Ok(())
}
