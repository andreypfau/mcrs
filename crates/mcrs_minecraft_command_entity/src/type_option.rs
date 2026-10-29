use crate::error::EntityError;
use crate::invertible::{Invertible, read_negation};
use crate::list_option::ListOption;
use crate::options::read_options;
use crate::selector::{SelectorResult, State, rewind};
use mcrs_command::{Reader, StringSpan};
use mcrs_minecraft_command::{Identifier, SpannedIdentifier, read_identifier};
use mcrs_minecraft_entity::keys::EntityType;

pub(crate) fn read_type<'a>(
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
// chisle: re-reads the option prefix per `type=#tag`, O(n^2) in the options; a heap-free seen-set in `State` lifts it
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

fn entity_type(id: Identifier<'_>) -> Option<EntityType> {
    EntityType::ALL
        .iter()
        .copied()
        .find(|entity_type| id.eq_str(entity_type.as_static_str()))
}
