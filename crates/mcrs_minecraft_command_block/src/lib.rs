//! The `minecraft:block_predicate` argument and the block lookup it reads.

// std: mcrs_minecraft_core and mcrs_minecraft_protocol, which hold the identifiers and the text
// of every argument, need it.
mod block_lookup;
mod block_predicate;
mod block_suggestion;
mod error;

pub use block_lookup::BlockLookup;
pub use block_predicate::{BlockPredicate, BlockPredicateArgument};
pub use error::{BlockError, BlockSubject};
