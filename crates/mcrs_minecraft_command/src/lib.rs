//! The command source, permissions, identifiers and the error text that the vanilla argument
//! families share, and the `commands` and suggestion packets of a player.

// std: mcrs_minecraft_core needs it.
mod error;
mod identifier;
mod packet;
mod permission;
mod source;

pub use error::{
    ErrorText, ResourceError, error_feedback, error_message, identifier_string, identifier_text,
    parse_error_line, snbt_text, span_text, string_text,
};
pub use identifier::{
    Identifier, SpannedIdentifier, identifier_matches, prefixed_identifier_matches,
    read_identifier, suggest_prefixed, text_matches,
};
pub use packet::{
    PlayerTree, SuggestionEntry, SuggestionReply, WireArgument, player_tree, suggestion_reply,
};
pub use permission::{CommandLevel, PermissionCheck};
pub use source::{Lookup, NoLookup, Source, anchored};
