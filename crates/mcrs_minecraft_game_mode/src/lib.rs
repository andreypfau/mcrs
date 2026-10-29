//! The game modes of a player, their names and their wire forms.

// std: mcrs_minecraft_wire, whose codecs write through std::io::Write.
mod game_mode;

pub use game_mode::{GameMode, OptGameMode, UnknownGameMode};
