use crate::bounds::read_bounds;
use crate::error::EntityError;
use crate::invertible::{Invertible, read_negation};
use crate::list_option::ListOption;
use crate::option_key::{KEYS, Key};
use crate::option_map::{read_map, read_progress, read_unquoted_key};
use crate::scalar_option::read_scalar;
use crate::selector::{Next, SelectorResult, State, error, rewind};
use crate::type_option::read_type;
use mcrs_command::{Reader, StringSpan, StringValue, SyntaxError};
use mcrs_minecraft_command::{SpannedIdentifier, read_identifier};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_snbt::check_compound;

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
