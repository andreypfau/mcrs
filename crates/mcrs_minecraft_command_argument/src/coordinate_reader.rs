use crate::coordinates::Coordinates;
use crate::coordinates_error::CoordinatesError;
use crate::world_coordinate::WorldCoordinate;
use mcrs_command::{
    LibraryError, Reader, SEPARATOR, SuggestionBuilder, SuggestionText, SyntaxError,
};

/// One coordinate: `~` with an optional offset, or an absolute value that `absolute` reads.
pub(crate) fn read_axis(
    reader: &mut Reader<'_>,
    missing: CoordinatesError,
    expected: LibraryError,
    absolute: impl FnOnce(&mut Reader<'_>) -> Result<f64, SyntaxError<CoordinatesError>>,
) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>> {
    if reader.peek_is('^') {
        return Err(SyntaxError::argument(
            CoordinatesError::PosMixed,
            reader.cursor(),
        ));
    }
    if reader.at_end() {
        return Err(SyntaxError::argument(missing, reader.cursor()));
    }
    if reader.peek_is('~') {
        reader.skip();
        let value = if reader.at_end() || reader.peek_is(SEPARATOR) {
            0.0
        } else {
            reader.read_double()?
        };
        return Ok(WorldCoordinate {
            value,
            relative: true,
        });
    }
    if reader.peek_is(SEPARATOR) {
        return Err(SyntaxError::library(expected, reader.cursor()));
    }
    Ok(WorldCoordinate {
        value: absolute(reader)?,
        relative: false,
    })
}

/// Where an absolute integer value of a world coordinate lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Centre {
    /// An integer value moves to the centre of its block.
    Block,
    /// The value stays as written.
    Exact,
}

/// Whether a read of three axes accepts the end of the input after a complete axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Completeness {
    Full,
    Partial,
}

/// One world coordinate.
pub(crate) fn read_world(
    reader: &mut Reader<'_>,
    centre: Centre,
) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>> {
    read_axis(
        reader,
        CoordinatesError::PosMissingDouble,
        LibraryError::ExpectedDouble,
        |reader| {
            let start = reader.cursor();
            let value = reader.read_double()?;
            Ok(
                if centre == Centre::Block
                    && !reader.span_from(start).text(reader.input()).contains('.')
                {
                    value + 0.5
                } else {
                    value
                },
            )
        },
    )
}

pub(crate) fn separator(
    reader: &mut Reader<'_>,
    start: usize,
    error: CoordinatesError,
) -> Result<(), SyntaxError<CoordinatesError>> {
    if reader.peek_is(SEPARATOR) {
        reader.skip();
        Ok(())
    } else {
        reader.set_cursor(start);
        Err(SyntaxError::argument(error, start))
    }
}

/// Three axes apart by one separator each. A partial read stops without an error at the end
/// of the input after a complete axis; the axes it did not read keep `unread`.
pub(crate) fn read_axes<T: Copy>(
    reader: &mut Reader<'_>,
    completeness: Completeness,
    unread: T,
    mut read_axis: impl FnMut(&mut Reader<'_>, usize) -> Result<T, SyntaxError<CoordinatesError>>,
) -> Result<[T; 3], SyntaxError<CoordinatesError>> {
    let start = reader.cursor();
    let mut values = [unread; 3];
    for (axis, value) in values.iter_mut().enumerate() {
        if axis > 0 {
            if completeness == Completeness::Partial && reader.at_end() {
                break;
            }
            separator(reader, start, CoordinatesError::Pos3dIncomplete)?;
        }
        *value = read_axis(reader, axis)?;
    }
    Ok(values)
}

/// Three local values in the order left, up, forwards.
pub(crate) fn read_local(
    reader: &mut Reader<'_>,
    completeness: Completeness,
) -> Result<[f64; 3], SyntaxError<CoordinatesError>> {
    let start = reader.cursor();
    read_axes(reader, completeness, 0.0, |reader, _| {
        if reader.at_end() {
            return Err(SyntaxError::argument(
                CoordinatesError::PosMissingDouble,
                reader.cursor(),
            ));
        }
        if !reader.peek_is('^') {
            reader.set_cursor(start);
            return Err(SyntaxError::argument(CoordinatesError::PosMixed, start));
        }
        reader.skip();
        if reader.at_end() || reader.peek_is(SEPARATOR) {
            Ok(0.0)
        } else {
            reader.read_double()
        }
    })
}

pub(crate) fn read_coordinates(
    reader: &mut Reader<'_>,
    completeness: Completeness,
    axis: impl FnMut(&mut Reader<'_>, usize) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>>,
) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
    if reader.peek_is('^') {
        return read_local(reader, completeness).map(Coordinates::Local);
    }
    let unread = WorldCoordinate {
        value: 0.0,
        relative: true,
    };
    read_axes(reader, completeness, unread, axis).map(Coordinates::World)
}

pub(crate) fn suggest_coordinates<'t, T>(
    builder: &mut SuggestionBuilder<'t, '_, T>,
    reads: impl Fn(&mut Reader<'_>) -> bool,
) {
    let remaining = builder.remaining();
    let trimmed = remaining.strip_suffix(SEPARATOR).unwrap_or(remaining);
    if !trimmed.is_empty() && !reads(&mut Reader::new(trimmed)) {
        return;
    }
    let [one, two, three] = if remaining.starts_with('^') {
        ["^", " ^", " ^ ^"]
    } else {
        ["~", " ~", " ~ ~"]
    };
    match trimmed.split(SEPARATOR).count() {
        _ if trimmed.is_empty() => {
            builder.suggest(SuggestionText::new(one), None);
            builder.suggest(SuggestionText::concat(one, two), None);
            builder.suggest(SuggestionText::concat(one, three), None);
        }
        1 => {
            builder.suggest(SuggestionText::concat(trimmed, two), None);
            builder.suggest(SuggestionText::concat(trimmed, three), None);
        }
        2 => builder.suggest(SuggestionText::concat(trimmed, two), None),
        _ => {}
    }
}

/// One block coordinate.
pub(crate) fn read_block(
    reader: &mut Reader<'_>,
) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>> {
    read_axis(
        reader,
        CoordinatesError::PosMissingInt,
        LibraryError::ExpectedInt,
        |reader| Ok(f64::from(reader.read_int::<CoordinatesError>()?)),
    )
}
