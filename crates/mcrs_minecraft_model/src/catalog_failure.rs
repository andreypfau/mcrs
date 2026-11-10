use std::error::Error;
use std::fmt;

use mcrs_minecraft_model_file::content_error::ContentError;

use crate::bake_error::BakeError;

/// A state or a file that the catalog bake skipped.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum CatalogFailure {
    /// A state id past the states of the corpus.
    PastStates { id: u16, count: usize },
    /// No registry block owns the state.
    NoOwner { id: u16 },
    /// The model of the state does not bake.
    Build { state: String, cause: BakeError },
    /// A pack file the bake read is wrong.
    Content(ContentError),
}

impl fmt::Display for CatalogFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogFailure::PastStates { id, count } => {
                write!(f, "state {id}: past the {count} corpus states")
            }
            CatalogFailure::NoOwner { id } => write!(f, "state {id}: no registry block owns it"),
            CatalogFailure::Build { state, cause } => write!(f, "{state}: {cause}"),
            CatalogFailure::Content(error) => write!(f, "content error: {error}"),
        }
    }
}

impl Error for CatalogFailure {}
