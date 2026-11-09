use std::error::Error;
use std::fmt;

use mcrs_minecraft_core::Direction;

/// Why the faces of an element do not bake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FaceBakeError {
    /// The corners of a face do not form a box face after the variant turn.
    NotBoxFace { key: Direction },
}

impl fmt::Display for FaceBakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FaceBakeError::NotBoxFace { key } => {
                write!(f, "a {} face is no box face after its turn", key.name())
            }
        }
    }
}

impl Error for FaceBakeError {}
