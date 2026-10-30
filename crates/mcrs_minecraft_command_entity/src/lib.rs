//! The `minecraft:entity` argument: the selector, its options and the search over entities.

// std: mcrs_minecraft_core and mcrs_minecraft_text, which hold the identifiers and the text
// of every argument, need it.
mod bounds;
mod entity_argument;
mod error;
mod invertible;
mod list_option;
mod option_key;
mod option_map;
mod options;
mod scalar_option;
mod search;
mod selector;
mod type_option;

pub use entity_argument::EntityArgument;
pub use error::EntityError;
pub use search::{Candidate, EntityBounds};
pub use selector::{Selector, SelectorKind, Target};
