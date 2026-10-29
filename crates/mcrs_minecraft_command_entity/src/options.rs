use crate::error::EntityError;
use crate::option_key::{KEYS, Key};
use crate::selector::{Next, SORTS, SelectorResult, State, error, rewind};
use mcrs_command::{LibraryError, Reader, StringSpan, StringValue, SyntaxError};
use mcrs_minecraft_command::{Identifier, SpannedIdentifier, by_name, read_identifier};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_snbt::check_compound;
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use mcrs_text_span::Span;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Invertible {
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
    pub(crate) fn prefixes(self) -> &'static [&'static str] {
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
    fn read(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<EntityError>>;
}

impl BoundNumber for i32 {
    const INVALID: fn(Span) -> LibraryError = LibraryError::InvalidInt;
    const SWAPPABLE: bool = false;
    fn read(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<EntityError>> {
        reader.read_int()
    }
}

impl BoundNumber for f64 {
    const INVALID: fn(Span) -> LibraryError = LibraryError::InvalidDouble;
    const SWAPPABLE: bool = false;
    fn read(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<EntityError>> {
        reader.read_double()
    }
}

impl BoundNumber for f32 {
    const INVALID: fn(Span) -> LibraryError = LibraryError::InvalidFloat;
    const SWAPPABLE: bool = true;
    fn read(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<EntityError>> {
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

fn parse_bound<T: BoundNumber>(
    input: &str,
    token: Span,
    start: usize,
) -> SelectorResult<Option<T>> {
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
fn read_bounds<T: BoundNumber>(reader: &mut Reader<'_>) -> SelectorResult<MinMaxBounds<T>> {
    let start = reader.cursor();
    if reader.at_end() {
        return error(EntityError::RangeEmpty, reader.cursor());
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
        return rewind(reader, start, EntityError::RangeEmpty);
    }
    let input = reader.input();
    let (min, max) = parse_bound::<T>(input, first, start)
        .and_then(|min| Ok((min, parse_bound::<T>(input, second, start)?)))
        .inspect_err(|_| reader.set_cursor(start))?;
    if let (Some(min), Some(max)) = (min, max)
        && !T::SWAPPABLE
        && min > max
    {
        return rewind(reader, start, EntityError::RangeSwapped);
    }
    Ok(MinMaxBounds { min, max })
}

fn is_negative<T: Copy + PartialOrd + Default>(bounds: &MinMaxBounds<T>) -> bool {
    let zero = T::default();
    bounds.min.is_some_and(|v| v < zero) || bounds.max.is_some_and(|v| v < zero)
}

/// A map of `{key=value, ...}` whose values `value` reads (the `scores` and `advancements`
/// options). Returns whether the map has no entry.
fn read_map(
    reader: &mut Reader<'_>,
    key: fn(&mut Reader<'_>) -> SelectorResult<()>,
    value: fn(&mut Reader<'_>) -> SelectorResult<()>,
) -> SelectorResult<bool> {
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

fn read_unquoted_key(reader: &mut Reader<'_>) -> SelectorResult<()> {
    reader.read_unquoted();
    Ok(())
}

fn read_progress(reader: &mut Reader<'_>) -> SelectorResult<()> {
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

fn read_scalar(reader: &mut Reader<'_>, state: &mut State<'_>, key: Key) -> SelectorResult<()> {
    let start = reader.cursor();
    let input = reader.input();
    let selector = &mut state.selector;
    match key {
        Key::Distance => {
            let bounds = read_bounds::<f64>(reader)?;
            if is_negative(&bounds) {
                return rewind(reader, start, EntityError::DistanceNegative);
            }
            selector.distance = Some(bounds);
            selector.world_limited = true;
        }
        Key::Level => {
            let bounds = read_bounds::<i32>(reader)?;
            if is_negative(&bounds) {
                return rewind(reader, start, EntityError::LevelNegative);
            }
            selector.level = Some(bounds);
            selector.includes_entities = false;
        }
        Key::X | Key::Y | Key::Z | Key::Dx | Key::Dy | Key::Dz => {
            let value = reader.read_double()?;
            let slot = match key {
                Key::X => &mut selector.origin[0],
                Key::Y => &mut selector.origin[1],
                Key::Z => &mut selector.origin[2],
                Key::Dx => &mut selector.size[0],
                Key::Dy => &mut selector.size[1],
                _ => &mut selector.size[2],
            };
            *slot = Some(value);
            selector.world_limited = true;
        }
        Key::XRotation => selector.x_rotation = Some(read_bounds::<f32>(reader)?),
        Key::YRotation => selector.y_rotation = Some(read_bounds::<f32>(reader)?),
        Key::Limit => {
            let limit = reader.read_int()?;
            if limit < 1 {
                return rewind(reader, start, EntityError::LimitTooSmall);
            }
            selector.max_results = u32::try_from(limit).ok();
            state.limit_set = true;
        }
        Key::Sort => {
            let word = reader.read_unquoted();
            let Some(order) = by_name(&SORTS, word.text(input)) else {
                return rewind(reader, start, EntityError::SortIrreversible(word));
            };
            selector.order = order;
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
) -> SelectorResult<()> {
    let inapplicable = EntityError::OptionsInapplicable(key_span);
    let start = reader.cursor();
    let input = reader.input();
    let selector = &mut state.selector;
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
            let Ok(mode) = word.text(input).parse::<GameMode>() else {
                return rewind(reader, start, EntityError::ModeInvalid(word));
            };
            selector.includes_entities = false;
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
        Key::Nbt => {
            let negated = read_negation(reader);
            reader.skip_whitespace();
            let end = check_compound(input, reader.cursor())
                .map_err(|error| SyntaxError::argument(EntityError::Nbt(error), error.position))?;
            reader.set_cursor(end);
            visit(ListOption::Nbt { negated });
        }
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
            let SpannedIdentifier {
                identifier: id,
                span,
            } = read_identifier(reader)?;
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
) -> SelectorResult<()> {
    let inapplicable = EntityError::OptionsInapplicable(key_span);
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
        let tag = read_identifier(reader)?.identifier;
        if state.check_duplicates && type_tag_seen(input, options_start, start, tag) {
            return rewind(reader, start, inapplicable);
        }
        state.types = Invertible::Multiple;
        visit(ListOption::TypeTag { tag, negated });
    } else {
        if !state.types.allows(negated) {
            return rewind(reader, start, inapplicable);
        }
        let SpannedIdentifier { identifier, span } = read_identifier(reader)?;
        let Some(entity_type) = entity_type(identifier) else {
            return rewind(reader, start, EntityError::TypeInvalid(span));
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
pub(crate) fn read_options<'a>(
    reader: &mut Reader<'a>,
    state: &mut State<'a>,
    visit: &mut impl FnMut(ListOption<'a>),
) -> SelectorResult<()> {
    let options_start = reader.cursor();
    loop {
        reader.skip_whitespace();
        state.stop_at(Next::Key, reader.cursor());
        if reader.at_end() || reader.peek_is(']') {
            break;
        }
        let key_start = reader.cursor();
        let key_span = reader.read_string()?;
        let name = StringValue::new(reader.input(), key_span);
        let Some(key) = KEYS
            .iter()
            .find(|entry| name == entry.name)
            .map(|entry| entry.key)
        else {
            return rewind(reader, key_start, EntityError::OptionsUnknown(key_span));
        };
        if !state.available(key) {
            return rewind(
                reader,
                key_start,
                EntityError::OptionsInapplicable(key_span),
            );
        }
        reader.skip_whitespace();
        if !reader.peek_is('=') {
            return rewind(reader, key_start, EntityError::OptionsValueless(key_span));
        }
        reader.skip();
        reader.skip_whitespace();
        let next = match key {
            Key::Sort => Next::Sort,
            Key::GameMode => Next::GameMode,
            Key::Type => Next::Type,
            _ => Next::Nothing,
        };
        state.stop_at(next, reader.cursor());
        read_value(reader, state, key, key_span, options_start, visit)?;
        reader.skip_whitespace();
        state.stop_at(Next::AfterValue, reader.cursor());
        if reader.at_end() || reader.peek_is(']') {
            break;
        }
        if !reader.peek_is(',') {
            return error(EntityError::OptionsUnterminated, reader.cursor());
        }
        reader.skip();
    }
    if reader.at_end() {
        return error(EntityError::OptionsUnterminated, reader.cursor());
    }
    reader.skip();
    state.stop_at(Next::Nothing, reader.cursor());
    Ok(())
}
