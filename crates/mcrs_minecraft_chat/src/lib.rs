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
