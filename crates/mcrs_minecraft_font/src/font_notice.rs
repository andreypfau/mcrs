use core::fmt;

use crate::definition_error::DefinitionError;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FontNotice {
    IgnoredFile {
        file: String,
        reason: DefinitionError,
    },
    UnknownProvider {
        file: String,
    },
}

impl fmt::Display for FontNotice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IgnoredFile { file, reason } => write!(formatter, "{file} is ignored: {reason}"),
            Self::UnknownProvider { file } => write!(
                formatter,
                "{file}: a provider of a kind this client does not read is skipped"
            ),
        }
    }
}
