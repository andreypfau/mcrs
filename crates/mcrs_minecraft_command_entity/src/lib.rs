//! The `minecraft:entity` argument: the selector, its options and the search over entities.

// std: mcrs_minecraft_core and mcrs_minecraft_protocol, which hold the identifiers and the text
// of every argument, need it.
mod entity_argument;
mod error;
mod option_key;
mod options;
mod search;
mod selector;

pub use entity_argument::EntityArgument;
pub use error::EntityError;
pub use search::{Candidate, EntityBounds};
pub use selector::{Selector, SelectorKind, Target};
