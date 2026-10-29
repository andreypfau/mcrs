// std: mcrs_minecraft_core and mcrs_minecraft_protocol, which hold the identifiers and the text
// of every argument, need it.
mod entity_argument;
mod error;
mod search;
mod selector;

pub use entity_argument::EntityArgument;
pub use error::EntityError;
pub use search::Candidate;
pub use selector::{Selector, Target};
