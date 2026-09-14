/// `SharedConstants.getCurrentVersion().name()`. There is no launcher and no
/// version manifest to read it from, so the target version is stated once here.
pub const VERSION_NAME: &str = "26.3-snapshot-9";

#[cfg(feature = "bevy")]
pub mod asset;
#[cfg(feature = "bevy")]
mod plugin;
pub mod registry;
pub mod resource_location;
#[cfg(feature = "bevy")]
pub mod state;
pub mod tag;

#[cfg(feature = "bevy")]
pub use plugin::MinecraftCorePlugin;
#[cfg(feature = "bevy")]
pub use registry::{
    PackSource, RegistryAccess, RegistrySnapshot, RegistrySnapshotErased, SnapshotEntry,
};
pub use registry::{ResourceKey, StaticId, StaticRegistry};
pub use resource_location::ResourceLocation;
#[cfg(feature = "bevy")]
pub use state::AppState;
pub use tag::{DynRegistryIndex, IdBitSet, RawBitSet, TagKey, TaggedRegistry};
#[cfg(feature = "bevy")]
pub use tag::{
    DynTagLoader, DynTagRegistry, TagEntry, TagFile, TagFileLoader, TagFileSettings, TagLoader,
    TagPhase, TagRef, TagRegistry, TagRegistryAppExt, TagSource,
};

// Re-export the proc macro for the rl! declarative macro.
#[doc(hidden)]
pub use mcrs_minecraft_core_macros::rl_impl as __rl_impl;
