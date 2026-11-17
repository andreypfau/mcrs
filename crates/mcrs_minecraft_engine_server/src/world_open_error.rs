use std::path::PathBuf;

use mcrs_minecraft_save::SaveError;

/// Why a server did not open its save directory and did not start.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WorldOpenError {
    /// Another process holds the lock of the save directory.
    #[error("{dir}: the world is open in another process")]
    Locked { dir: PathBuf },
    /// `level.dat` has no `DataVersion` (`found` is `None`) or another one than `expected`.
    #[error(
        "{dir}: level.dat has DataVersion {}, expected {expected}",
        found.map_or_else(|| "none".to_owned(), |found| found.to_string())
    )]
    DataVersion {
        dir: PathBuf,
        found: Option<i32>,
        expected: i32,
    },
    /// A file of the save directory does not read or does not write; the error names the file.
    #[error(transparent)]
    Save(#[from] SaveError),
    /// The dimension list or the world preset does not resolve against the registries.
    #[error("{0}")]
    Dimensions(String),
}
