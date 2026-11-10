//! A language file of the game: translation patterns by key, and the filler of their `%s`,
//! `%n$s` and `%%` placeholders, and the text of a number argument.

// std: the translations are a HashMap, and mcrs_minecraft_nbt needs std.
mod language;
mod pattern;
mod primitive;

pub use language::{EN_US, Language};
pub use pattern::{Piece, fill_pattern};
pub use primitive::primitive_text;
