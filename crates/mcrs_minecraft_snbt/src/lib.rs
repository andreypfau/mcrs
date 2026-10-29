//! The read and the write of stringified NBT as the game reads and writes it.

// std: mcrs_minecraft_nbt, whose reader and writer go through std::io.

extern crate alloc;

mod build;
mod error;
#[cfg(test)]
mod golden;
mod java_number;
mod number;
mod parse;
mod reader;
mod text;
mod write;

pub use error::{IntegerType, SnbtError, SnbtErrorKind};
pub use java_number::{java_double, java_float};
pub use parse::{check_compound, parse_compound, parse_tag, read_compound, read_tag};
pub use write::{write, write_compound};

#[cfg(test)]
mod tests;
