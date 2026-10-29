//! The vanilla argument set, its errors, the `commands` packet of a player and the client tree.

// std: mcrs_minecraft_core and mcrs_minecraft_protocol, which hold the identifiers and the
// command packets, need it.
mod client;
mod error;
mod packet;
mod vanilla_argument;
mod vanilla_lookup;

pub use client::{ClientArgument, ClientGraph, SuggestionType, TreeError, client_tree};
pub use error::{ArgumentError, CommandSyntaxError};
pub use packet::{VanillaGraph, player_tree, suggestion_reply};
pub use vanilla_argument::{UnknownParser, VanillaArgument};
pub use vanilla_lookup::VanillaLookup;
