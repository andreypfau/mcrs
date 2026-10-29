//! The client chat screen without rendering: the input box, the history, the client parse and
//! suggestions of commands, the hint lines and the chat log.

// std: mcrs_minecraft_command and mcrs_minecraft_protocol, which hold the argument types and the
// text components, need it.
mod abilities;
mod command_input;
mod edit_box;
mod entries;
mod history;
mod log;
mod normalize;
mod overlay;
mod screen;
mod suggestion_menu;

pub use abilities::{ChatAbilities, ChatAccess};
pub use command_input::{CommandInput, Context, Highlight, Hint, Request};
pub use edit_box::{Anchor, EditBox, Step};
pub use entries::{Entries, Tooltip};
pub use history::History;
pub use log::{ChatLines, ChatLog, Line, Tag, fade};
pub use normalize::normalize;
pub use overlay::Overlay;
pub use screen::{
    ChatScreen, Closed, InputStyle, Key, Outcome, Outgoing, UnsentDraft, keeps_draft,
};
pub use suggestion_menu::{LIST_ROWS, SuggestionMenu};

/// The most code units the input box and a chat message hold (CHAT-BOX-001, CHAT-SEND-001).
pub const MAX_INPUT: usize = 256;
