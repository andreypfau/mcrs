use crate::bounds::{is_negative, read_bounds};
use crate::error::EntityError;
use crate::option_key::Key;
use crate::selector::{SORTS, SelectorResult, State, rewind};
use mcrs_command::Reader;
use mcrs_minecraft_command_argument::by_name;

pub(crate) fn read_scalar(
    reader: &mut Reader<'_>,
    state: &mut State<'_>,
    key: Key,
) -> SelectorResult<()> {
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
