use std::path::PathBuf;

use mcrs_minecraft_save::SaveError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WorldOpenError {
    #[error("{dir}: the world is open in another process")]
    Locked { dir: PathBuf },
    #[error(
        "{dir}: level.dat has DataVersion {}, expected {expected}",
        found.map_or_else(|| "none".to_owned(), |found| found.to_string())
    )]
    DataVersion {
        dir: PathBuf,
        found: Option<i32>,
        expected: i32,
    },
    #[error(transparent)]
    Save(#[from] SaveError),
    #[error("{0}")]
    Dimensions(String),
}
