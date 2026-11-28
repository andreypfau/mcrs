use core::error::Error;
use core::fmt;

use mcrs_minecraft_resource::InvalidResourceLocation;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DefinitionError {
    Json {
        reason: String,
    },
    InvalidId(InvalidResourceLocation),
    AscentAboveHeight {
        texture: String,
        ascent: i32,
        height: i32,
    },
    UnevenRows {
        texture: String,
    },
    UnorderedOverride {
        hex_file: String,
        from: char,
        to: char,
    },
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json { reason } => write!(formatter, "{reason}"),
            Self::InvalidId(error) => write!(formatter, "{error}"),
            Self::AscentAboveHeight {
                texture,
                ascent,
                height,
            } => write!(
                formatter,
                "{texture}: the ascent {ascent} is above the height {height}"
            ),
            Self::UnevenRows { texture } => {
                write!(formatter, "{texture}: the rows of chars differ in length")
            }
            Self::UnorderedOverride { hex_file, from, to } => write!(
                formatter,
                "{hex_file}: an override from {from:?} is not below {to:?}"
            ),
        }
    }
}

impl Error for DefinitionError {}
