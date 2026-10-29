/// The length of `text` in UTF-16 code units, the unit of a string limit on the wire.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The UTF-16 position of the byte offset `offset` in `text`, the unit of a position on the
/// wire. An offset inside a character counts from the start of that character.
pub fn utf16_position(text: &str, offset: usize) -> usize {
    text.get(..text.floor_char_boundary(offset))
        .map_or(0, utf16_len)
}

/// The byte offset of the UTF-16 position `position` in `text`, clamped to its end. A position
/// inside a surrogate pair gives the offset after the pair.
pub fn utf16_byte_offset(text: &str, position: usize) -> usize {
    let mut units = 0;
    for (offset, c) in text.char_indices() {
        if units >= position {
            return offset;
        }
        units += c.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_byte_offset_survives_the_round_trip_through_utf16() {
        let text = "a\u{e9}\u{1f600}b";
        let positions: Vec<usize> = text
            .char_indices()
            .map(|(offset, _)| utf16_position(text, offset))
            .collect();
        assert_eq!(positions, [0, 1, 2, 4]);
        for (offset, _) in text.char_indices() {
            assert_eq!(
                utf16_byte_offset(text, utf16_position(text, offset)),
                offset
            );
        }
        assert_eq!(utf16_byte_offset(text, 3), 7, "inside a pair rounds up");
        assert_eq!(utf16_byte_offset(text, 99), text.len());
    }
}
