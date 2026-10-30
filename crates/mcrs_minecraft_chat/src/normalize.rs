use crate::max_input::MAX_INPUT;
use alloc::string::String;
use mcrs_command::is_whitespace;

/// The text a send carries, written into `out`.
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
    let end = out.trim_end_matches(' ').len();
    out.truncate(end);
    let start = out.len() - out.trim_start_matches(' ').len();
    out.drain(..start);
    if let Some((kept, _)) = out.char_indices().nth(MAX_INPUT) {
        out.truncate(kept);
    }
}
