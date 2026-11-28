use core::error::Error;
use core::fmt;

use mcrs_minecraft_resource_pack::PackError;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LanguageError {
    Pack(PackError),
    Json { file: String, reason: String },
}

impl fmt::Display for LanguageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pack(error) => write!(formatter, "{error}"),
            Self::Json { file, reason } => write!(formatter, "{file}: {reason}"),
        }
    }
}

impl Error for LanguageError {}
