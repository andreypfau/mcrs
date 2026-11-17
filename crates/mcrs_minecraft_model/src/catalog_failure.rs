use std::error::Error;
use std::fmt;

use mcrs_minecraft_model_file::content_error::ContentError;

use crate::bake_error::BakeError;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum CatalogFailure {
    PastStates { id: u16, count: usize },
    NoOwner { id: u16 },
    Build { state: String, cause: BakeError },
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
