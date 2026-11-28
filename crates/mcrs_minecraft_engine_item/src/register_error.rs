use core::error::Error;
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegisterError {
    DuplicateKey,
    MaskFull,
    UnsupportedStorage,
    GateTableFull,
    SourcesExhausted,
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DuplicateKey => "an item component type with this key is registered",
            Self::MaskFull => "the world has the maximum number of item component types",
            Self::UnsupportedStorage => "an item component type must be stored in tables",
            Self::GateTableFull => "the world has the maximum number of slot gates",
            Self::SourcesExhausted => "the world has the maximum number of request sources",
        })
    }
}

impl Error for RegisterError {}
