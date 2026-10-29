/// Whitespace as the Brigadier reader skips it.
pub const fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | '\u{1c}'..='\u{20}'
            | '\u{1680}'
            | '\u{2000}'..='\u{2006}'
            | '\u{2008}'..='\u{200a}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{2028}'
            | '\u{2029}'
    )
}
