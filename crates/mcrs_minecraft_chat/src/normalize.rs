use crate::{MAX_INPUT, utf16_prefix};
use mcrs_command::is_whitespace;

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
