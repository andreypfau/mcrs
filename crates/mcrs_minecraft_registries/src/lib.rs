#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

extern crate self as mcrs_minecraft_registries;

#[cfg(feature = "bevy")]
mod bevy;
mod diagnostics;
mod id;
mod json;
mod load;
mod order;
mod pack;
mod refs;
mod registry;
mod require;
pub mod static_key;
mod tags;
mod text;

#[cfg(feature = "bevy")]
pub use bevy::CurrentRegistries;
pub use diagnostics::{Diagnostic, Diagnostics, Level, Missing, Referrer};
pub use id::Key;
pub use load::{
    DataDeclaration, Declaration, LoadCtx, Registries, RegistryView, Stage, StageScope, StageView,
    ValueSeed, load, load_with,
};
pub use mcrs_minecraft_registries_derive::{Resolve, registry_value};
pub use pack::{
    AvailablePack, Features, KnownPack, PackFiles, PackIndex, PackSelection, PackStack,
    StackedPack, pack_stack,
};
pub use refs::{
    Inline, KeyOr, KeySet, Named, Namer, Ref, Refs, Resolve, ResolveError, Resolved, Resolver,
};
pub use registry::{ReadValue, Registry, RegistryOf, RegistryType};
pub use require::{require_id, require_registry, require_tag, require_tags};
pub use static_key::StaticKey;
pub use tags::{IdSet, Tags};

#[doc(hidden)]
pub use serde as __serde;

// chisle: a value read this way resolves its references through whatever thread-local scope the
// caller installs around the stage, not through the `LoadCtx` seeds; giving the type a seed-based
// `ReadValue` retires its use of this macro, and the macro goes with the last one.
#[doc(hidden)]
#[macro_export]
macro_rules! read_with_serde {
    ($($value:ty),+ $(,)?) => {$(
        impl $crate::ReadValue for $value {
            fn read<'de, D: $crate::__serde::Deserializer<'de>>(
                _: &$crate::LoadCtx<'_>,
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                <Self as $crate::__serde::Deserialize<'de>>::deserialize(deserializer)
            }
        }
    )+};
}

#[macro_export]
macro_rules! declare_registries {
    ($($value:ty => $key:expr, $stage:ident $(, $non_empty:ident)?;)+) => {$(
        impl $value {
            pub fn declaration() -> $crate::DataDeclaration<$crate::RegistryOf<$value>> {
                $crate::Declaration::data($key.location().into(), $crate::Stage::$stage)
                    $(.$non_empty())?
            }
        }
    )+};
}
