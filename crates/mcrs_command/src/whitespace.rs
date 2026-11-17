pub const fn is_whitespace(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\u{85}' | '\u{a0}' | '\u{2007}' | '\u{202f}')
}
