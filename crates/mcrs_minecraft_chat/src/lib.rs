// std: mcrs_minecraft_command and mcrs_minecraft_protocol, which hold the argument types and the
// text components, need it.
mod ability;
mod command;
mod edit;
mod history;
mod log;
mod normalize;
mod screen;

pub use ability::ChatAbilities;
pub use command::{
    CommandInput, Context, Entries, Highlight, Hint, LIST_ROWS, Request, SuggestionList, Tooltip,
};
pub use edit::EditBox;
pub use history::History;
pub use log::{ChatLines, ChatLog, Line, Overlay, Tag, fade};
pub use normalize::normalize;
pub use screen::{ChatScreen, Closed, Key, Outcome, Send, keeps_draft};

/// The most code units the input box and a chat message hold (CHAT-BOX-001, CHAT-SEND-001).
pub const MAX_INPUT: usize = 256;

/// The length of a text in UTF-16 code units, the unit of every limit and position on the wire.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The longest start of `text` that holds at most `max` UTF-16 code units, never splitting a
/// character.
pub(crate) fn utf16_prefix(text: &str, max: usize) -> &str {
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
pub(crate) fn byte_offset(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        if units >= utf16 {
            return index;
        }
        units += c.len_utf16();
    }
    text.len()
}
