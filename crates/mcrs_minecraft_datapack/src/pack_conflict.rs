use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PackConflict {
    LocalNamedAsStandard(String),
    EmbeddedIdTaken(String),
}

impl fmt::Display for PackConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackConflict::LocalNamedAsStandard(name) => {
                write!(f, "the extra pack {name} has the name of a local pack")
            }
            PackConflict::EmbeddedIdTaken(id) => {
                write!(f, "the embedded pack {id} has the id of another pack")
            }
        }
    }
}

impl core::error::Error for PackConflict {}
