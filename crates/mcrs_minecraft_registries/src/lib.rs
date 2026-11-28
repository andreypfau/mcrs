#![cfg_attr(not(test), deny(clippy::arithmetic_side_effects))]

extern crate self as mcrs_minecraft_registries;

#[cfg(feature = "bevy")]
mod bevy;
mod content_paths;
mod declaration;
mod diagnostics;
pub mod dispatch;
mod load;
mod network;
mod order;
mod receive;
mod refs;
mod registries;
mod registry;
mod tag_file;
mod tags;
mod text;
mod wire;

#[cfg(feature = "bevy")]
pub use bevy::CurrentRegistries;
pub use content_paths::ContentPaths;
pub use declaration::{DataDeclaration, Declaration, RegistryAdditions, insert_entry};
pub use diagnostics::{Diagnostic, Diagnostics, Missing, Referrer};
pub use mcrs_minecraft_registries_derive::{FromWire, Resolve, dispatch, registry_value};
pub use mcrs_minecraft_static_registry as static_key;
pub use mcrs_minecraft_static_registry::{
    StaticEntry, decode_registry_id, encode_registry_id, static_registry,
};
pub use network::NetworkSnapshot;
pub use receive::{Received, ReceivedTags};
pub use refs::{
    Checked, Inline, KeySet, Named, Namer, Ref, Refs, Resolve, ResolveError, Resolved, Resolver,
};
pub use registries::{Registries, Stage};
pub use registry::{DataValue, Entries, Registry};
pub use tag_file::TagEntry;
pub use tags::{IdSet, Tags};
pub use wire::{
    BlockStates, FromWire, Wire, WireId, WireNames, WireSet, decode_holder_id, encode_holder_id,
};

#[doc(hidden)]
pub use mcrs_minecraft_static_registry::{__serde, __wire};
