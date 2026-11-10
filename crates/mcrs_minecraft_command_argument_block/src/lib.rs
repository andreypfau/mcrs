//! The `minecraft:block_predicate` and `minecraft:block_state` arguments and the block lookup
//! they read.

// std: mcrs_minecraft_core needs it.
mod block_lookup;
mod block_predicate;
mod block_state_argument;
mod block_suggestion;
mod error;

pub use block_lookup::BlockLookup;
pub use block_predicate::{BlockPredicate, BlockPredicateArgument};
pub use block_state_argument::{BlockStateArgument, BlockStateInput};
pub use error::{BlockError, BlockSubject};
