#[cfg(feature = "bevy")]
pub mod access;
pub mod resource_key;
#[cfg(feature = "bevy")]
pub mod snapshot;
pub mod static_registry;

#[cfg(feature = "bevy")]
pub use access::{
    ErasedEntry, ErasedRegistrySnapshot, PackSource, RegistryAccess, RegistrySnapshotErased,
};
pub use resource_key::ResourceKey;
#[cfg(feature = "bevy")]
pub use snapshot::{RegistrySnapshot, SnapshotEntry};
pub use static_registry::{StaticId, StaticRegistry};
