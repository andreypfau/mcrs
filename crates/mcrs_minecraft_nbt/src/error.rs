use std::{
    fmt::Display,
    io::{self},
};

use serde::{de, ser};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("The root tag of the NBT file is not a compound tag. Received tag id: {0}")]
    NoRootCompound(u8),
    #[error("The root tag is TAG_End")]
    EndRoot,
    #[error("Encountered an unknown NBT tag id: {0}.")]
    UnknownTagId(u8),
    #[error("Failed to Cesu 8 Decode")]
    Cesu8DecodingError,
    #[error("Serde error: {0}")]
    SerdeError(String),
    #[error("NBT doesn't support this type: {0}")]
    UnsupportedType(String),
    #[error("NBT reading was cut short: {0}")]
    Incomplete(io::Error),
    #[error("Negative list length: {0}")]
    NegativeLength(i32),
    #[error("Length too large: {0}")]
    LargeLength(usize),
    #[error("Tried to read NBT tag with too high complexity, depth > {MAX_DEPTH}")]
    TooDeep,
}

/// `NbtAccounter.defaultQuota`: lists and compounds may nest this far.
pub const MAX_DEPTH: u32 = 512;

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::SerdeError(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::SerdeError(msg.to_string())
    }
}
