#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

mod build;
mod error;
#[cfg(test)]
mod golden;
mod java_number;
mod lenient_uuid;
mod number;
mod parse;
mod reader;
mod text;
mod write;

pub use error::{IntegerType, SnbtError, SnbtErrorKind};
pub use java_number::{java_double, java_float};
pub use lenient_uuid::parse_lenient_uuid;
pub use parse::{check_compound, parse_compound, parse_tag, read_compound, read_tag};
pub use write::{write, write_compound};

#[cfg(test)]
mod tests;
