use std::fmt;

use mcrs_minecraft_protocol::Error;

#[derive(Debug)]
#[non_exhaustive]
pub enum ColumnDecodeError {
    Sections(Error),
    Light(Error),
    UnknownBiome(u8),
}

impl fmt::Display for ColumnDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnDecodeError::Sections(error) | ColumnDecodeError::Light(error) => {
                write!(f, "{error}")
            }
            ColumnDecodeError::UnknownBiome(number) => {
                write!(f, "biome {number} is not one the session registries hold")
            }
        }
    }
}

impl std::error::Error for ColumnDecodeError {}
