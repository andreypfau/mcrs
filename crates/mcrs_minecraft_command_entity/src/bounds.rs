use crate::error::EntityError;
use crate::selector::{SelectorResult, error, rewind};
use mcrs_command::Span;
use mcrs_command::{LibraryError, Reader, SyntaxError};
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;

pub(crate) trait BoundNumber: Copy + PartialOrd {
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
pub(crate) fn read_bounds<T: BoundNumber>(
    reader: &mut Reader<'_>,
) -> SelectorResult<MinMaxBounds<T>> {
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

pub(crate) fn is_negative<T: Copy + PartialOrd + Default>(bounds: &MinMaxBounds<T>) -> bool {
    let zero = T::default();
    bounds.min.is_some_and(|v| v < zero) || bounds.max.is_some_and(|v| v < zero)
}
