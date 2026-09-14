#[cfg(feature = "bevy")]
pub mod app;
pub mod bitset;
pub mod dyn_index;
#[cfg(feature = "bevy")]
pub mod file;
pub mod key;
#[cfg(feature = "bevy")]
pub mod registry;
#[cfg(feature = "bevy")]
pub mod tag_ref;

#[cfg(feature = "bevy")]
pub use app::{TagLoadersSettled, TagPhase, TagRegistryAppExt};
pub use bitset::{BitSet, IdBitSet, RawBitSet, TagId};
pub use dyn_index::DynRegistryIndex;
#[cfg(feature = "bevy")]
pub use file::{TagEntry, TagFile, TagFileLoader, TagFileSettings};
pub use key::{TagKey, TaggedRegistry};
#[cfg(feature = "bevy")]
pub use registry::{
    DynTagLoader, DynTagRegistry, TagLoader, TagRegistry, TagSource, resolve_tag_file,
    resolve_tag_file_ordered,
};
#[cfg(feature = "bevy")]
pub use tag_ref::TagRef;
