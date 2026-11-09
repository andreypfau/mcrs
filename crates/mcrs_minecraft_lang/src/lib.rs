//! A language file of the game: translation patterns by key, and the filler of their `%s`,
//! `%n$s` and `%%` placeholders.

// std: the translations are a HashMap.
mod language;
mod pattern;

pub use language::{EN_US, Language};
pub use pattern::{Piece, fill_pattern};
