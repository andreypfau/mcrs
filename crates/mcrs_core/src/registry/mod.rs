pub mod access;
pub mod resource_key;
pub mod snapshot;
pub mod static_registry;

pub use access::{ErasedEntry, ErasedRegistrySnapshot, PackSource, RegistryAccess, RegistrySnapshotErased};
pub use resource_key::ResourceKey;
pub use snapshot::{RegistrySnapshot, SnapshotEntry};
pub use static_registry::{StaticId, StaticRegistry};
