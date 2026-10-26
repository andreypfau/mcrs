use crate::error::{ArgumentError, CommandSyntaxError};
use crate::identifier::{
    Identifier, prefixed_identifier_matches, read_identifier, suggest_prefixed, text_matches,
};
use crate::source::{Lookup, Source};
use glam::DVec3;
use mcrs_command::suggest::eq_ignore_case;
use mcrs_command::{
    Argument, LibraryError, Reader, Span, StringSpan, StringValue, SuggestionBuilder,
    SuggestionText, SyntaxError,
};
use mcrs_minecraft_core::mth::wrap_degrees;
use mcrs_minecraft_entity::keys::{ENTITY_TYPE, EntityType};
use mcrs_minecraft_loot::keys::PREDICATE;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use uuid::Uuid;

const MAX_NAME_UNITS: usize = 16;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Order {
    Arbitrary,
    Nearest,
    Furthest,
    Random,
}

#[derive(Clone, Copy, Debug)]
pub enum Target<'a> {
    Name(StringValue<'a>),
    Uuid(Uuid),
    Kind(char),
}

/// The selector an entity argument read. The options that hold several values are read again
/// from the command line when the search tests an entity.
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
    input: &'a str,
    options: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Invertible {
    #[default]
    None,
    Single,
    Multiple,
}

impl Invertible {
    fn allows(self, negated: bool) -> bool {
        if negated {
            self != Invertible::Single
        } else {
            self == Invertible::None
        }
    }

    /// The forms a value can take now: plain and negated, only negated, or none.
    fn prefixes(self) -> &'static [&'static str] {
        match self {
            Invertible::None => &["", "!"],
            Invertible::Multiple => &["!"],
            Invertible::Single => &[],
        }
    }

    fn after(negated: bool) -> Self {
        if negated {
            Invertible::Multiple
        } else {
            Invertible::Single
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Name,
    Distance,
    Level,
    X,
    Y,
    Z,
    Dx,
    Dy,
    Dz,
    XRotation,
    YRotation,
    Limit,
    Sort,
    GameMode,
    Team,
    Type,
    Tag,
    Nbt,
    Scores,
    Advancements,
    Predicate,
}

macro_rules! keys {
    ($($name:literal => $key:ident,)+) => {
        [$(($name, Key::$key, concat!("argument.entity.options.", $name, ".description")),)+]
    };
}

const KEYS: [(&str, Key, &str); 21] = keys! {
    "name" => Name,
    "distance" => Distance,
    "level" => Level,
    "x" => X,
    "y" => Y,
    "z" => Z,
    "dx" => Dx,
    "dy" => Dy,
    "dz" => Dz,
    "x_rotation" => XRotation,
    "y_rotation" => YRotation,
    "limit" => Limit,
    "sort" => Sort,
    "gamemode" => GameMode,
    "team" => Team,
    "type" => Type,
    "tag" => Tag,
    "nbt" => Nbt,
    "scores" => Scores,
    "advancements" => Advancements,
    "predicate" => Predicate,
};

const KINDS: [(&str, &str, Option<u32>, Order); 6] = [
    (
        "@p",
        "argument.entity.selector.nearestPlayer",
        Some(1),
        Order::Nearest,
    ),
    (
        "@a",
        "argument.entity.selector.allPlayers",
        None,
        Order::Arbitrary,
    ),
    (
        "@r",
        "argument.entity.selector.randomPlayer",
        Some(1),
        Order::Random,
    ),
    (
        "@s",
        "argument.entity.selector.self",
        Some(1),
        Order::Arbitrary,
    ),
    (
        "@e",
        "argument.entity.selector.allEntities",
        None,
        Order::Arbitrary,
    ),
    (
        "@n",
        "argument.entity.selector.nearestEntity",
        Some(1),
        Order::Nearest,
    ),
];

const SORTS: [(&str, Order); 4] = [
    ("nearest", Order::Nearest),
    ("furthest", Order::Furthest),
    ("random", Order::Random),
    ("arbitrary", Order::Arbitrary),
];

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

/// Where the read stopped, and so what can follow there (ARG-ENT-011).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
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

struct State<'a> {
    selector: Selector<'a>,
    names: Invertible,
    modes: Invertible,
    teams: Invertible,
    types: Invertible,
    limit_set: bool,
    sort_set: bool,
    scores_set: bool,
    advancements_set: bool,
    next: (Next, usize),
    check_duplicates: bool,
}

impl<'a> State<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            selector: Selector {
                target: Target::Kind('e'),
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
            next: (Next::NameOrSelector, 0),
            check_duplicates: true,
        }
    }

    fn available(&self, key: Key) -> bool {
        let s = &self.selector;
        match key {
            Key::Name => self.names != Invertible::Single,
            Key::GameMode => self.modes != Invertible::Single,
            Key::Team => self.teams != Invertible::Single,
            Key::Type => self.types != Invertible::Single,
            Key::Distance => s.distance.is_none(),
            Key::Level => s.level.is_none(),
            Key::X => s.origin[0].is_none(),
            Key::Y => s.origin[1].is_none(),
            Key::Z => s.origin[2].is_none(),
            Key::Dx => s.size[0].is_none(),
            Key::Dy => s.size[1].is_none(),
            Key::Dz => s.size[2].is_none(),
            Key::XRotation => s.x_rotation.is_none(),
            Key::YRotation => s.y_rotation.is_none(),
            Key::Limit => !s.current_entity && !self.limit_set,
            Key::Sort => !s.current_entity && !self.sort_set,
            Key::Scores => !self.scores_set,
            Key::Advancements => !self.advancements_set,
            Key::Tag | Key::Nbt | Key::Predicate => true,
        }
    }
}

type Read<T> = Result<T, CommandSyntaxError>;

fn error<T>(kind: ArgumentError, position: usize) -> Read<T> {
    Err(SyntaxError::argument(kind, position))
}

fn read_negation(reader: &mut Reader<'_>) -> bool {
    reader.skip_whitespace();
    let negated = reader.peek_is('!');
    if negated {
        reader.skip();
        reader.skip_whitespace();
    }
    negated
}

trait BoundNumber: Copy + PartialOrd {
    const SWAPPABLE: bool;
    const INVALID: fn(Span) -> LibraryError;
    fn read(reader: &mut Reader<'_>) -> Result<Self, CommandSyntaxError>;
}

impl BoundNumber for i32 {
    const INVALID: fn(Span) -> LibraryError = LibraryError::InvalidInt;
    const SWAPPABLE: bool = false;
    fn read(reader: &mut Reader<'_>) -> Result<Self, CommandSyntaxError> {
        reader.read_int()
    }
}

impl BoundNumber for f64 {
    const INVALID: fn(Span) -> LibraryError = LibraryError::InvalidDouble;
    const SWAPPABLE: bool = false;
    fn read(reader: &mut Reader<'_>) -> Result<Self, CommandSyntaxError> {
        reader.read_double()
    }
}

impl BoundNumber for f32 {
    const INVALID: fn(Span) -> LibraryError = LibraryError::InvalidFloat;
    const SWAPPABLE: bool = true;
    fn read(reader: &mut Reader<'_>) -> Result<Self, CommandSyntaxError> {
        reader.read_float()
    }
}

/// A run of number characters that stops before `..`.
fn bound_token(reader: &mut Reader<'_>) -> Span {
    let start = reader.cursor();
    while let Some(c) = reader.peek() {
        let range_follows = c == '.' && reader.peek_nth(1) == Some('.');
        if range_follows || !matches!(c, '0'..='9' | '-' | '.') {
            break;
        }
        reader.skip();
    }
    reader.span_from(start)
}

fn parse_bound<T: BoundNumber>(input: &str, token: Span, start: usize) -> Read<Option<T>> {
    if token.is_empty() {
        return Ok(None);
    }
    let mut reader = Reader::at(input.get(..token.end).unwrap_or_default(), token.start);
    T::read(&mut reader)
        .ok()
        .filter(|_| reader.at_end())
        .map(Some)
        .ok_or(SyntaxError::library(T::INVALID(token), start))
}

/// Bounds (ARG-ENT-007).
fn read_bounds<T: BoundNumber>(reader: &mut Reader<'_>) -> Read<MinMaxBounds<T>> {
    let start = reader.cursor();
    if reader.at_end() {
        return error(ArgumentError::RangeEmpty, reader.cursor());
    }
    let first = bound_token(reader);
    let second = if reader.peek_is('.') && reader.peek_nth(1) == Some('.') {
        reader.skip();
        reader.skip();
        bound_token(reader)
    } else {
        first
    };
    if first.is_empty() && second.is_empty() {
        return rewind(reader, start, ArgumentError::RangeEmpty);
    }
    let input = reader.input();
    let (min, max) = parse_bound::<T>(input, first, start)
        .and_then(|min| Ok((min, parse_bound::<T>(input, second, start)?)))
        .inspect_err(|_| reader.set_cursor(start))?;
    if let (Some(min), Some(max)) = (min, max)
        && !T::SWAPPABLE
        && min > max
    {
        return rewind(reader, start, ArgumentError::RangeSwapped);
    }
    Ok(MinMaxBounds { min, max })
}

fn is_negative<T: Copy + PartialOrd + Default>(bounds: &MinMaxBounds<T>) -> bool {
    let zero = T::default();
    bounds.min.is_some_and(|v| v < zero) || bounds.max.is_some_and(|v| v < zero)
}

fn rewind<T>(reader: &mut Reader<'_>, start: usize, kind: ArgumentError) -> Read<T> {
    reader.set_cursor(start);
    error(kind, start)
}

/// A map of `{key=value, ...}` whose values `value` reads (the `scores` and `advancements`
/// options). Returns whether the map has no entry.
fn read_map(
    reader: &mut Reader<'_>,
    key: fn(&mut Reader<'_>) -> Read<()>,
    value: fn(&mut Reader<'_>) -> Read<()>,
) -> Read<bool> {
    reader.expect('{')?;
    reader.skip_whitespace();
    let mut empty = true;
    while !reader.at_end() && !reader.peek_is('}') {
        empty = false;
        reader.skip_whitespace();
        key(reader)?;
        reader.skip_whitespace();
        reader.expect('=')?;
        reader.skip_whitespace();
        value(reader)?;
        reader.skip_whitespace();
        if reader.peek_is(',') {
            reader.skip();
        }
    }
    reader.expect('}')?;
    Ok(empty)
}

fn read_unquoted_key(reader: &mut Reader<'_>) -> Read<()> {
    reader.read_unquoted();
    Ok(())
}

fn read_progress(reader: &mut Reader<'_>) -> Read<()> {
    let read_bool = |reader: &mut Reader<'_>| reader.read_bool().map(drop);
    if reader.peek_is('{') {
        read_map(reader, read_unquoted_key, read_bool).map(drop)
    } else {
        read_bool(reader)
    }
}

fn entity_type(id: Identifier<'_>) -> Option<EntityType> {
    EntityType::ALL
        .iter()
        .copied()
        .find(|entity_type| id.eq_str(entity_type.as_static_str()))
}

fn read_scalar(reader: &mut Reader<'_>, state: &mut State<'_>, key: Key) -> Read<()> {
    let start = reader.cursor();
    let input = reader.input();
    let s = &mut state.selector;
    match key {
        Key::Distance => {
            let bounds = read_bounds::<f64>(reader)?;
            if is_negative(&bounds) {
                return rewind(reader, start, ArgumentError::DistanceNegative);
            }
            s.distance = Some(bounds);
            s.world_limited = true;
        }
        Key::Level => {
            let bounds = read_bounds::<i32>(reader)?;
            if is_negative(&bounds) {
                return rewind(reader, start, ArgumentError::LevelNegative);
            }
            s.level = Some(bounds);
            s.includes_entities = false;
        }
        Key::X | Key::Y | Key::Z | Key::Dx | Key::Dy | Key::Dz => {
            let value = reader.read_double()?;
            let slot = match key {
                Key::X => &mut s.origin[0],
                Key::Y => &mut s.origin[1],
                Key::Z => &mut s.origin[2],
                Key::Dx => &mut s.size[0],
                Key::Dy => &mut s.size[1],
                _ => &mut s.size[2],
            };
            *slot = Some(value);
            s.world_limited = true;
        }
        Key::XRotation => s.x_rotation = Some(read_bounds::<f32>(reader)?),
        Key::YRotation => s.y_rotation = Some(read_bounds::<f32>(reader)?),
        Key::Limit => {
            let limit = reader.read_int()?;
            if limit < 1 {
                return rewind(reader, start, ArgumentError::LimitTooSmall);
            }
            s.max_results = u32::try_from(limit).ok();
            state.limit_set = true;
        }
        Key::Sort => {
            let word = reader.read_unquoted();
            let Some(&(_, order)) = SORTS.iter().find(|(name, _)| *name == word.text(input)) else {
                return rewind(reader, start, ArgumentError::SortIrreversible(word));
            };
            s.order = order;
            state.sort_set = true;
        }
        _ => {}
    }
    Ok(())
}

fn read_value<'a>(
    reader: &mut Reader<'a>,
    state: &mut State<'a>,
    key: Key,
    key_span: StringSpan,
    options_start: usize,
    visit: &mut impl FnMut(ListOption<'a>),
) -> Read<()> {
    let inapplicable = ArgumentError::OptionsInapplicable(key_span);
    let start = reader.cursor();
    let input = reader.input();
    let s = &mut state.selector;
    match key {
        Key::Name => {
            let negated = read_negation(reader);
            let name = reader.read_string()?;
            if !state.names.allows(negated) {
                return rewind(reader, start, inapplicable);
            }
            state.names = Invertible::after(negated);
            visit(ListOption::Name {
                name: StringValue::new(input, name),
                negated,
            });
        }
        Key::GameMode => {
            let negated = read_negation(reader);
            if !state.modes.allows(negated) {
                return rewind(reader, start, inapplicable);
            }
            let word = reader.read_unquoted();
            let Some(mode) = GameMode::from_name(word.text(input)) else {
                return rewind(reader, start, ArgumentError::ModeInvalid(word));
            };
            s.includes_entities = false;
            state.modes = Invertible::after(negated);
            visit(ListOption::GameMode { mode, negated });
        }
        Key::Team => {
            let negated = read_negation(reader);
            let team = reader.read_unquoted();
            if !state.teams.allows(negated) {
                return rewind(reader, start, inapplicable);
            }
            state.teams = Invertible::after(negated);
            visit(ListOption::Team {
                team: team.text(input),
                negated,
            });
        }
        Key::Type => read_type(reader, state, key_span, options_start, visit)?,
        Key::Tag => {
            let negated = read_negation(reader);
            let tag = reader.read_unquoted();
            visit(ListOption::Tag {
                tag: tag.text(input),
                negated,
            });
        }
        // chisle: the text form of NBT has no specification yet, so no `nbt` value parses;
        // a parser of that grammar lifts it.
        Key::Nbt => return rewind(reader, start, inapplicable),
        Key::Scores => {
            let empty = read_map(reader, read_unquoted_key, |reader| {
                read_bounds::<i32>(reader).map(drop)
            })?;
            state.scores_set = true;
            visit(ListOption::Scores { empty });
        }
        Key::Advancements => {
            let empty = read_map(
                reader,
                |reader| read_identifier(reader).map(drop),
                read_progress,
            )?;
            if !empty {
                state.selector.includes_entities = false;
            }
            state.advancements_set = true;
            visit(ListOption::Advancements { empty });
        }
        Key::Predicate => {
            read_negation(reader);
            let (id, span) = read_identifier(reader)?;
            visit(ListOption::Predicate { id, span });
        }
        _ => read_scalar(reader, state, key)?,
    }
    Ok(())
}

fn read_type<'a>(
    reader: &mut Reader<'a>,
    state: &mut State<'a>,
    key_span: StringSpan,
    options_start: usize,
    visit: &mut impl FnMut(ListOption<'a>),
) -> Read<()> {
    let inapplicable = ArgumentError::OptionsInapplicable(key_span);
    let start = reader.cursor();
    let input = reader.input();
    let negated = read_negation(reader);
    reader.skip_whitespace();
    if reader.peek_is('#') {
        reader.skip();
        reader.skip_whitespace();
        if state.types == Invertible::Single {
            return rewind(reader, start, inapplicable);
        }
        let (tag, _) = read_identifier(reader)?;
        if state.check_duplicates && type_tag_seen(input, options_start, start, tag) {
            return rewind(reader, start, inapplicable);
        }
        state.types = Invertible::Multiple;
        visit(ListOption::TypeTag { tag, negated });
    } else {
        if !state.types.allows(negated) {
            return rewind(reader, start, inapplicable);
        }
        let (id, id_span) = read_identifier(reader)?;
        let Some(entity_type) = entity_type(id) else {
            return rewind(reader, start, ArgumentError::TypeInvalid(id_span));
        };
        if !negated {
            state.selector.type_limit = Some(entity_type);
            if entity_type == EntityType::Player {
                state.selector.includes_entities = false;
            }
        }
        state.types = Invertible::after(negated);
        visit(ListOption::Type {
            entity_type,
            negated,
        });
    }
    Ok(())
}

/// Whether an earlier `type` option of the same selector named this tag.
fn type_tag_seen(input: &str, options_start: usize, before: usize, tag: Identifier<'_>) -> bool {
    let prefix = input.get(..before).unwrap_or_default();
    let mut reader = Reader::at(prefix, options_start);
    let mut state = State::new(prefix);
    state.check_duplicates = false;
    let mut seen = false;
    let _ = read_options(&mut reader, &mut state, &mut |option| {
        if let ListOption::TypeTag { tag: read, .. } = option {
            seen |= read == tag;
        }
    });
    seen
}

/// The options after `[` (ARG-ENT-004).
fn read_options<'a>(
    reader: &mut Reader<'a>,
    state: &mut State<'a>,
    visit: &mut impl FnMut(ListOption<'a>),
) -> Read<()> {
    let options_start = reader.cursor();
    loop {
        reader.skip_whitespace();
        state.next = (Next::Key, reader.cursor());
        if reader.at_end() || reader.peek_is(']') {
            break;
        }
        let key_start = reader.cursor();
        let key_span = reader.read_string()?;
        let name = StringValue::new(reader.input(), key_span);
        let Some(&(_, key, _)) = KEYS.iter().find(|(text, ..)| name.eq_str(text)) else {
            return rewind(reader, key_start, ArgumentError::OptionsUnknown(key_span));
        };
        if !state.available(key) {
            return rewind(
                reader,
                key_start,
                ArgumentError::OptionsInapplicable(key_span),
            );
        }
        reader.skip_whitespace();
        if !reader.peek_is('=') {
            return rewind(reader, key_start, ArgumentError::OptionsValueless(key_span));
        }
        reader.skip();
        reader.skip_whitespace();
        let next = match key {
            Key::Sort => Next::Sort,
            Key::GameMode => Next::GameMode,
            Key::Type => Next::Type,
            _ => Next::Nothing,
        };
        state.next = (next, reader.cursor());
        read_value(reader, state, key, key_span, options_start, visit)?;
        reader.skip_whitespace();
        state.next = (Next::AfterValue, reader.cursor());
        if reader.at_end() || reader.peek_is(']') {
            break;
        }
        if !reader.peek_is(',') {
            return error(ArgumentError::OptionsUnterminated, reader.cursor());
        }
        reader.skip();
    }
    if reader.at_end() {
        return error(ArgumentError::OptionsUnterminated, reader.cursor());
    }
    reader.skip();
    state.next = (Next::Nothing, reader.cursor());
    Ok(())
}

/// The UUID of `text` (ARG-ENT-005): `Err` for four `-` that do not split hexadecimal groups.
fn parse_uuid(text: &str) -> Option<Result<Uuid, ()>> {
    if text.matches('-').count() != 4 {
        return None;
    }
    let widths = [8, 4, 4, 4, 12];
    let mut value: u128 = 0;
    for (group, width) in text.split('-').zip(widths) {
        if group.is_empty() || group.len() > width || !group.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Some(Err(()));
        }
        let Ok(digits) = u64::from_str_radix(group, 16) else {
            return Some(Err(()));
        };
        value = (value << (width * 4)) | u128::from(digits);
    }
    Some(Ok(Uuid::from_u128(value)))
}

fn read_selector<'a, L>(
    reader: &mut Reader<'a>,
    source: &Source<'_, L>,
    state: &mut State<'a>,
) -> Read<()> {
    let start = reader.cursor();
    state.next = (Next::NameOrSelector, start);
    if reader.peek_is('@') {
        if !source.level.can_use_selectors() {
            return error(ArgumentError::SelectorNotAllowed, start);
        }
        reader.skip();
        state.next = (Next::Kind, start);
        let Some(kind) = reader.peek() else {
            return error(ArgumentError::SelectorMissing, reader.cursor());
        };
        let Some(&(.., max_results, order)) = KINDS.iter().find(|(text, ..)| text.ends_with(kind))
        else {
            return error(ArgumentError::SelectorUnknown(kind), reader.cursor());
        };
        reader.skip();
        let players = matches!(kind, 'p' | 'a' | 'r');
        let s = &mut state.selector;
        *s = Selector {
            target: Target::Kind(kind),
            max_results,
            includes_entities: !players,
            order,
            type_limit: players.then_some(EntityType::Player),
            alive: !players && kind != 's',
            current_entity: kind == 's',
            ..*s
        };
        state.next = (Next::Open, reader.cursor());
        if reader.peek_is('[') {
            reader.skip();
            state.selector.options = Some(reader.cursor());
            read_options(reader, state, &mut |_| {})?;
        }
        return Ok(());
    }
    if !reader.at_end() {
        state.next = (Next::Name, start);
    }
    let string = reader.read_string()?;
    let value = StringValue::new(reader.input(), string);
    let uuid = value.as_unescaped().and_then(parse_uuid);
    let target = match uuid {
        Some(Ok(uuid)) => Target::Uuid(uuid),
        None if !value.is_empty() && value.len_utf16() <= MAX_NAME_UNITS => Target::Name(value),
        _ => return rewind(reader, start, ArgumentError::EntityInvalid),
    };
    let s = &mut state.selector;
    s.target = target;
    s.includes_entities = matches!(target, Target::Uuid(_));
    s.max_results = Some(1);
    Ok(())
}

impl EntityArgument {
    fn check(&self, selector: &Selector<'_>, start: usize) -> Read<()> {
        if self.single && selector.max_results.is_none_or(|max| max > 1) {
            let kind = if self.players_only {
                ArgumentError::PlayerTooMany
            } else {
                ArgumentError::EntityTooMany
            };
            return error(kind, start);
        }
        if self.players_only && selector.includes_entities && !selector.current_entity {
            return error(ArgumentError::PlayerEntities, start);
        }
        Ok(())
    }

    /// ARG-ENT-011, for the server: player names, kinds, option keys and the values of `sort`,
    /// `gamemode` and `type`.
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
        let (next, at) = state.next;
        let selectors = source.level.can_use_selectors();
        let mut at = builder.at(at);
        let remaining = at.remaining();
        if next == Next::Kind || next == Next::NameOrSelector && selectors {
            let kinds = KINDS
                .iter()
                .map(|&(kind, tooltip, ..)| (kind, Some(tooltip)));
            suggest_prefixed(&mut at, &[""], kinds, remaining, text_starts);
        }
        match next {
            Next::NameOrSelector | Next::Name => {
                at.suggest_matching(source.lookup.player_names(), text_matches);
            }
            Next::Kind => {}
            Next::Open => at.suggest(SuggestionText::new("["), None),
            Next::Key => {
                for (name, key, tooltip) in KEYS {
                    let text = SuggestionText::concat(name, "=");
                    if state.available(key) && text.starts_with(remaining) {
                        at.suggest(text, Some(tooltip));
                    }
                }
            }
            Next::AfterValue => {
                at.suggest(SuggestionText::new(","), None);
                at.suggest(SuggestionText::new("]"), None);
            }
            Next::Sort => {
                let sorts = SORTS.iter().map(|&(name, _)| (name, None));
                suggest_prefixed(&mut at, &[""], sorts, remaining, text_starts);
            }
            Next::GameMode => {
                let names = GameMode::ALL.iter().map(|mode| (mode.name(), None));
                let prefixes = state.modes.prefixes();
                suggest_prefixed(&mut at, prefixes, names, remaining, text_starts);
            }
            Next::Type => {
                let prefixes = state.types.prefixes();
                let types = EntityType::ALL
                    .iter()
                    .map(|entity_type| (entity_type.as_static_str(), None));
                suggest_prefixed(
                    &mut at,
                    prefixes,
                    types,
                    remaining,
                    prefixed_identifier_matches,
                );
                if state.types != Invertible::Single {
                    let tags = source
                        .lookup
                        .tags(ENTITY_TYPE.location())
                        .map(|tag| (tag, None));
                    suggest_prefixed(
                        &mut at,
                        &["#", "!#"],
                        tags,
                        remaining,
                        prefixed_identifier_matches,
                    );
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
    type Error = ArgumentError;

    fn parse<'a>(&self, reader: &mut Reader<'a>, source: &Source<'_, L>) -> Read<Selector<'a>> {
        let start = reader.cursor();
        let mut state = State::new(reader.input());
        read_selector(reader, source, &mut state)?;
        self.check(&state.selector, start)?;
        Ok(state.selector)
    }
}

/// An entity as the search sees it.
pub trait Candidate {
    fn uuid(&self) -> Uuid;
    /// The plain text of its name; a player's name for a player.
    fn name(&self) -> &str;
    fn entity_type(&self) -> EntityType;
    fn position(&self) -> DVec3;
    /// The minimum and the maximum corner of its bounding box.
    fn bounding_box(&self) -> (DVec3, DVec3);
    fn yaw(&self) -> f32;
    fn pitch(&self) -> f32;
    fn game_mode(&self) -> Option<GameMode>;
    fn experience_level(&self) -> Option<i32>;
    fn alive(&self) -> bool;
    fn has_tag(&self, tag: &str) -> bool;
    fn has_any_tag(&self) -> bool;
    fn team(&self) -> Option<&str>;
    /// The entity is in the dimension of the source.
    fn in_source_dimension(&self) -> bool;

    fn is_player(&self) -> bool {
        self.entity_type() == EntityType::Player
    }
}

impl<'a> Selector<'a> {
    /// Each option that holds one of several values, in the order of the text.
    pub(crate) fn list_options(&self, mut each: impl FnMut(ListOption<'a>)) {
        let Some(at) = self.options else {
            return;
        };
        let mut reader = Reader::at(self.input, at);
        let mut state = State::new(self.input);
        state.check_duplicates = false;
        let _ = read_options(&mut reader, &mut state, &mut each);
    }

    fn in_box<C: Candidate>(&self, candidate: &C, origin: DVec3) -> bool {
        if self.size.iter().all(Option::is_none) {
            return true;
        }
        let size = DVec3::from(self.size.map(|d| d.unwrap_or(0.0)));
        let low = origin + size.min(DVec3::ZERO);
        let high = origin + size.max(DVec3::ZERO) + DVec3::ONE;
        let (min, max) = candidate.bounding_box();
        if min == max {
            let p = candidate.position();
            return p.cmpge(low).all() && p.cmple(high).all();
        }
        min.cmplt(high).all() && max.cmpgt(low).all()
    }

    /// Reads the options once and keeps the entities of `found` that pass each one.
    fn retain_listed<C: Candidate, L: Lookup>(&self, found: &mut Vec<&C>, lookup: &L) {
        self.list_options(|option| {
            found.retain(|candidate| match option {
                ListOption::Name { name, negated } => name.eq_str(candidate.name()) != negated,
                ListOption::GameMode { mode, negated } => candidate
                    .game_mode()
                    .is_some_and(|actual| (actual == mode) != negated),
                ListOption::Team { team, negated } => {
                    (candidate.team().unwrap_or_default() == team) != negated
                }
                ListOption::Type {
                    entity_type,
                    negated,
                } => (candidate.entity_type() == entity_type) != negated,
                ListOption::TypeTag { tag, negated } => {
                    lookup.tag_contains(
                        ENTITY_TYPE.location(),
                        tag,
                        candidate.entity_type().location(),
                    ) != negated
                }
                ListOption::Tag { tag, negated } => {
                    let has = if tag.is_empty() {
                        !candidate.has_any_tag()
                    } else {
                        candidate.has_tag(tag)
                    };
                    has != negated
                }
                // No scoreboard and no advancements exist, so an objective or an
                // advancement that the map names never does either.
                ListOption::Scores { empty } | ListOption::Advancements { empty } => empty,
                // chisle: a loot predicate is not evaluated against an entity yet, so one that
                // exists selects nothing; evaluating loot conditions here lifts it.
                ListOption::Predicate { .. } => false,
            });
        });
    }

    fn passes<C: Candidate>(&self, candidate: &C, origin: DVec3) -> bool {
        if self.alive && !candidate.alive() {
            return false;
        }
        if self
            .type_limit
            .is_some_and(|limit| candidate.entity_type() != limit)
        {
            return false;
        }
        if let Some(level) = &self.level
            && !candidate
                .experience_level()
                .is_some_and(|l| level.matches(l))
        {
            return false;
        }
        let pitch = wrap_degrees(candidate.pitch());
        if let Some(MinMaxBounds { min, max }) = self.x_rotation {
            let min = min.map(wrap_degrees);
            let max = max.map(wrap_degrees);
            let inside = match (min, max) {
                (Some(min), Some(max)) if min > max => pitch >= min || pitch <= max,
                _ => min.is_none_or(|m| pitch >= m) && max.is_none_or(|m| pitch <= m),
            };
            if !inside {
                return false;
            }
        }
        if let Some(MinMaxBounds { min, max }) = self.y_rotation {
            let yaw = candidate.yaw().rem_euclid(360.0);
            let min = min.map_or(0.0, |m| m.rem_euclid(360.0));
            let inside = match max.map(|m| m.rem_euclid(360.0)) {
                None => yaw >= min,
                Some(max) if min <= max => yaw >= min && yaw <= max,
                Some(max) => yaw >= min || yaw <= max,
            };
            if !inside {
                return false;
            }
        }
        self.in_box(candidate, origin)
            && self
                .distance
                .as_ref()
                .is_none_or(|distance| distance.matches(origin.distance(candidate.position())))
    }

    fn missing_predicate<L: Lookup>(&self, lookup: &L) -> Option<Span> {
        let mut missing = None;
        self.list_options(|option| {
            if let ListOption::Predicate { id, span } = option
                && missing.is_none()
                && !lookup.has_entry(PREDICATE.location(), id)
            {
                missing = Some(span);
            }
        });
        missing
    }

    /// The entities the selector finds (ARG-ENT-012 to ARG-ENT-014). `random` gives an index
    /// below its argument.
    pub fn select<'c, C: Candidate, L: Lookup>(
        &self,
        source: &Source<'_, L>,
        source_entity: Option<&'c C>,
        candidates: impl Iterator<Item = &'c C>,
        random: &mut impl FnMut(usize) -> usize,
        out: &mut Vec<&'c C>,
    ) -> Result<(), ArgumentError> {
        out.clear();
        if matches!(self.target, Target::Kind(_)) && !source.level.can_use_selectors() {
            return Err(ArgumentError::SelectorNotAllowed);
        }
        if let Some(id) = self.missing_predicate(source.lookup) {
            let registry = PREDICATE.location();
            return Err(ArgumentError::ResourceNotFound { id, registry });
        }
        let [x, y, z] = self.origin;
        let origin = DVec3::new(
            x.unwrap_or(source.position.x),
            y.unwrap_or(source.position.y),
            z.unwrap_or(source.position.z),
        );
        match self.target {
            Target::Name(name) => {
                out.extend(
                    candidates
                        .filter(|c| c.is_player())
                        .find(|c| eq_ignore_case(name.chars(), c.name().chars())),
                );
                return Ok(());
            }
            Target::Uuid(uuid) => {
                out.extend(
                    candidates
                        .filter(|c| self.includes_entities || c.is_player())
                        .find(|c| c.uuid() == uuid),
                );
                return Ok(());
            }
            Target::Kind('s') => {
                out.extend(source_entity.filter(|entity| {
                    (self.includes_entities || entity.is_player()) && self.passes(*entity, origin)
                }));
                self.retain_listed(out, source.lookup);
                return Ok(());
            }
            Target::Kind(_) => {}
        }
        out.extend(candidates.filter(|c| {
            (self.includes_entities || c.is_player())
                && (!self.world_limited || c.in_source_dimension())
                && self.passes(*c, origin)
        }));
        self.retain_listed(out, source.lookup);
        let distance = |c: &&C| origin.distance_squared(c.position());
        match self.order {
            Order::Arbitrary => {}
            Order::Nearest => out.sort_unstable_by(|a, b| distance(a).total_cmp(&distance(b))),
            Order::Furthest => out.sort_unstable_by(|a, b| distance(b).total_cmp(&distance(a))),
            Order::Random => {
                for index in (1..out.len()).rev() {
                    out.swap(index, random(index + 1).min(index));
                }
            }
        }
        if let Some(max) = self.max_results {
            out.truncate(max as usize);
        }
        Ok(())
    }
}
