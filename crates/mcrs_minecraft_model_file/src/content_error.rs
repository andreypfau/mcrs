use std::error::Error;
use std::fmt;

use crate::block_state_file_error::BlockStateFileError;
use crate::face_bake_error::FaceBakeError;
use crate::model_file_error::ModelFileError;

#[derive(Clone, Debug, PartialEq)]
pub struct ContentError {
    pub path: String,
    pub kind: ContentErrorKind,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ContentErrorKind {
    Model(ModelFileError),
    BlockState(BlockStateFileError),
    FaceBake(FaceBakeError),
    NoModel,
    NoBlockState,
    ModelCycle { parent: String },
    NoPart { state: Vec<usize> },
    Location { reason: String },
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ContentErrorKind::ModelCycle { parent } => {
                write!(f, "model {}: its chain has a cycle at {parent}", self.path)
            }
            kind => write!(f, "{}: {kind}", self.path),
        }
    }
}

impl fmt::Display for ContentErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentErrorKind::Model(error) => error.fmt(f),
            ContentErrorKind::BlockState(error) => error.fmt(f),
            ContentErrorKind::FaceBake(error) => error.fmt(f),
            ContentErrorKind::NoModel => f.write_str("no such model"),
            ContentErrorKind::NoBlockState => f.write_str("no such blockstate file"),
            ContentErrorKind::ModelCycle { parent } => {
                write!(f, "its chain has a cycle at {parent}")
            }
            ContentErrorKind::NoPart { state } => {
                write!(f, "no part for the state {state:?}")
            }
            ContentErrorKind::Location { reason } => f.write_str(reason),
        }
    }
}

impl Error for ContentError {}
