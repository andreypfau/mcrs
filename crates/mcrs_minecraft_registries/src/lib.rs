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
mod diagnostics;
pub mod dispatch;
mod load;
mod network;
mod order;
mod refs;
mod registries;
mod registry;
mod require;
pub mod static_key;
mod tags;
mod text;

#[cfg(feature = "bevy")]
pub use bevy::CurrentRegistries;
pub use diagnostics::{Diagnostic, Diagnostics, Missing, Referrer};
pub use load::{
    DataDeclaration, Declaration, LoadCtx, Received, TagEntry, insert_entry, load, load_received,
};
pub use network::NetworkSnapshot;
pub use registries::{Registries, Stage};
pub use mcrs_minecraft_registries_derive::{Resolve, dispatch, registry_value};
pub use refs::{
    Inline, KeySet, Named, Namer, Ref, Refs, Resolve, ResolveError, Resolved, Resolver,
};
pub use registry::{DataValue, Entries, Registry};
pub use require::{require_id, require_registry, require_tags};
pub use static_key::StaticEntry;
pub use tags::{IdSet, Tags};

#[doc(hidden)]
pub use serde as __serde;
