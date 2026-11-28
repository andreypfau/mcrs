use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackError {
    Duplicate { path: String },
    Missing { path: String },
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::Duplicate { path } => {
                write!(
                    f,
                    "{path}: a pack layer supplies a file the client pack holds"
                )
            }
            PackError::Missing { path } => write!(f, "the resource pack has no {path}"),
        }
    }
}

impl Error for PackError {}
