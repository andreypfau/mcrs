use core::fmt;

use mcrs_minecraft_pack_file::SyntaxError;

/// A content error that rejects a whole geometry file.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadError {
    /// The text is not lenient JSON.
    Syntax(SyntaxError),
    /// The root of the file is not an object.
    NotAnObject,
    /// The file has no `format_version` string.
    MissingFormatVersion,
    /// No shape has this `format_version`.
    UnsupportedFormat { version: String },
    /// A file of the current shape has no array `minecraft:geometry`.
    MissingGeometries,
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Syntax(error) => error.fmt(formatter),
            LoadError::NotAnObject => formatter.write_str("the root is not an object"),
            LoadError::MissingFormatVersion => {
                formatter.write_str("the file has no `format_version` string")
            }
            LoadError::UnsupportedFormat { version } => {
                write!(formatter, "`format_version` `{version}` is not supported")
            }
            LoadError::MissingGeometries => {
                formatter.write_str("the file has no array `minecraft:geometry`")
            }
        }
    }
}

impl core::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            LoadError::Syntax(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SyntaxError> for LoadError {
    fn from(error: SyntaxError) -> Self {
        LoadError::Syntax(error)
    }
}
