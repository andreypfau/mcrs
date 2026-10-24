#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]

extern crate self as mcrs_minecraft_registries;

#[cfg(feature = "bevy")]
mod bevy;
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
pub mod static_key;
mod tag_file;
mod tags;
mod text;
mod wire;

#[cfg(feature = "bevy")]
pub use bevy::CurrentRegistries;
pub use declaration::{DataDeclaration, Declaration, RegistryAdditions, insert_entry};
pub use diagnostics::{Diagnostic, Diagnostics, Missing, Referrer};
pub use mcrs_minecraft_registries_derive::{FromWire, Resolve, dispatch, registry_value};
pub use network::NetworkSnapshot;
pub use receive::{Received, ReceivedTags};
pub use refs::{
    Checked, Inline, KeySet, Named, Namer, Ref, Refs, Resolve, ResolveError, Resolved, Resolver,
};
pub use registries::{Registries, Stage};
pub use registry::{DataValue, Entries, Registry};
pub use static_key::StaticEntry;
pub use tag_file::TagEntry;
pub use tags::{IdSet, Tags};
pub use wire::{BlockStates, FromWire, Wire, WireId, WireNames, WireSet};

#[doc(hidden)]
pub use serde as __serde;
