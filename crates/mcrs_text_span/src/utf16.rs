/// The position counted in UTF-16 code units, the unit of every position on the wire.
pub fn utf16_position(input: &str, position: usize) -> usize {
    input
        .get(..input.floor_char_boundary(position))
        .map_or(0, |start| start.encode_utf16().count())
}

/// The length of a text in UTF-16 code units, the unit of every limit and position on the wire.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The longest start of `text` that holds at most `max` UTF-16 code units, never splitting a
/// character.
pub fn utf16_prefix(text: &str, max: usize) -> &str {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        units += c.len_utf16();
        if units > max {
            return text.get(..index).unwrap_or_default();
        }
    }
    text
}

/// The byte offset of a UTF-16 position in `text`, clamped to its end.
pub fn byte_offset(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        if units >= utf16 {
            return index;
        }
        units += c.len_utf16();
    }
    text.len()
}
