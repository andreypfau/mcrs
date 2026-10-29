use crate::selector::SelectorResult;
use mcrs_command::Reader;

/// A map of `{key=value, ...}` whose values `value` reads (the `scores` and `advancements`
/// options). Returns whether the map has no entry.
pub(crate) fn read_map(
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

pub(crate) fn read_unquoted_key(reader: &mut Reader<'_>) -> SelectorResult<()> {
    reader.read_unquoted();
    Ok(())
}

pub(crate) fn read_progress(reader: &mut Reader<'_>) -> SelectorResult<()> {
    let read_bool = |reader: &mut Reader<'_>| reader.read_bool().map(drop);
    if reader.peek_is('{') {
        read_map(reader, read_unquoted_key, read_bool).map(drop)
    } else {
        read_bool(reader)
    }
}
