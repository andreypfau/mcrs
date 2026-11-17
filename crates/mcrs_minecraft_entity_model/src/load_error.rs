use core::fmt;

use mcrs_minecraft_pack_file::SyntaxError;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadError {
    Syntax(SyntaxError),
    NotAnObject,
    MissingFormatVersion,
    UnsupportedFormat { version: String },
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
