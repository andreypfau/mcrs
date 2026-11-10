use std::error::Error;
use std::fmt;

use crate::block_state_file_error::BlockStateFileError;
use crate::face_bake_error::FaceBakeError;
use crate::model_file_error::ModelFileError;

/// A defect in one file of a pack. The file is skipped and the bake goes on.
#[derive(Clone, Debug, PartialEq)]
pub struct ContentError {
    /// The pack path of the file, or the identifier of the model for a defect of its chain.
    pub path: String,
    pub kind: ContentErrorKind,
}

/// What is wrong with a file of a pack.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ContentErrorKind {
    /// The model file is wrong.
    Model(ModelFileError),
    /// The blockstate file, or one key of it, is wrong.
    BlockState(BlockStateFileError),
    /// The faces of a model do not bake.
    FaceBake(FaceBakeError),
    /// A model that a file names is not in the pack.
    NoModel,
    /// The pack has no blockstate file for a block.
    NoBlockState,
    /// The parent chain of a model leads back to `parent`.
    ModelCycle { parent: String },
    /// The blockstate file has no part for the state, given as property value indices.
    NoPart { state: Vec<usize> },
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
        }
    }
}

impl Error for ContentError {}
