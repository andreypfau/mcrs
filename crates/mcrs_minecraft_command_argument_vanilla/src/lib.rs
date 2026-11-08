//! The vanilla argument set, its errors, its parsers on the wire and the client tree.

// std: mcrs_minecraft_core needs it.
mod client;
mod error;
mod nbt_compound_argument;
mod vanilla_argument;
mod vanilla_lookup;

pub use client::{ClientArgument, ClientGraph, SuggestionType, TreeError, client_tree};
pub use error::{ArgumentError, CommandSyntaxError};
pub use nbt_compound_argument::{NbtCompoundArgument, NbtCompoundText};
pub use vanilla_argument::{UnknownParser, VanillaArgument, VanillaGraph};
pub use vanilla_lookup::VanillaLookup;
