use std::error::Error;
use std::fmt;

use png::DecodingError;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PngError {
    Truncated,
    Malformed { message: String },
    TooLarge,
    Layout,
}

impl fmt::Display for PngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PngError::Truncated => f.write_str("the PNG data ends early"),
            PngError::Malformed { message } => write!(f, "the PNG data is malformed: {message}"),
            PngError::TooLarge => f.write_str("the PNG image is past the decoder limits"),
            PngError::Layout => {
                f.write_str("the PNG decoder gave pixels that do not expand to RGBA")
            }
        }
    }
}

impl Error for PngError {}

impl From<DecodingError> for PngError {
    fn from(error: DecodingError) -> Self {
        match error {
            DecodingError::IoError(_) => PngError::Truncated,
            DecodingError::LimitsExceeded => PngError::TooLarge,
            DecodingError::Format(error) => PngError::Malformed {
                message: error.to_string(),
            },
            DecodingError::Parameter(error) => PngError::Malformed {
                message: error.to_string(),
            },
        }
    }
}
