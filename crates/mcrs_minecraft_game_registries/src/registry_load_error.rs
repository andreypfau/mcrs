use std::fmt;

use mcrs_minecraft_datapack::{Level, PackConflict, ReadError};
use mcrs_minecraft_registries::Diagnostics;

#[derive(Debug)]
pub enum RegistryLoadError {
    Read(ReadError),
    Declarations(String),
    Pack(PackConflict),
    Load(Diagnostics),
}

impl fmt::Display for RegistryLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryLoadError::Read(error) => write!(f, "cannot read {error}"),
            RegistryLoadError::Declarations(message) => f.write_str(message),
            RegistryLoadError::Pack(conflict) => conflict.fmt(f),
            RegistryLoadError::Load(diagnostics) => {
                let errors = diagnostics.iter().filter(|d| d.level() == Level::Error);
                write!(f, "registry load failed: {} errors", errors.count())?;
                for diagnostic in diagnostics.iter() {
                    write!(f, "\n{:?}: {diagnostic}", diagnostic.level())?;
                }
                Ok(())
            }
        }
    }
}

impl core::error::Error for RegistryLoadError {}

impl From<ReadError> for RegistryLoadError {
    fn from(error: ReadError) -> Self {
        RegistryLoadError::Read(error)
    }
}
