use core::error::Error;
use core::fmt;

use mcrs_minecraft_resource_pack::{PackError, PngError};

use crate::unihex_error::UnihexError;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FontError {
    Pack(PackError),
    Png { texture: String, error: PngError },
    Grid { texture: String },
    Unihex { archive: String, error: UnihexError },
    SelfReference { id: String },
    InvalidId { id: String, reason: String },
}

impl fmt::Display for FontError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pack(error) => write!(formatter, "{error}"),
            Self::Png { texture, error } => write!(formatter, "cannot decode {texture}: {error}"),
            Self::Grid { texture } => {
                write!(formatter, "{texture}: the texture is smaller than its grid")
            }
            Self::Unihex { archive, error } => write!(formatter, "{archive}: {error}"),
            Self::SelfReference { id } => write!(formatter, "the font {id} references itself"),
            Self::InvalidId { id, reason } => write!(formatter, "{id:?} is not an id: {reason}"),
        }
    }
}

impl Error for FontError {}

impl From<PackError> for FontError {
    fn from(error: PackError) -> Self {
        Self::Pack(error)
    }
}
