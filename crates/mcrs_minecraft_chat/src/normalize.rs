use crate::{MAX_INPUT, utf16_prefix};

fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | '\u{1c}'..='\u{20}'
            | '\u{1680}'
            | '\u{2000}'..='\u{2006}'
            | '\u{2008}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{205f}'
            | '\u{3000}'
    )
}

/// The text a send carries (CHAT-SEND-001), written into `out`.
pub fn normalize(input: &str, out: &mut String) {
    out.clear();
    let trimmed = input.trim_matches(|c: char| c <= '\u{20}');
    let mut pending_space = false;
    for c in trimmed.chars() {
        if is_whitespace(c) {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(if c == '\u{a0}' { ' ' } else { c });
    }
    let kept = utf16_prefix(out, MAX_INPUT).len();
    out.truncate(kept);
}
