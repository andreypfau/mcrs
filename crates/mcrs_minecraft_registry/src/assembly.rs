use crate::registry::RegistryError;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use std::fmt;
use std::sync::Arc;

type Name = ResourceLocation<Arc<str>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyError {
    Registry(RegistryError),
    UnknownRegistry {
        registry: String,
    },
    DuplicateTag {
        registry: Name,
        tag: Name,
    },
    TooManyTags {
        registry: Name,
        len: usize,
    },
    MemberOutOfRange {
        registry: Name,
        tag: Name,
        member: u16,
    },
    PackOutOfRange {
        registry: Name,
        entry: usize,
        pack: u32,
    },
    SyncedTwice {
        registry: Name,
    },
    NotParsed {
        registry: String,
    },
    WrongValueType {
        registry: String,
        parses: &'static str,
        given: &'static str,
    },
}

impl From<RegistryError> for AssemblyError {
    fn from(error: RegistryError) -> Self {
        AssemblyError::Registry(error)
    }
}

impl fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssemblyError::Registry(error) => error.fmt(f),
            AssemblyError::UnknownRegistry { registry } => {
                write!(f, "the registry set holds no registry {registry}")
            }
            AssemblyError::DuplicateTag { registry, tag } => {
                write!(f, "registry {registry} lists the tag #{tag} twice")
            }
            AssemblyError::TooManyTags { registry, len } => {
                write!(
                    f,
                    "registry {registry} has {len} tags, more than a tag id can number"
                )
            }
            AssemblyError::MemberOutOfRange {
                registry,
                tag,
                member,
            } => write!(
                f,
                "tag #{tag} of registry {registry} holds the id {member}, past the end of the registry"
            ),
            AssemblyError::PackOutOfRange {
                registry,
                entry,
                pack,
            } => write!(
                f,
                "entry {entry} of registry {registry} comes from pack {pack}, which the pack list lacks"
            ),
            AssemblyError::SyncedTwice { registry } => {
                write!(f, "registry {registry} is synced twice")
            }
            AssemblyError::NotParsed { registry } => {
                write!(f, "registry {registry} declares no value type")
            }
            AssemblyError::WrongValueType {
                registry,
                parses,
                given,
            } => write!(
                f,
                "registry {registry} parses {parses} and was given values of {given}"
            ),
        }
    }
}

impl std::error::Error for AssemblyError {}
