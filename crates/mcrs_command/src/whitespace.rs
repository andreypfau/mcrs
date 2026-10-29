/// Whitespace that an argument type skips: Unicode `White_Space`, without the
/// no-break spaces and U+0085, which Brigadier does not skip.
pub const fn is_whitespace(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\u{85}' | '\u{a0}' | '\u{2007}' | '\u{202f}')
}
